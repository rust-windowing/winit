use std::{
    cell::{RefCell, RefMut},
    mem::{self, ManuallyDrop},
    os::raw::c_void,
    ptr,
    time::Instant,
};

use crate::{
    event::{Event, StartCause},
    event_loop::ControlFlow,
};

use crate::platform_impl::platform::{
    event_loop::{EventHandler, Never},
    ffi::{
        id, kCFRunLoopCommonModes, CFAbsoluteTimeGetCurrent, CFRelease, CFRunLoopAddTimer,
        CFRunLoopGetMain, CFRunLoopRef, CFRunLoopTimerCreate, CFRunLoopTimerInvalidate,
        CFRunLoopTimerRef, CFRunLoopTimerSetNextFireDate, NSUInteger,
    },
    window::Inner as WindowInner,
};

macro_rules! bug {
    ($msg:expr) => {
        panic!("winit iOS bug, file an issue: {}", $msg)
    };
}

// this is the state machine for the app lifecycle
#[derive(Debug)]
enum AppStateImpl {
    NotLaunched {
        queued_windows: Vec<*mut WindowInner>,
        queued_events: Vec<Event<Never>>,
    },
    Launching {
        queued_windows: Vec<*mut WindowInner>,
        queued_events: Vec<Event<Never>>,
        queued_event_handler: Box<dyn EventHandler>,
    },
    ProcessingEvents {
        event_handler: Box<dyn EventHandler>,
        active_control_flow: ControlFlow,
    },
    // special state to deal with reentrancy and prevent mutable aliasing.
    InUserCallback {
        queued_events: Vec<Event<Never>>,
    },
    Waiting {
        waiting_event_handler: Box<dyn EventHandler>,
        start: Instant,
    },
    PollFinished {
        waiting_event_handler: Box<dyn EventHandler>,
    },
    Terminated,
}

pub struct AppState {
    app_state: AppStateImpl,
    control_flow: ControlFlow,
    waker: EventLoopWaker,
}

impl AppState {
    // requires main thread
    unsafe fn get_mut() -> RefMut<'static, AppState> {
        // basically everything in UIKit requires the main thread, so it's pointless to use the
        // std::sync APIs.
        // must be mut because plain `static` requires `Sync`
        static mut APP_STATE: RefCell<Option<AppState>> = RefCell::new(None);

        if cfg!(debug_assertions) {
            assert_main_thread!(
                "bug in winit: `AppState::get_mut()` can only be called on the main thread"
            );
        }

