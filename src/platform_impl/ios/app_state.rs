use std::{
    cell::{RefCell, RefMut},
    mem::{self, ManuallyDrop},
    os::raw::c_void,
    ptr,
    time::Instant,
};

use objc::runtime::{BOOL, YES};

use crate::{
    event::{Event, StartCause},
    event_loop::ControlFlow,
};

use crate::platform_impl::platform::{
    event_loop::{EventHandler, Never},
    ffi::{
        id, kCFRunLoopCommonModes, CFAbsoluteTimeGetCurrent, CFRelease, CFRunLoopAddTimer,
        CFRunLoopGetMain, CFRunLoopRef, CFRunLoopTimerCreate, CFRunLoopTimerInvalidate,
        CFRunLoopTimerRef, CFRunLoopTimerSetNextFireDate, NSInteger, NSOperatingSystemVersion,
        NSUInteger,
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
        queued_events: Vec<Event<Never>>,
    },
    Launching {
        queued_windows: Vec<id>,
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
                let _: id = msg_send![window, retain];
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
        // start waking up the event loop now!
        debug_assert!(
            this.control_flow == ControlFlow::Poll,
            "winit iOS is unexpectedly not setup to `Poll` on launch!"
        );
        this.waker.start();

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
                let _: id = msg_send![screen, retain];
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

        let events = std::iter::once(Event::NewEvents(StartCause::Init)).chain(events);
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

macro_rules! os_capabilities {
    (
        $(
            $(#[$attr:meta])*
            $error_name:ident: $objc_call:literal,
            $name:ident: $major:literal-$minor:literal
        ),*
        $(,)*
    ) => {
        #[derive(Clone, Debug)]
        pub struct OSCapabilities {
            $(
                pub $name: bool,
            )*

            os_version: NSOperatingSystemVersion,
        }

        impl From<NSOperatingSystemVersion> for OSCapabilities {
            fn from(os_version: NSOperatingSystemVersion) -> OSCapabilities {
                $(let $name = os_version.meets_requirements($major, $minor);)*
                OSCapabilities { $($name,)* os_version, }
            }
        }

        impl OSCapabilities {$(
            $(#[$attr])*
            pub fn $error_name(&self, extra_msg: &str) {
                log::warn!(
                    concat!("`", $objc_call, "` requires iOS {}.{}+. This device is running iOS {}.{}.{}. {}"),
                    $major, $minor, self.os_version.major, self.os_version.minor, self.os_version.patch,
                    extra_msg
                )
            }
        )*}
    };
}

os_capabilities! {
    /// https://developer.apple.com/documentation/uikit/uiview/2891103-safeareainsets?language=objc
    #[allow(unused)] // error message unused
    safe_area_err_msg: "-[UIView safeAreaInsets]",
    safe_area: 11-0,
    /// https://developer.apple.com/documentation/uikit/uiviewcontroller/2887509-setneedsupdateofhomeindicatoraut?language=objc
    home_indicator_hidden_err_msg: "-[UIViewController setNeedsUpdateOfHomeIndicatorAutoHidden]",
    home_indicator_hidden: 11-0,
    /// https://developer.apple.com/documentation/uikit/uiviewcontroller/2887507-setneedsupdateofscreenedgesdefer?language=objc
    defer_system_gestures_err_msg: "-[UIViewController setNeedsUpdateOfScreenEdgesDeferringSystem]",
    defer_system_gestures: 11-0,
    /// https://developer.apple.com/documentation/uikit/uiscreen/2806814-maximumframespersecond?language=objc
    maximum_frames_per_second_err_msg: "-[UIScreen maximumFramesPerSecond]",
    maximum_frames_per_second: 10-3,
    /// https://developer.apple.com/documentation/uikit/uitouch/1618110-force?language=objc
    #[allow(unused)] // error message unused
    force_touch_err_msg: "-[UITouch force]",
    force_touch: 9-0,
}

impl NSOperatingSystemVersion {
    fn meets_requirements(&self, required_major: NSInteger, required_minor: NSInteger) -> bool {
        (self.major, self.minor) >= (required_major, required_minor)
    }
}

pub fn os_capabilities() -> OSCapabilities {
    lazy_static! {
        static ref OS_CAPABILITIES: OSCapabilities = {
            let version: NSOperatingSystemVersion = unsafe {
                let process_info: id = msg_send![class!(NSProcessInfo), processInfo];
                let atleast_ios_8: BOOL = msg_send![
                    process_info,
                    respondsToSelector: sel!(operatingSystemVersion)
                ];
                // winit requires atleast iOS 8 because no one has put the time into supporting earlier os versions.
                // Older iOS versions are increasingly difficult to test. For example, Xcode 11 does not support
                // debugging on devices with an iOS version of less than 8. Another example, in order to use an iOS
                // simulator older than iOS 8, you must download an older version of Xcode (<9), and at least Xcode 7
                // has been tested to not even run on macOS 10.15 - Xcode 8 might?
                //
                // The minimum required iOS version is likely to grow in the future.
                assert!(
                    atleast_ios_8 == YES,
                    "`winit` requires iOS version 8 or greater"
                );
                msg_send![process_info, operatingSystemVersion]
            };
            version.into()
        };
    }
    OS_CAPABILITIES.clone()
}
