use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::ProtocolObject;
use objc2::{available, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDidFinishLaunchingNotification,
    NSApplicationWillTerminateNotification, NSEventMask, NSWindow,
};
use objc2_foundation::{
    NSDate, NSDefaultRunLoopMode, NSNotificationCenter, NSObjectProtocol, NSTimeInterval,
};
use rwh_06::HasDisplayHandle;

use super::super::notification_center::create_observer;
use super::app::override_send_event;
use super::app_state::AppState;
use super::cursor::CustomCursor;
use super::event::dummy_event;
use super::monitor;
use super::observer::setup_control_flow_observers;
use crate::application::ApplicationHandler;
use crate::error::{EventLoopError, RequestError};
use crate::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as CoreEventLoopProxy, OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::platform::macos::ActivationPolicy;
use crate::platform::pump_events::PumpStatus;
use crate::platform_impl::Window;
use crate::window::{CustomCursor as RootCustomCursor, CustomCursorSource, Theme};

#[derive(Debug)]
pub struct ActiveEventLoop {
    pub(super) app_state: Rc<AppState>,
    pub(super) mtm: MainThreadMarker,
}

impl ActiveEventLoop {
    pub(crate) fn hide_application(&self) {
        NSApplication::sharedApplication(self.mtm).hide(None)
    }

    pub(crate) fn hide_other_applications(&self) {
        NSApplication::sharedApplication(self.mtm).hideOtherApplications(None)
    }

    pub(crate) fn set_allows_automatic_window_tabbing(&self, enabled: bool) {
        NSWindow::setAllowsAutomaticWindowTabbing(enabled, self.mtm)
    }

    pub(crate) fn allows_automatic_window_tabbing(&self) -> bool {
        NSWindow::allowsAutomaticWindowTabbing(self.mtm)
    }
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> CoreEventLoopProxy {
        CoreEventLoopProxy::new(self.app_state.event_loop_proxy().clone())
    }

    fn create_window(
        &self,
        window_attributes: crate::window::WindowAttributes,
    ) -> Result<Box<dyn crate::window::Window>, RequestError> {
        Ok(Box::new(Window::new(self, window_attributes)?))
    }

    fn create_custom_cursor(
        &self,
        source: CustomCursorSource,
    ) -> Result<RootCustomCursor, RequestError> {
        Ok(RootCustomCursor { inner: CustomCursor::new(source.inner)? })
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = RootMonitorHandle>> {
        Box::new(monitor::available_monitors().into_iter().map(|inner| RootMonitorHandle { inner }))
    }

    fn primary_monitor(&self) -> Option<crate::monitor::MonitorHandle> {
        let monitor = monitor::primary_monitor();
        Some(RootMonitorHandle { inner: monitor })
    }

    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn system_theme(&self) -> Option<Theme> {
        let app = NSApplication::sharedApplication(self.mtm);

        // Dark appearance was introduced in macOS 10.14
        if available!(macos = 10.14) {
            Some(super::window_delegate::appearance_to_theme(&app.effectiveAppearance()))
        } else {
            Some(Theme::Light)
        }
    }

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.app_state.set_control_flow(control_flow)
    }

    fn control_flow(&self) -> ControlFlow {
        self.app_state.control_flow()
    }

    fn exit(&self) {
        self.app_state.exit()
    }