        let mut guard = APP_STATE.borrow_mut();
        if guard.is_none() {
            #[inline(never)]
            #[cold]
            unsafe fn init_guard(guard: &mut RefMut<'static, Option<AppState>>) {
                let waker = EventLoopWaker::new(CFRunLoopGetMain());
                **guard = Some(AppState {
                    app_state: AppStateImpl::NotLaunched {
                        queued_windows: Vec::new(),
                        queued_events: Vec::new(),
                    },
                    control_flow: ControlFlow::default(),
                    waker,
                });
            }
            init_guard(&mut guard)
        }
        RefMut::map(guard, |state| state.as_mut().unwrap())
    }

    pub unsafe fn defer_window_init(window: *mut WindowInner) {
        let mut this = AppState::get_mut();
        match this.app_state {
            // `UIApplicationMain` not called yet, so defer initialization
            AppStateImpl::NotLaunched {
                ref mut queued_windows,
                ..
            } => queued_windows.push(window),
            // `UIApplicationMain` already called, so initialize immediately
            _ => (*window).init(),
            ref app_state => unreachable!("unexpected state: {:#?}", app_state),
        }
        drop(this);
    }

    pub unsafe fn cancel_deferred_window_init(window: *mut WindowInner) {
        let mut this = AppState::get_mut();
        match this.app_state {
            AppStateImpl::NotLaunched {
                ref mut queued_windows,
                ..
            }
            | AppStateImpl::Launching {
                ref mut queued_windows,
                ..
            } => {
                queued_windows.remove(queued_windows.iter().position(|x| x == &window).unwrap());
            }
            _ => (),
        }
        drop(this);
    }

    // requires main thread
    pub unsafe fn will_launch(queued_event_handler: Box<dyn EventHandler>) {
        let mut this = AppState::get_mut();
        let (queued_windows, queued_events) = match &mut this.app_state {
            &mut AppStateImpl::NotLaunched {
                ref mut queued_windows,
                ref mut queued_events,
            } => {
                let windows = ptr::read(queued_windows);
                let events = ptr::read(queued_events);
                (windows, events)
            }
            _ => panic!(
                "winit iOS expected the app to be in a `NotLaunched` \
                 state, but was not - please file an issue"
            ),
        };
        ptr::write(
            &mut this.app_state,
            AppStateImpl::Launching {
                queued_windows,
                queued_events,
                queued_event_handler,
            },
        );
    }

    // requires main thread
    pub unsafe fn did_finish_launching() {
        let mut this = AppState::get_mut();
        let (windows, events, event_handler) = match &mut this.app_state {
            &mut AppStateImpl::Launching {
                ref mut queued_windows,
                ref mut queued_event_handler,
                ref mut queued_events,
                ..
            } => {
                let windows = mem::replace(queued_windows, Vec::new());
                let events = ptr::read(queued_events);
                let event_handler = ptr::read(queued_event_handler);
                (windows, events, event_handler)
            }
            _ => panic!(
                "winit iOS expected the app to be in a `Launching` \
                 state, but was not - please file an issue"
            ),
        };
        // have to drop RefMut because the window setup code below can trigger new events
        drop(this);

        // Create UIKit views, view controllers and windows for each window
        for window in windows {
            (*window).init();
        }

        let mut this = AppState::get_mut();
        match this.app_state {
            AppStateImpl::Launching { .. } => (),
            _ => unreachable!(
                "winit iOS expected the app to be in a `Launching` state, but was not - please \
                 file an issue"
            ),
        };
        ptr::write(
            &mut this.app_state,
            AppStateImpl::ProcessingEvents {
                event_handler,
                active_control_flow: ControlFlow::Poll,
            },
        );
        drop(this);

        let events = std::iter::once(Event::NewEvents(StartCause::Init)).chain(events);
        AppState::handle_nonuser_events(events);
    }

    // requires main thread
    // AppState::did_finish_launching handles the special transition `Init`
    pub unsafe fn handle_wakeup_transition() {
        let mut this = AppState::get_mut();
        let event =
            match this.control_flow {
                ControlFlow::Poll => {
                    let event_handler = match &mut this.app_state {
                        &mut AppStateImpl::NotLaunched { .. }
                        | &mut AppStateImpl::Launching { .. } => return,
                        &mut AppStateImpl::PollFinished {
                            ref mut waiting_event_handler,
                        } => ptr::read(waiting_event_handler),
                        _ => bug!("`EventHandler` unexpectedly started polling"),
                    };
                    ptr::write(
                        &mut this.app_state,
                        AppStateImpl::ProcessingEvents {
                            event_handler,
                            active_control_flow: ControlFlow::Poll,
                        },
                    );
                    Event::NewEvents(StartCause::Poll)
                }
                ControlFlow::Wait => {
                    let (event_handler, start) = match &mut this.app_state {
                        &mut AppStateImpl::NotLaunched { .. }
                        | &mut AppStateImpl::Launching { .. } => return,
                        &mut AppStateImpl::Waiting {
                            ref mut waiting_event_handler,
                            ref mut start,
                        } => (ptr::read(waiting_event_handler), *start),
                        _ => bug!("`EventHandler` unexpectedly woke up"),
                    };
                    ptr::write(
                        &mut this.app_state,
                        AppStateImpl::ProcessingEvents {
                            event_handler,
                            active_control_flow: ControlFlow::Wait,
                        },
                    );
                    Event::NewEvents(StartCause::WaitCancelled {
                        start,
                        requested_resume: None,
                    })
                }
                ControlFlow::WaitUntil(requested_resume) => {
                    let (event_handler, start) = match &mut this.app_state {
                        &mut AppStateImpl::NotLaunched { .. }
                        | &mut AppStateImpl::Launching { .. } => return,
                        &mut AppStateImpl::Waiting {
                            ref mut waiting_event_handler,
                            ref mut start,
                        } => (ptr::read(waiting_event_handler), *start),
                        _ => bug!("`EventHandler` unexpectedly woke up"),
                    };
                    ptr::write(
                        &mut this.app_state,
                        AppStateImpl::ProcessingEvents {
                            event_handler,
                            active_control_flow: ControlFlow::WaitUntil(requested_resume),
                        },
                    );
                    if Instant::now() >= requested_resume {
                        Event::NewEvents(StartCause::ResumeTimeReached {
                            start,
                            requested_resume,
                        })
                    } else {
                        Event::NewEvents(StartCause::WaitCancelled {
                            start,
                            requested_resume: Some(requested_resume),
                        })
                    }
                }
                ControlFlow::Exit => bug!("unexpected controlflow `Exit`"),
            };
        drop(this);
        AppState::handle_nonuser_event(event)
    }

    // requires main thread
    pub unsafe fn handle_nonuser_event(event: Event<Never>) {
        AppState::handle_nonuser_events(std::iter::once(event))
    }

    // requires main thread
    pub unsafe fn handle_nonuser_events<I: IntoIterator<Item = Event<Never>>>(events: I) {
        let mut this = AppState::get_mut();
        let mut control_flow = this.control_flow;
        let (mut event_handler, active_control_flow) = match &mut this.app_state {
            &mut AppStateImpl::Launching {
                ref mut queued_events,
                ..
            }
            | &mut AppStateImpl::NotLaunched {
                ref mut queued_events,
                ..
            }
            | &mut AppStateImpl::InUserCallback {
                ref mut queued_events,
                ..
            } => {
                queued_events.extend(events);
                return;
            }
            &mut AppStateImpl::ProcessingEvents {
                ref mut event_handler,
                ref mut active_control_flow,
            } => (ptr::read(event_handler), *active_control_flow),
            &mut AppStateImpl::PollFinished { .. }
            | &mut AppStateImpl::Waiting { .. }
            | &mut AppStateImpl::Terminated => bug!("unexpected attempted to process an event"),
        };
        ptr::write(
            &mut this.app_state,
            AppStateImpl::InUserCallback {
                queued_events: Vec::new(),
            },
        );
        drop(this);

        for event in events {
            event_handler.handle_nonuser_event(event, &mut control_flow)
        }
        loop {
            let mut this = AppState::get_mut();
            let queued_events = match &mut this.app_state {
                &mut AppStateImpl::InUserCallback {
                    ref mut queued_events,
                } => mem::replace(queued_events, Vec::new()),
                _ => bug!("unexpected `AppStateImpl`"),
            };
            if queued_events.is_empty() {
                this.app_state = AppStateImpl::ProcessingEvents {
                    event_handler,
                    active_control_flow,
                };
                this.control_flow = control_flow;
                break;
            }
            drop(this);
            for event in queued_events {
                event_handler.handle_nonuser_event(event, &mut control_flow)
            }
        }
    }

    // requires main thread
    pub unsafe fn handle_user_events() {
        let mut this = AppState::get_mut();
        let mut control_flow = this.control_flow;
        let (mut event_handler, active_control_flow) = match &mut this.app_state {
            &mut AppStateImpl::NotLaunched { .. } | &mut AppStateImpl::Launching { .. } => return,
            &mut AppStateImpl::ProcessingEvents {
                ref mut event_handler,
                ref mut active_control_flow,
            } => (ptr::read(event_handler), *active_control_flow),
            &mut AppStateImpl::InUserCallback { .. }
            | &mut AppStateImpl::PollFinished { .. }
            | &mut AppStateImpl::Waiting { .. }
            | &mut AppStateImpl::Terminated => bug!("unexpected attempted to process an event"),
        };
        ptr::write(
            &mut this.app_state,
            AppStateImpl::InUserCallback {
                queued_events: Vec::new(),
            },
        );
        drop(this);

        event_handler.handle_user_events(&mut control_flow);
        loop {
            let mut this = AppState::get_mut();
            let queued_events = match &mut this.app_state {
                &mut AppStateImpl::InUserCallback {
                    ref mut queued_events,
                } => mem::replace(queued_events, Vec::new()),
                _ => bug!("unexpected `AppStateImpl`"),
            };
            if queued_events.is_empty() {
                this.app_state = AppStateImpl::ProcessingEvents {
                    event_handler,
                    active_control_flow,
                };
                this.control_flow = control_flow;
                break;
            }
            drop(this);
            for event in queued_events {
                event_handler.handle_nonuser_event(event, &mut control_flow)
            }
            event_handler.handle_user_events(&mut control_flow);
        }
    }

    // requires main thread
    pub unsafe fn handle_events_cleared() {
        let mut this = AppState::get_mut();
        match &mut this.app_state {
            &mut AppStateImpl::NotLaunched { .. } | &mut AppStateImpl::Launching { .. } => return,
            &mut AppStateImpl::ProcessingEvents { .. } => {}
            _ => unreachable!(),
        };
        drop(this);

        AppState::handle_user_events();
        AppState::handle_nonuser_event(Event::EventsCleared);

        let mut this = AppState::get_mut();
        let (event_handler, old) = match &mut this.app_state {
            &mut AppStateImpl::ProcessingEvents {
                ref mut event_handler,
                ref mut active_control_flow,
            } => (
                ManuallyDrop::new(ptr::read(event_handler)),
                *active_control_flow,
            ),
            _ => unreachable!(),
        };

        let new = this.control_flow;
        match (old, new) {
            (ControlFlow::Poll, ControlFlow::Poll) => ptr::write(
                &mut this.app_state,
                AppStateImpl::PollFinished {
                    waiting_event_handler: ManuallyDrop::into_inner(event_handler),
                },
            ),
            (ControlFlow::Wait, ControlFlow::Wait) => {
                let start = Instant::now();
                ptr::write(
                    &mut this.app_state,
                    AppStateImpl::Waiting {
                        waiting_event_handler: ManuallyDrop::into_inner(event_handler),
                        start,
                    },
                )
            }
            (ControlFlow::WaitUntil(old_instant), ControlFlow::WaitUntil(new_instant))
                if old_instant == new_instant =>
            {
                let start = Instant::now();
                ptr::write(
                    &mut this.app_state,
                    AppStateImpl::Waiting {
                        waiting_event_handler: ManuallyDrop::into_inner(event_handler),
                        start,
                    },
                )
            }
            (_, ControlFlow::Wait) => {
                let start = Instant::now();
                ptr::write(
                    &mut this.app_state,
                    AppStateImpl::Waiting {
                        waiting_event_handler: ManuallyDrop::into_inner(event_handler),
                        start,
                    },
                );
                this.waker.stop()
            }
            (_, ControlFlow::WaitUntil(new_instant)) => {
                let start = Instant::now();
                ptr::write(
                    &mut this.app_state,
                    AppStateImpl::Waiting {
                        waiting_event_handler: ManuallyDrop::into_inner(event_handler),
                        start,
                    },
                );
                this.waker.start_at(new_instant)
            }
            (_, ControlFlow::Poll) => {
                ptr::write(
                    &mut this.app_state,
                    AppStateImpl::PollFinished {
                        waiting_event_handler: ManuallyDrop::into_inner(event_handler),
                    },
                );
                this.waker.start()
            }
            (_, ControlFlow::Exit) => {
                // https://developer.apple.com/library/archive/qa/qa1561/_index.html
                // it is not possible to quit an iOS app gracefully and programatically
                warn!("`ControlFlow::Exit` ignored on iOS");
                this.control_flow = old
            }
        }
    }

    pub fn terminated() {
        let mut this = unsafe { AppState::get_mut() };
        let mut old = mem::replace(&mut this.app_state, AppStateImpl::Terminated);
        let mut control_flow = this.control_flow;
        if let AppStateImpl::ProcessingEvents {
            ref mut event_handler,
            ..
        } = old
        {
            drop(this);
            event_handler.handle_nonuser_event(Event::LoopDestroyed, &mut control_flow)
        } else {
            bug!("`LoopDestroyed` happened while not processing events")
        }
    }
}

