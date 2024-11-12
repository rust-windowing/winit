use std::ffi::{c_char, c_int, c_void};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use core_foundation::base::{CFIndex, CFRelease};
use core_foundation::runloop::{
    kCFRunLoopAfterWaiting, kCFRunLoopBeforeWaiting, kCFRunLoopCommonModes, kCFRunLoopDefaultMode,
    kCFRunLoopExit, CFRunLoopActivity, CFRunLoopAddObserver, CFRunLoopAddSource, CFRunLoopGetMain,
    CFRunLoopObserverCreate, CFRunLoopObserverRef, CFRunLoopSourceContext, CFRunLoopSourceCreate,
    CFRunLoopSourceInvalidate, CFRunLoopSourceRef, CFRunLoopSourceSignal, CFRunLoopWakeUp,
};
use objc2::rc::Retained;
use objc2::{msg_send_id, ClassType};
use objc2_foundation::{MainThreadMarker, NSNotificationCenter, NSObject};
use objc2_ui_kit::{
    UIApplication, UIApplicationDidBecomeActiveNotification,
    UIApplicationDidEnterBackgroundNotification, UIApplicationDidFinishLaunchingNotification,
    UIApplicationDidReceiveMemoryWarningNotification, UIApplicationMain,
    UIApplicationWillEnterForegroundNotification, UIApplicationWillResignActiveNotification,
    UIApplicationWillTerminateNotification, UIScreen,
};

use super::super::notification_center::create_observer;
use super::app_state::{send_occluded_event_for_all_windows, AppState, EventWrapper};
use super::{app_state, monitor, MonitorHandle};
use crate::application::ApplicationHandler;
use crate::error::{EventLoopError, NotSupportedError, RequestError};
use crate::event::Event;
use crate::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as CoreEventLoopProxy, EventLoopProxyProvider,
    OwnedDisplayHandle as RootOwnedDisplayHandle,
};
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::platform_impl::Window;
use crate::window::{CustomCursor, CustomCursorSource, Theme, Window as CoreWindow};

#[derive(Debug)]
pub(crate) struct ActiveEventLoop {
    pub(super) mtm: MainThreadMarker,
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> CoreEventLoopProxy {
        CoreEventLoopProxy::new(AppState::get_mut(self.mtm).event_loop_proxy().clone())
    }

    fn create_window(
        &self,
        window_attributes: crate::window::WindowAttributes,
    ) -> Result<Box<dyn CoreWindow>, RequestError> {
        Ok(Box::new(Window::new(self, window_attributes)?))
    }

    fn create_custom_cursor(
        &self,
        _source: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        Err(NotSupportedError::new("create_custom_cursor is not supported").into())
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = RootMonitorHandle>> {
        Box::new(monitor::uiscreens(self.mtm).into_iter().map(|inner| RootMonitorHandle { inner }))
    }

    fn primary_monitor(&self) -> Option<crate::monitor::MonitorHandle> {
        #[allow(deprecated)]
        let monitor = MonitorHandle::new(UIScreen::mainScreen(self.mtm));
        Some(RootMonitorHandle { inner: monitor })
    }

    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn set_control_flow(&self, control_flow: ControlFlow) {
        AppState::get_mut(self.mtm).set_control_flow(control_flow)
    }

    fn system_theme(&self) -> Option<Theme> {
        None
    }

    fn control_flow(&self) -> ControlFlow {
        AppState::get_mut(self.mtm).control_flow()
    }

    fn exit(&self) {
        // https://developer.apple.com/library/archive/qa/qa1561/_index.html
        // it is not possible to quit an iOS app gracefully and programmatically
        tracing::warn!("`ControlFlow::Exit` ignored on iOS");
    }

    fn exiting(&self) -> bool {
        false
    }

    fn owned_display_handle(&self) -> RootOwnedDisplayHandle {
        RootOwnedDisplayHandle { platform: OwnedDisplayHandle }
    }

    fn rwh_06_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
}

impl rwh_06::HasDisplayHandle for ActiveEventLoop {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::RawDisplayHandle::UiKit(rwh_06::UiKitDisplayHandle::new());
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct OwnedDisplayHandle;

impl OwnedDisplayHandle {
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::UiKitDisplayHandle::new().into())
    }
}

pub struct EventLoop {
    mtm: MainThreadMarker,
    window_target: ActiveEventLoop,