    fn exiting(&self) -> bool {
        self.app_state.exiting()
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
        let raw = rwh_06::RawDisplayHandle::AppKit(rwh_06::AppKitDisplayHandle::new());
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

pub struct EventLoop {
    /// Store a reference to the application for convenience.
    ///
    /// We intentionally don't store `WinitApplication` since we want to have
    /// the possibility of swapping that out at some point.
    app: Retained<NSApplication>,
    app_state: Rc<AppState>,

    /// Whether an outer event loop is running.
    pump_has_sent_init: bool,

    window_target: ActiveEventLoop,

    // Since macOS 10.11, we no longer need to remove the observers before they are deallocated;
    // the system instead cleans it up next time it would have posted a notification to it.
    //
    // Though we do still need to keep the observers around to prevent them from being deallocated.
    _did_finish_launching_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    _will_terminate_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) activation_policy: Option<ActivationPolicy>,
    pub(crate) default_menu: bool,
    pub(crate) activate_ignoring_other_apps: bool,
}

impl Default for PlatformSpecificEventLoopAttributes {
    fn default() -> Self {
        Self { activation_policy: None, default_menu: true, activate_ignoring_other_apps: true }
    }
}

impl EventLoop {
    pub(crate) fn new(
        attributes: &PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopError> {
        let mtm = MainThreadMarker::new()
            .expect("on macOS, `EventLoop` must be created on the main thread!");

        let activation_policy = match attributes.activation_policy {
            None => None,
            Some(ActivationPolicy::Regular) => Some(NSApplicationActivationPolicy::Regular),
            Some(ActivationPolicy::Accessory) => Some(NSApplicationActivationPolicy::Accessory),
            Some(ActivationPolicy::Prohibited) => Some(NSApplicationActivationPolicy::Prohibited),
        };

        let app_state = AppState::setup_global(
            mtm,
            activation_policy,
            attributes.default_menu,
            attributes.activate_ignoring_other_apps,
        );

        // Initialize the application (if it has not already been).
        let app = NSApplication::sharedApplication(mtm);

        // Override `sendEvent:` on the application to forward to our application state.
        override_send_event(&app);

        // Queue `NSApplicationDidFinishLaunchingNotification` and generally
        // make sure the application is fully initialized (once the run loop
        // starts).
        //
        // This is technically only necessary when using `pump_app_events`
        // (`app.run()` will do it for us in `run_app_on_demand`), but we
        // might as well do it everywhere.
        unsafe { app.finishLaunching() };

        let center = unsafe { NSNotificationCenter::defaultCenter() };

        let weak_app_state = Rc::downgrade(&app_state);
        let _did_finish_launching_observer = create_observer(
            &center,
            // `applicationDidFinishLaunching:`
            unsafe { NSApplicationDidFinishLaunchingNotification },
            move |notification| {
                if let Some(app_state) = weak_app_state.upgrade() {
                    app_state.did_finish_launching(notification);
                }
            },
        );

        let weak_app_state = Rc::downgrade(&app_state);
        let _will_terminate_observer = create_observer(
            &center,
            // `applicationWillTerminate:`
            unsafe { NSApplicationWillTerminateNotification },
            move |notification| {
                if let Some(app_state) = weak_app_state.upgrade() {
                    app_state.will_terminate(notification);
                }
            },
        );

        setup_control_flow_observers(mtm);

        Ok(EventLoop {
            app,
            app_state: app_state.clone(),
            pump_has_sent_init: false,
            window_target: ActiveEventLoop { app_state, mtm },
            _did_finish_launching_observer,
            _will_terminate_observer,
        })
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.window_target
    }

    pub fn run_app<A: ApplicationHandler>(mut self, app: A) -> Result<(), EventLoopError> {
        self.run_app_on_demand(app)
    }

    pub fn run_app_on_demand<A: ApplicationHandler>(
        &mut self,
        mut app: A,
    ) -> Result<(), EventLoopError> {
        self.app_state.clear_exit();
        self.app_state.set_event_handler(&mut app, || {
            autoreleasepool(|_| {
                if self.app_state.is_launched() {
                    // The `NSApplicationDidFinishLaunchingNotification` notification is globally
                    // only delivered once, but for the purpose of our events, we want to act
                    // as-if an entirely new event loop has been started on each invocation of
                    // `run_app_on_demand`.
                    self.app_state.dispatch_init_events();
                }

                // NOTE: We don't base this on `pump_events` because
                // `nextEventMatchingMask:untilDate:inMode:dequeue:` is worse supported,
                // especially as the top-level handler. In part because this sets the `isRunning`
                // flag (which is used by crates like `rfd`), while `nextEventMatchingMask` won't.
                //
                // NOTE: Make sure to not run the application re-entrantly, as that'd be confusing.
                self.app.run();

                self.app_state.internal_exit()
            })
        });

        Ok(())
    }

    pub fn pump_app_events<A: ApplicationHandler>(
        &mut self,
        timeout: Option<Duration>,
        mut app: A,
    ) -> PumpStatus {
        self.app_state.set_event_handler(&mut app, || {
            autoreleasepool(|_| {
                if self.app_state.is_launched() && !self.pump_has_sent_init {
                    // If the application is already launched, we won't get the re-initialization
                    // events. Dispatch them here instead.
                    self.app_state.dispatch_init_events();
                }
                self.pump_has_sent_init = true;

                // Only run for as long as the given `Duration` allows so we don't block the
                // external loop.
                let expiration_date = match timeout {
                    Some(Duration::ZERO) => unsafe { NSDate::distantPast() },
                    Some(duration) => unsafe {
                        NSDate::dateWithTimeIntervalSinceNow(
                            duration.as_secs_f64() as NSTimeInterval
                        )
                    },
                    None => unsafe { NSDate::distantFuture() },
                };

                // Wait for an event to arrive within the specified duration,
                // and let the application handle it if one did.
                let event = unsafe {
                    self.app.nextEventMatchingMask_untilDate_inMode_dequeue(
                        NSEventMask::Any,
                        Some(&expiration_date),
                        NSDefaultRunLoopMode,
                        true,
                    )
                };
                if let Some(event) = event {
                    unsafe { self.app.sendEvent(&event) };
                }

                if self.app_state.exiting() {
                    self.app_state.internal_exit();
                    // If we start again, we'll emit a new set of initialization events.
                    self.pump_has_sent_init = false;
                    PumpStatus::Exit(0)
                } else {
                    PumpStatus::Continue
                }
            })
        })
    }
}

pub(crate) struct OwnedDisplayHandle;

impl HasDisplayHandle for OwnedDisplayHandle {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::RawDisplayHandle::AppKit(rwh_06::AppKitDisplayHandle::new());
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

pub(super) fn stop_app_immediately(app: &NSApplication) {
    autoreleasepool(|_| {
        app.stop(None);
        // To stop event loop immediately, we need to post some event here.
        // See: https://stackoverflow.com/questions/48041279/stopping-the-nsapplication-main-event-loop/48064752#48064752
        app.postEvent_atStart(&dummy_event().unwrap(), true);
    });
}
