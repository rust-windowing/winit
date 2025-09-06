use std::sync::Arc;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{msg_send, ClassType, MainThreadMarker};
use objc2_core_foundation::{kCFRunLoopDefaultMode, CFIndex, CFRunLoopActivity};
use objc2_foundation::{NSNotificationCenter, NSObjectProtocol};
use objc2_ui_kit::{
    UIApplication, UIApplicationDidBecomeActiveNotification,
    UIApplicationDidEnterBackgroundNotification, UIApplicationDidFinishLaunchingNotification,
    UIApplicationDidReceiveMemoryWarningNotification, UIApplicationWillEnterForegroundNotification,
    UIApplicationWillResignActiveNotification, UIApplicationWillTerminateNotification, UIScreen,
};
use rwh_06::HasDisplayHandle;
use winit_common::core_foundation::{MainRunLoop, MainRunLoopObserver};
use winit_core::application::ApplicationHandler;
use winit_core::cursor::{CustomCursor, CustomCursorSource};
use winit_core::error::{EventLoopError, NotSupportedError, RequestError};
use winit_core::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as CoreEventLoopProxy, OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use winit_core::monitor::MonitorHandle as CoreMonitorHandle;
use winit_core::window::{Theme, Window as CoreWindow};

use super::app_state::{send_occluded_event_for_all_windows, AppState};
use super::notification_center::create_observer;
use crate::monitor::MonitorHandle;
use crate::window::Window;
use crate::{app_state, monitor};

#[derive(Debug)]
pub struct ActiveEventLoop {
    pub(super) mtm: MainThreadMarker,
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> CoreEventLoopProxy {
        CoreEventLoopProxy::new(AppState::get(self.mtm).event_loop_proxy().clone())
    }

    fn create_window(
        &self,
        window_attributes: winit_core::window::WindowAttributes,
    ) -> Result<Box<dyn CoreWindow>, RequestError> {
        Ok(Box::new(Window::new(self, window_attributes)?))
    }

    fn create_custom_cursor(
        &self,
        _source: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        Err(NotSupportedError::new("create_custom_cursor is not supported").into())
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(
            monitor::uiscreens(self.mtm)
                .into_iter()
                .map(|monitor| CoreMonitorHandle(Arc::new(monitor))),
        )
    }

    fn primary_monitor(&self) -> Option<winit_core::monitor::MonitorHandle> {
        #[allow(deprecated)]
        let monitor = MonitorHandle::new(UIScreen::mainScreen(self.mtm));
        Some(CoreMonitorHandle(Arc::new(monitor)))
    }

    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn set_control_flow(&self, control_flow: ControlFlow) {
        AppState::get(self.mtm).set_control_flow(control_flow)
    }

    fn system_theme(&self) -> Option<Theme> {
        None
    }

    fn control_flow(&self) -> ControlFlow {
        AppState::get(self.mtm).control_flow()
    }

    fn exit(&self) {
        // https://developer.apple.com/library/archive/qa/qa1561/_index.html
        // it is not possible to quit an iOS app gracefully and programmatically
        tracing::warn!("`ControlFlow::Exit` ignored on iOS");
    }

    fn exiting(&self) -> bool {
        false
    }