    // Since iOS 9.0, we no longer need to remove the observers before they are deallocated; the
    // system instead cleans it up next time it would have posted a notification to it.
    //
    // Though we do still need to keep the observers around to prevent them from being deallocated.
    _did_finish_launching_observer: Retained<NSObject>,
    _did_become_active_observer: Retained<NSObject>,
    _will_resign_active_observer: Retained<NSObject>,
    _will_enter_foreground_observer: Retained<NSObject>,
    _did_enter_background_observer: Retained<NSObject>,
    _will_terminate_observer: Retained<NSObject>,
    _did_receive_memory_warning_observer: Retained<NSObject>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {}

impl EventLoop {
    pub(crate) fn new(
        _: &PlatformSpecificEventLoopAttributes,
    ) -> Result<EventLoop, EventLoopError> {
        let mtm = MainThreadMarker::new()
            .expect("On iOS, `EventLoop` must be created on the main thread");

        static mut SINGLETON_INIT: bool = false;
        unsafe {
            assert!(
                !SINGLETON_INIT,
                "Only one `EventLoop` is supported on iOS. `EventLoopProxy` might be helpful"
            );
            SINGLETON_INIT = true;
        }

        // this line sets up the main run loop before `UIApplicationMain`
        setup_control_flow_observers();

        let center = unsafe { NSNotificationCenter::defaultCenter() };

        let _did_finish_launching_observer = create_observer(
            &center,
            // `application:didFinishLaunchingWithOptions:`
            unsafe { UIApplicationDidFinishLaunchingNotification },
            move |_| {
                app_state::did_finish_launching(mtm);
            },
        );
        let _did_become_active_observer = create_observer(
            &center,
            // `applicationDidBecomeActive:`
            unsafe { UIApplicationDidBecomeActiveNotification },
            move |_| {
                app_state::handle_nonuser_event(mtm, EventWrapper::StaticEvent(Event::Resumed));
            },
        );
        let _will_resign_active_observer = create_observer(
            &center,
            // `applicationWillResignActive:`
            unsafe { UIApplicationWillResignActiveNotification },
            move |_| {
                app_state::handle_nonuser_event(mtm, EventWrapper::StaticEvent(Event::Suspended));
            },
        );
        let _will_enter_foreground_observer = create_observer(
            &center,
            // `applicationWillEnterForeground:`
            unsafe { UIApplicationWillEnterForegroundNotification },
            move |notification| {
                let app = unsafe { notification.object() }.expect(
                    "UIApplicationWillEnterForegroundNotification to have application object",
                );
                // SAFETY: The `object` in `UIApplicationWillEnterForegroundNotification` is
                // documented to be `UIApplication`.
                let app: Retained<UIApplication> = unsafe { Retained::cast(app) };
                send_occluded_event_for_all_windows(&app, false);
            },
        );
        let _did_enter_background_observer = create_observer(
            &center,
            // `applicationDidEnterBackground:`
            unsafe { UIApplicationDidEnterBackgroundNotification },
            move |notification| {
                let app = unsafe { notification.object() }.expect(
                    "UIApplicationDidEnterBackgroundNotification to have application object",
                );
                // SAFETY: The `object` in `UIApplicationDidEnterBackgroundNotification` is
                // documented to be `UIApplication`.
                let app: Retained<UIApplication> = unsafe { Retained::cast(app) };
                send_occluded_event_for_all_windows(&app, true);
            },
        );
        let _will_terminate_observer = create_observer(
            &center,
            // `applicationWillTerminate:`
            unsafe { UIApplicationWillTerminateNotification },
            move |notification| {
                let app = unsafe { notification.object() }
                    .expect("UIApplicationWillTerminateNotification to have application object");
                // SAFETY: The `object` in `UIApplicationWillTerminateNotification` is
                // (somewhat) documented to be `UIApplication`.
                let app: Retained<UIApplication> = unsafe { Retained::cast(app) };
                app_state::terminated(&app);
            },
        );
        let _did_receive_memory_warning_observer = create_observer(
            &center,
            // `applicationDidReceiveMemoryWarning:`
            unsafe { UIApplicationDidReceiveMemoryWarningNotification },
            move |_| {
                app_state::handle_nonuser_event(
                    mtm,
                    EventWrapper::StaticEvent(Event::MemoryWarning),
                );
            },
        );

        Ok(EventLoop {
            mtm,
            window_target: ActiveEventLoop { mtm },
            _did_finish_launching_observer,
            _did_become_active_observer,
            _will_resign_active_observer,
            _will_enter_foreground_observer,
            _did_enter_background_observer,
            _will_terminate_observer,
            _did_receive_memory_warning_observer,
        })
    }

