use std::{
    cell::{RefCell, RefMut},
    mem::{self, ManuallyDrop},
    os::raw::c_void,
    ptr,
    time::Instant,
};

use crate::{
    dpi::LogicalSize,
    event::{Event, StartCause, WindowEvent},
    event_loop::ControlFlow,
    window::WindowId,
};

use crate::platform_impl::platform::{
    event_loop::{EventHandler, EventProxy, EventWrapper},
    ffi::{
        id, kCFRunLoopCommonModes, CFAbsoluteTimeGetCurrent, CFRelease, CFRunLoopAddTimer,
        CFRunLoopGetMain, CFRunLoopRef, CFRunLoopTimerCreate, CFRunLoopTimerInvalidate,
        CFRunLoopTimerRef, CFRunLoopTimerSetNextFireDate, CGRect, CGSize, NSUInteger,
    },
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
        queued_windows: Vec<id>,
        queued_events: Vec<EventWrapper>,
    },
    Launching {
        queued_windows: Vec<id>,
        queued_events: Vec<EventWrapper>,
        queued_event_handler: Box<dyn EventHandler>,
    },
    ProcessingEvents {
        event_handler: Box<dyn EventHandler>,
        active_control_flow: ControlFlow,
    },
    // special state to deal with reentrancy and prevent mutable aliasing.
    InUserCallback {
        queued_events: Vec<EventWrapper>,
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

impl Drop for AppStateImpl {
    fn drop(&mut self) {
        match self {
            &mut AppStateImpl::NotLaunched {
                ref mut queued_windows,
                ..
            }
            | &mut AppStateImpl::Launching {
                ref mut queued_windows,
                ..
            } => unsafe {
                for &mut window in queued_windows {
                    let () = msg_send![window, release];
                }
            },
            _ => {}
        }
    }
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

    // requires main thread and window is a UIWindow
    // retains window
    pub unsafe fn set_key_window(window: id) {
        let mut this = AppState::get_mut();
        match &mut this.app_state {
            &mut AppStateImpl::NotLaunched {
                ref mut queued_windows,
                ..
            } => {
                queued_windows.push(window);
                msg_send![window, retain];
                return;
            }
            &mut AppStateImpl::ProcessingEvents { .. } => {}
            &mut AppStateImpl::InUserCallback { .. } => {}
            &mut AppStateImpl::Terminated => panic!(
                "Attempt to create a `Window` \
                 after the app has terminated"
            ),
            app_state => unreachable!("unexpected state: {:#?}", app_state), /* all other cases should be impossible */
        }
        drop(this);
        msg_send![window, makeKeyAndVisible]
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
        let windows = match &mut this.app_state {
            &mut AppStateImpl::Launching {
                ref mut queued_windows,
                ..
            } => mem::replace(queued_windows, Vec::new()),
            _ => panic!(
                "winit iOS expected the app to be in a `Launching` \
                 state, but was not - please file an issue"
            ),
        };
        // have to drop RefMut because the window setup code below can trigger new events
        drop(this);

        for window in windows {
            let count: NSUInteger = msg_send![window, retainCount];
            // make sure the window is still referenced
            if count > 1 {
                // Do a little screen dance here to account for windows being created before
                // `UIApplicationMain` is called. This fixes visual issues such as being
                // offcenter and sized incorrectly. Additionally, to fix orientation issues, we
                // gotta reset the `rootViewController`.
                //
                // relevant iOS log:
                // ```
                // [ApplicationLifecycle] Windows were created before application initialzation
                // completed. This may result in incorrect visual appearance.
                // ```
                let screen: id = msg_send![window, screen];
                let () = msg_send![screen, retain];
                let () = msg_send![window, setScreen:0 as id];
                let () = msg_send![window, setScreen: screen];
                let () = msg_send![screen, release];
                let controller: id = msg_send![window, rootViewController];
                let () = msg_send![window, setRootViewController:ptr::null::<()>()];
                let () = msg_send![window, setRootViewController: controller];
                let () = msg_send![window, makeKeyAndVisible];
            }
            let () = msg_send![window, release];
        }

        let mut this = AppState::get_mut();
        let (windows, events, event_handler) = match &mut this.app_state {
            &mut AppStateImpl::Launching {
                ref mut queued_windows,
                ref mut queued_events,
                ref mut queued_event_handler,
            } => {
                let windows = ptr::read(queued_windows);
                let events = ptr::read(queued_events);
                let event_handler = ptr::read(queued_event_handler);
                (windows, events, event_handler)
            }
            _ => panic!(
                "winit iOS expected the app to be in a `Launching` \
                 state, but was not - please file an issue"
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

        let events = std::iter::once(EventWrapper::StaticEvent(Event::NewEvents(
            StartCause::Init,
        )))
        .chain(events);
        AppState::handle_nonuser_events(events);

        // the above window dance hack, could possibly trigger new windows to be created.
        // we can just set those windows up normally, as they were created after didFinishLaunching
        for window in windows {
            let count: NSUInteger = msg_send![window, retainCount];
            // make sure the window is still referenced
            if count > 1 {
                let () = msg_send![window, makeKeyAndVisible];
            }
            let () = msg_send![window, release];
        }
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
        AppState::handle_nonuser_event(EventWrapper::StaticEvent(event))
    }

    // requires main thread
    pub unsafe fn handle_nonuser_event(event_wrapper: EventWrapper) {
        AppState::handle_nonuser_events(std::iter::once(event_wrapper))
    }

    // requires main thread
    pub unsafe fn handle_nonuser_events<I: IntoIterator<Item = EventWrapper>>(event_wrappers: I) {
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
                queued_events.extend(event_wrappers);
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

        for event_wrapper in event_wrappers {
            match event_wrapper {
                EventWrapper::StaticEvent(event) => {
                    event_handler.handle_nonuser_event(event, &mut control_flow)
                }
                EventWrapper::EventProxy(proxy) => {
                    Self::handle_event_proxy(&mut event_handler, control_flow, proxy)
                }
            }
        }
        loop {
            let mut this = AppState::get_mut();
            let queued_event_wrappers = match &mut this.app_state {
                &mut AppStateImpl::InUserCallback {
                    ref mut queued_events,
                } => mem::replace(queued_events, Vec::new()),
                _ => bug!("unexpected `AppStateImpl`"),
            };
            if queued_event_wrappers.is_empty() {
                this.app_state = AppStateImpl::ProcessingEvents {
                    event_handler,
                    active_control_flow,
                };
                this.control_flow = control_flow;
                break;
            }
            drop(this);
            for event_wrapper in queued_event_wrappers {
                match event_wrapper {
                    EventWrapper::StaticEvent(event) => {
                        event_handler.handle_nonuser_event(event, &mut control_flow)
                    }
                    EventWrapper::EventProxy(proxy) => {
                        Self::handle_event_proxy(&mut event_handler, control_flow, proxy)
                    }
                }
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
            let queued_event_wrappers = match &mut this.app_state {
                &mut AppStateImpl::InUserCallback {
                    ref mut queued_events,
                } => mem::replace(queued_events, Vec::new()),
                _ => bug!("unexpected `AppStateImpl`"),
            };
            if queued_event_wrappers.is_empty() {
                this.app_state = AppStateImpl::ProcessingEvents {
                    event_handler,
                    active_control_flow,
                };
                this.control_flow = control_flow;
                break;
            }
            drop(this);
            for event_wrapper in queued_event_wrappers {
                match event_wrapper {
                    EventWrapper::StaticEvent(event) => {
                        event_handler.handle_nonuser_event(event, &mut control_flow)
                    }
                    EventWrapper::EventProxy(proxy) => {
                        Self::handle_event_proxy(&mut event_handler, control_flow, proxy)
                    }
                }
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
        AppState::handle_nonuser_event(EventWrapper::StaticEvent(Event::EventsCleared));

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

    fn get_view_and_screen_frame(window_id: id) -> (id, CGRect) {
        unsafe {
            let view_controller: id = msg_send![window_id, rootViewController];
            let view: id = msg_send![view_controller, view];
            let bounds: CGRect = msg_send![window_id, bounds];
            let screen: id = msg_send![window_id, screen];
            let screen_space: id = msg_send![screen, coordinateSpace];
            let screen_frame: CGRect =
                msg_send![window_id, convertRect:bounds toCoordinateSpace:screen_space];
            (view, screen_frame)
        }
    }

    fn handle_event_proxy(
        event_handler: &mut Box<dyn EventHandler>,
        control_flow: ControlFlow,
        proxy: EventProxy,
    ) {
        match proxy {
            EventProxy::HiDpiFactorChangedProxy {
                suggested_size,
                hidpi_factor,
                window_id,
            } => Self::handle_hidpi_proxy(
                event_handler,
                control_flow,
                suggested_size,
                hidpi_factor,
                window_id,
            ),
        }
    }

    fn handle_hidpi_proxy(
        event_handler: &mut Box<dyn EventHandler>,
        mut control_flow: ControlFlow,
        suggested_size: LogicalSize,
        hidpi_factor: f64,
        window_id: id,
    ) {
        let size = suggested_size.to_physical(hidpi_factor);
        let new_inner_size = &mut Some(size);
        let event = Event::WindowEvent {
            window_id: WindowId(window_id.into()),
            event: WindowEvent::HiDpiFactorChanged {
                hidpi_factor,
                new_inner_size,
            },
        };
        event_handler.handle_nonuser_event(event, &mut control_flow);
        let (view, screen_frame) = Self::get_view_and_screen_frame(window_id);
        if let Some(physical_size) = new_inner_size {
            let logical_size = physical_size.to_logical(hidpi_factor);
            let size = CGSize::new(logical_size);
            let new_frame: CGRect = CGRect::new(screen_frame.origin, size);
            unsafe {
                let () = msg_send![view, setFrame: new_frame];
            }
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
            // create a timer with a 1microsec interval (1ns does not work) to mimic polling.
            // it is initially setup with a first fire time really far into the
            // future, but that gets changed to fire immediatley in did_finish_launching
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