struct EventLoopWaker {
    timer: CFRunLoopTimerRef,
}

impl Drop for EventLoopWaker {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopTimerInvalidate(self.timer);
            CFRelease(self.timer as _);
        }
    }
}

impl EventLoopWaker {
    fn new(rl: CFRunLoopRef) -> EventLoopWaker {
        extern "C" fn wakeup_main_loop(_timer: CFRunLoopTimerRef, _info: *mut c_void) {}
        unsafe {
            // Create a timer with a 0.1µs interval (1ns does not work) to mimic polling.
            // It is initially setup with a first fire time really far into the
            // future, but that gets changed to fire immediately in did_finish_launching
            let timer = CFRunLoopTimerCreate(
                ptr::null_mut(),
                std::f64::MAX,
                0.000_000_1,
                0,
                0,
                wakeup_main_loop,
                ptr::null_mut(),
            );
            CFRunLoopAddTimer(rl, timer, kCFRunLoopCommonModes);

            EventLoopWaker { timer }
        }
    }

    fn stop(&mut self) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.timer, std::f64::MAX) }
    }

    fn start(&mut self) {
        unsafe { CFRunLoopTimerSetNextFireDate(self.timer, std::f64::MIN) }
    }

    fn start_at(&mut self, instant: Instant) {
        let now = Instant::now();
        if now >= instant {
            self.start();
        } else {
            unsafe {
                let current = CFAbsoluteTimeGetCurrent();
                let duration = instant - now;
                let fsecs =
                    duration.subsec_nanos() as f64 / 1_000_000_000.0 + duration.as_secs() as f64;
                CFRunLoopTimerSetNextFireDate(self.timer, current + fsecs)
            }
        }
    }
}