    pub fn run_app<A: ApplicationHandler>(self, mut app: A) -> ! {
        let application: Option<Retained<UIApplication>> =
            unsafe { msg_send_id![UIApplication::class(), sharedApplication] };
        assert!(
            application.is_none(),
            "\
                `EventLoop` cannot be `run` after a call to `UIApplicationMain` on iOS\nNote: \
             `EventLoop::run_app` calls `UIApplicationMain` on iOS",
        );

        extern "C" {
            // These functions are in crt_externs.h.
            fn _NSGetArgc() -> *mut c_int;
            fn _NSGetArgv() -> *mut *mut *mut c_char;
        }

        app_state::launch(self.mtm, &mut app, || unsafe {
            UIApplicationMain(
                *_NSGetArgc(),
                NonNull::new(*_NSGetArgv()).unwrap(),
                // We intentionally override neither the application nor the delegate, to allow
                // the user to do so themselves!
                None,
                None,
            );
        });

        unreachable!()
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.window_target
    }
}

pub struct EventLoopProxy {
    pub(crate) wake_up: AtomicBool,
    source: CFRunLoopSourceRef,
}

unsafe impl Send for EventLoopProxy {}
unsafe impl Sync for EventLoopProxy {}

impl Drop for EventLoopProxy {
    fn drop(&mut self) {
        unsafe {
            CFRunLoopSourceInvalidate(self.source);
            CFRelease(self.source as _);
        }
    }
}

impl EventLoopProxy {
    pub(crate) fn new() -> EventLoopProxy {
        unsafe {
            // just wake up the eventloop
            extern "C" fn event_loop_proxy_handler(_: *const c_void) {}

            // adding a Source to the main CFRunLoop lets us wake it up and
            // process user events through the normal OS EventLoop mechanisms.
            let rl = CFRunLoopGetMain();
            let mut context = CFRunLoopSourceContext {
                version: 0,
                info: ptr::null_mut(),
                retain: None,
                release: None,
                copyDescription: None,
                equal: None,
                hash: None,
                schedule: None,
                cancel: None,
                perform: event_loop_proxy_handler,
            };
            let source = CFRunLoopSourceCreate(ptr::null_mut(), CFIndex::MAX - 1, &mut context);
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CFRunLoopWakeUp(rl);

            EventLoopProxy { wake_up: AtomicBool::new(false), source }
        }
    }
}

impl EventLoopProxyProvider for EventLoopProxy {
    fn wake_up(&self) {
        self.wake_up.store(true, AtomicOrdering::Relaxed);
        unsafe {
            // let the main thread know there's a new event
            CFRunLoopSourceSignal(self.source);
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
    }
}

fn setup_control_flow_observers() {
    unsafe {
        // begin is queued with the highest priority to ensure it is processed before other
        // observers
        extern "C" fn control_flow_begin_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            let mtm = MainThreadMarker::new().unwrap();
            #[allow(non_upper_case_globals)]
            match activity {
                kCFRunLoopAfterWaiting => app_state::handle_wakeup_transition(mtm),
                _ => unreachable!(),
            }
        }

        // Core Animation registers its `CFRunLoopObserver` that performs drawing operations in
        // `CA::Transaction::ensure_implicit` with a priority of `0x1e8480`. We set the main_end
        // priority to be 0, in order to send AboutToWait before RedrawRequested. This value was
        // chosen conservatively to guard against apple using different priorities for their redraw
        // observers in different OS's or on different devices. If it so happens that it's too
        // conservative, the main symptom would be non-redraw events coming in after `AboutToWait`.
        //
        // The value of `0x1e8480` was determined by inspecting stack traces and the associated
        // registers for every `CFRunLoopAddObserver` call on an iPad Air 2 running iOS 11.4.
        //
        // Also tested to be `0x1e8480` on iPhone 8, iOS 13 beta 4.
        extern "C" fn control_flow_main_end_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            let mtm = MainThreadMarker::new().unwrap();
            #[allow(non_upper_case_globals)]
            match activity {
                kCFRunLoopBeforeWaiting => app_state::handle_main_events_cleared(mtm),
                kCFRunLoopExit => {}, // may happen when running on macOS
                _ => unreachable!(),
            }
        }

        // end is queued with the lowest priority to ensure it is processed after other observers
        extern "C" fn control_flow_end_handler(
            _: CFRunLoopObserverRef,
            activity: CFRunLoopActivity,
            _: *mut c_void,
        ) {
            let mtm = MainThreadMarker::new().unwrap();
            #[allow(non_upper_case_globals)]
            match activity {
                kCFRunLoopBeforeWaiting => app_state::handle_events_cleared(mtm),
                kCFRunLoopExit => {}, // may happen when running on macOS
                _ => unreachable!(),
            }
        }

        let main_loop = CFRunLoopGetMain();

        let begin_observer = CFRunLoopObserverCreate(
            ptr::null_mut(),
            kCFRunLoopAfterWaiting,
            1, // repeat = true
            CFIndex::MIN,
            control_flow_begin_handler,
            ptr::null_mut(),
        );
        CFRunLoopAddObserver(main_loop, begin_observer, kCFRunLoopDefaultMode);

        let main_end_observer = CFRunLoopObserverCreate(
            ptr::null_mut(),
            kCFRunLoopExit | kCFRunLoopBeforeWaiting,
            1, // repeat = true
            0, // see comment on `control_flow_main_end_handler`
            control_flow_main_end_handler,
            ptr::null_mut(),
        );
        CFRunLoopAddObserver(main_loop, main_end_observer, kCFRunLoopDefaultMode);

        let end_observer = CFRunLoopObserverCreate(
            ptr::null_mut(),
            kCFRunLoopExit | kCFRunLoopBeforeWaiting,
            1, // repeat = true
            CFIndex::MAX,
            control_flow_end_handler,
            ptr::null_mut(),
        );
        CFRunLoopAddObserver(main_loop, end_observer, kCFRunLoopDefaultMode);
    }
}