    fn owned_display_handle(&self) -> CoreOwnedDisplayHandle {
        CoreOwnedDisplayHandle::new(Arc::new(OwnedDisplayHandle))
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

impl HasDisplayHandle for OwnedDisplayHandle {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::RawDisplayHandle::UiKit(rwh_06::UiKitDisplayHandle::new());
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

#[derive(Debug)]
pub struct EventLoop {
    mtm: MainThreadMarker,
    window_target: ActiveEventLoop,

    // Since iOS 9.0, we no longer need to remove the observers before they are deallocated; the
    // system instead cleans it up next time it would have posted a notification to it.
    //
    // Though we do still need to keep the observers around to prevent them from being deallocated.
    _did_finish_launching_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    _did_become_active_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    _will_resign_active_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    _will_enter_foreground_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    _did_enter_background_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    _will_terminate_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    _did_receive_memory_warning_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,

    _wakeup_observer: MainRunLoopObserver,
    _main_events_cleared_observer: MainRunLoopObserver,
    _events_cleared_observer: MainRunLoopObserver,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PlatformSpecificEventLoopAttributes {}

impl EventLoop {
    pub fn new(_: &PlatformSpecificEventLoopAttributes) -> Result<EventLoop, EventLoopError> {
        let mtm = MainThreadMarker::new()
            .expect("On iOS, `EventLoop` must be created on the main thread");

        if !AppState::setup_global(mtm) {
            // Required, AppState is global state, and event loop can only be run once.
            return Err(EventLoopError::RecreationAttempt);
        }

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
            move |_| app_state::handle_resumed(mtm),
        );
        let _will_resign_active_observer = create_observer(
            &center,
            // `applicationWillResignActive:`
            unsafe { UIApplicationWillResignActiveNotification },
            move |_| app_state::handle_suspended(mtm),
        );
        let _will_enter_foreground_observer = create_observer(
            &center,
            // `applicationWillEnterForeground:`
            unsafe { UIApplicationWillEnterForegroundNotification },
            move |notification| {
                let app = unsafe { notification.object() }.expect(
                    "UIApplicationWillEnterForegroundNotification to have application object",
                );
                // The `object` in `UIApplicationWillEnterForegroundNotification` is documented to
                // be `UIApplication`.
                let app = app.downcast::<UIApplication>().unwrap();
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
                // The `object` in `UIApplicationDidEnterBackgroundNotification` is documented to be
                // `UIApplication`.
                let app = app.downcast::<UIApplication>().unwrap();
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
                // The `object` in `UIApplicationWillTerminateNotification` is (somewhat) documented
                // to be `UIApplication`.
                let app = app.downcast::<UIApplication>().unwrap();
                app_state::terminated(&app);
            },
        );
        let _did_receive_memory_warning_observer = create_observer(
            &center,
            // `applicationDidReceiveMemoryWarning:`
            unsafe { UIApplicationDidReceiveMemoryWarningNotification },
            move |_| app_state::handle_memory_warning(mtm),
        );

        let main_loop = MainRunLoop::get(mtm);
        let mode = unsafe { kCFRunLoopDefaultMode }.unwrap();

        let _wakeup_observer = MainRunLoopObserver::new(
            mtm,
            CFRunLoopActivity::AfterWaiting,
            true,
            // Queued with the highest priority to ensure it is processed before other observers.
            CFIndex::MIN,
            move |_| app_state::handle_wakeup_transition(mtm),
        );
        main_loop.add_observer(&_wakeup_observer, mode);

        let _main_events_cleared_observer = MainRunLoopObserver::new(
            mtm,
            CFRunLoopActivity::BeforeWaiting,
            true,
            // Core Animation registers its `CFRunLoopObserver` that performs drawing operations in
            // `CA::Transaction::ensure_implicit` with a priority of `0x1e8480`. We set the
            // main_end priority to be 0, in order to send `AboutToWait` before `RedrawRequested`.
            // This value was chosen conservatively to guard against apple using different
            // priorities for their redraw observers in different OS's or on different devices. If
            // it so happens that it's too conservative, the main symptom would be non-redraw
            // events coming in after `AboutToWait`.
            //
            // The value of `0x1e8480` was determined by inspecting stack traces and the associated
            // registers for every `CFRunLoopAddObserver` call on an iPad Air 2 running iOS 11.4.
            //
            // Also tested to be `0x1e8480` on iPhone 8, iOS 13 beta 4.
            0,
            move |_| app_state::handle_main_events_cleared(mtm),
        );
        main_loop.add_observer(&_main_events_cleared_observer, mode);

        let _events_cleared_observer = MainRunLoopObserver::new(
            mtm,
            CFRunLoopActivity::BeforeWaiting,
            true,
            // Queued with the lowest priority to ensure it is processed after other observers.
            CFIndex::MAX,
            move |_| app_state::handle_events_cleared(mtm),
        );
        main_loop.add_observer(&_events_cleared_observer, mode);

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
            _wakeup_observer,
            _main_events_cleared_observer,
            _events_cleared_observer,
        })
    }

    pub fn run_app<A: ApplicationHandler>(self, app: A) -> ! {
        let application: Option<Retained<UIApplication>> =
            unsafe { msg_send![UIApplication::class(), sharedApplication] };
        assert!(
            application.is_none(),
            "\
                `EventLoop` cannot be `run` after a call to `UIApplicationMain` on iOS\nNote: \
             `EventLoop::run_app` calls `UIApplicationMain` on iOS",
        );

        // We intentionally override neither the application nor the delegate,
        // to allow the user to do so themselves!
        app_state::launch(self.mtm, app, || UIApplication::main(None, None, self.mtm))
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.window_target
    }
}
