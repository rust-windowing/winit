use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::ProtocolObject;
use objc2::{available, MainThreadMarker};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDidFinishLaunchingNotification,
    NSApplicationWillTerminateNotification, NSWindow,
};
use objc2_core_foundation::{kCFRunLoopCommonModes, CFIndex, CFRunLoopActivity};
use objc2_foundation::{NSNotificationCenter, NSObjectProtocol};
use rwh_06::HasDisplayHandle;
use winit_common::core_foundation::{MainRunLoop, MainRunLoopObserver};
use winit_core::application::ApplicationHandler;
use winit_core::cursor::{CustomCursor as CoreCustomCursor, CustomCursorSource};
use winit_core::error::{EventLoopError, RequestError};
use winit_core::event_loop::pump_events::PumpStatus;
use winit_core::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as CoreEventLoopProxy, OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use winit_core::monitor::MonitorHandle as CoreMonitorHandle;
use winit_core::window::Theme;

use super::app::override_send_event;
use super::app_state::AppState;
use super::cursor::CustomCursor;
use super::event::dummy_event;
use super::monitor;
use super::notification_center::create_observer;
use crate::window::Window;
use crate::ActivationPolicy;

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
        window_attributes: winit_core::window::WindowAttributes,
    ) -> Result<Box<dyn winit_core::window::Window>, RequestError> {
        Ok(Box::new(Window::new(self, window_attributes)?))
    }

    fn create_custom_cursor(
        &self,
        source: CustomCursorSource,
    ) -> Result<CoreCustomCursor, RequestError> {
        Ok(CoreCustomCursor(Arc::new(CustomCursor::new(source)?)))
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(
            monitor::available_monitors()
                .into_iter()
                .map(|monitor| CoreMonitorHandle(Arc::new(monitor))),
        )
    }

    fn primary_monitor(&self) -> Option<winit_core::monitor::MonitorHandle> {
        let monitor = monitor::primary_monitor();
        Some(CoreMonitorHandle(Arc::new(monitor)))
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

#[derive(Debug)]
pub struct EventLoop {
    /// Store a reference to the application for convenience.
    ///
    /// We intentionally don't store `WinitApplication` since we want to have
    /// the possibility of swapping that out at some point.
    app: Retained<NSApplication>,
    app_state: Rc<AppState>,

    window_target: ActiveEventLoop,

    // Since macOS 10.11, we no longer need to remove the observers before they are deallocated;
    // the system instead cleans it up next time it would have posted a notification to it.
    //
    // Though we do still need to keep the observers around to prevent them from being deallocated.
    _did_finish_launching_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,
    _will_terminate_observer: Retained<ProtocolObject<dyn NSObjectProtocol>>,

    _before_waiting_observer: MainRunLoopObserver,
    _after_waiting_observer: MainRunLoopObserver,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct PlatformSpecificEventLoopAttributes {
    pub activation_policy: Option<ActivationPolicy>,
    pub default_menu: bool,
    pub activate_ignoring_other_apps: bool,
}

impl Default for PlatformSpecificEventLoopAttributes {
    fn default() -> Self {
        Self { activation_policy: None, default_menu: true, activate_ignoring_other_apps: true }
    }
}

impl EventLoop {
    pub fn new(attributes: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
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
        )
        .ok_or_else(|| EventLoopError::RecreationAttempt)?;

        // Initialize the application (if it has not already been).
        let app = NSApplication::sharedApplication(mtm);

        // Override `sendEvent:` on the application to forward to our application state.
        override_send_event(&app);

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

        let main_loop = MainRunLoop::get(mtm);
        let mode = unsafe { kCFRunLoopCommonModes }.unwrap();

        let app_state_clone = Rc::clone(&app_state);
        let _before_waiting_observer = MainRunLoopObserver::new(
            mtm,
            CFRunLoopActivity::BeforeWaiting,
            true,
            // Queued with the lowest priority to ensure it is processed after other observers.
            // Without that, we'd get a `LoopExiting` after `AboutToWait`.
            CFIndex::MAX,
            move |_| app_state_clone.cleared(),
        );
        main_loop.add_observer(&_before_waiting_observer, mode);

        let app_state_clone = Rc::clone(&app_state);
        let _after_waiting_observer = MainRunLoopObserver::new(
            mtm,
            CFRunLoopActivity::AfterWaiting,
            true,
            // Queued with the highest priority to ensure it is processed before other observers.
            CFIndex::MIN,
            move |_| app_state_clone.wakeup(),
        );
        main_loop.add_observer(&_after_waiting_observer, mode);

        Ok(EventLoop {
            app,
            app_state: app_state.clone(),
            window_target: ActiveEventLoop { app_state, mtm },
            _did_finish_launching_observer,
            _will_terminate_observer,
            _before_waiting_observer,
            _after_waiting_observer,
        })
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.window_target
    }

    pub fn run_app<A: ApplicationHandler>(mut self, app: A) -> Result<(), EventLoopError> {
        self.run_app_on_demand(app)
    }

    // NB: we don't base this on `pump_events` because for `MacOs` we can't support
    // `pump_events` elegantly (we just ask to run the loop for a "short" amount of
    // time and so a layered implementation would end up using a lot of CPU due to
    // redundant wake ups.
    pub fn run_app_on_demand<A: ApplicationHandler>(
        &mut self,
        app: A,
    ) -> Result<(), EventLoopError> {
        self.app_state.clear_exit();
        self.app_state.set_event_handler(app, || {
            autoreleasepool(|_| {
                // clear / normalize pump_events state
                self.app_state.set_wait_timeout(None);
                self.app_state.set_stop_before_wait(false);
                self.app_state.set_stop_after_wait(false);
                self.app_state.set_stop_on_redraw(false);

                if self.app_state.is_launched() {
                    debug_assert!(!self.app_state.is_running());
                    self.app_state.set_is_running(true);
                    self.app_state.dispatch_init_events();
                }

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
        app: A,
    ) -> PumpStatus {
        self.app_state.set_event_handler(app, || {
            autoreleasepool(|_| {
                // As a special case, if the application hasn't been launched yet then we at least
                // run the loop until it has fully launched.
                if !self.app_state.is_launched() {
                    debug_assert!(!self.app_state.is_running());

                    self.app_state.set_stop_on_launch();
                    self.app.run();

                    // Note: we dispatch `NewEvents(Init)` + `Resumed` events after the application
                    // has launched
                } else if !self.app_state.is_running() {
                    // Even though the application may have been launched, it's possible we aren't
                    // running if the `EventLoop` was run before and has since
                    // exited. This indicates that we just starting to re-run
                    // the same `EventLoop` again.
                    self.app_state.set_is_running(true);
                    self.app_state.dispatch_init_events();
                } else {
                    // Only run for as long as the given `Duration` allows so we don't block the
                    // external loop.
                    match timeout {
                        Some(Duration::ZERO) => {
                            self.app_state.set_wait_timeout(None);
                            self.app_state.set_stop_before_wait(true);
                        },
                        Some(duration) => {
                            self.app_state.set_stop_before_wait(false);
                            let timeout = Instant::now() + duration;
                            self.app_state.set_wait_timeout(Some(timeout));
                            self.app_state.set_stop_after_wait(true);
                        },
                        None => {
                            self.app_state.set_wait_timeout(None);
                            self.app_state.set_stop_before_wait(false);
                            self.app_state.set_stop_after_wait(true);
                        },
                    }
                    self.app_state.set_stop_on_redraw(true);
                    self.app.run();
                }

                if self.app_state.exiting() {
                    self.app_state.internal_exit();
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

/// Tell all windows to close.
///
/// This will synchronously trigger `WindowEvent::Destroyed` within
/// `windowWillClose:`, giving the application one last chance to handle
/// those events. It doesn't matter if the user also ends up closing the
/// windows in `Window`'s `Drop` impl, once a window has been closed once, it
/// stays closed.
///
/// This ensures that no windows linger on after the event loop has exited,
/// see <https://github.com/rust-windowing/winit/issues/4135>.
pub(super) fn notify_windows_of_exit(app: &NSApplication) {
    for window in app.windows() {
        window.close();
    }
}
