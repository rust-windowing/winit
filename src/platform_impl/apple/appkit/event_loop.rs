use std::any::Any;
use std::cell::Cell;
use std::os::raw::c_void;
use std::panic::{catch_unwind, resume_unwind, RefUnwindSafe, UnwindSafe};
use std::ptr;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use core_foundation::base::{CFIndex, CFRelease};
use core_foundation::runloop::{
    kCFRunLoopCommonModes, CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopSourceContext,
    CFRunLoopSourceCreate, CFRunLoopSourceRef, CFRunLoopSourceSignal, CFRunLoopWakeUp,
};
use objc2::rc::{autoreleasepool, Retained};
use objc2::{msg_send_id, sel, ClassType};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDidFinishLaunchingNotification,
    NSApplicationWillTerminateNotification, NSWindow,
};
use objc2_foundation::{MainThreadMarker, NSNotificationCenter, NSObject, NSObjectProtocol};

use super::super::notification_center::create_observer;
use super::app::WinitApplication;
use super::app_state::AppState;
use super::cursor::CustomCursor;
use super::event::dummy_event;
use super::monitor;
use super::observer::setup_control_flow_observers;
use crate::application::ApplicationHandler;
use crate::error::{EventLoopError, ExternalError};
use crate::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as RootEventLoopProxy, OwnedDisplayHandle as RootOwnedDisplayHandle,
};
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::platform::macos::ActivationPolicy;
use crate::platform::pump_events::PumpStatus;
use crate::platform_impl::Window;
use crate::window::{CustomCursor as RootCustomCursor, CustomCursorSource, Theme};

#[derive(Default)]
pub struct PanicInfo {
    inner: Cell<Option<Box<dyn Any + Send + 'static>>>,
}

// WARNING:
// As long as this struct is used through its `impl`, it is UnwindSafe.
// (If `get_mut` is called on `inner`, unwind safety may get broken.)
impl UnwindSafe for PanicInfo {}
impl RefUnwindSafe for PanicInfo {}
impl PanicInfo {
    pub fn is_panicking(&self) -> bool {
        let inner = self.inner.take();
        let result = inner.is_some();
        self.inner.set(inner);
        result
    }

    /// Overwrites the current state if the current state is not panicking
    pub fn set_panic(&self, p: Box<dyn Any + Send + 'static>) {
        if !self.is_panicking() {
            self.inner.set(Some(p));
        }
    }

    pub fn take(&self) -> Option<Box<dyn Any + Send + 'static>> {
        self.inner.take()
    }
}

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
    fn create_proxy(&self) -> RootEventLoopProxy {
        let event_loop_proxy = EventLoopProxy::new(self.app_state.proxy_wake_up());
        RootEventLoopProxy { event_loop_proxy }
    }

    fn create_window(
        &self,
        window_attributes: crate::window::WindowAttributes,
    ) -> Result<Box<dyn crate::window::Window>, crate::error::OsError> {
        Ok(Box::new(Window::new(self, window_attributes)?))
    }

    fn create_custom_cursor(
        &self,
        source: CustomCursorSource,
    ) -> Result<RootCustomCursor, ExternalError> {
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

        if app.respondsToSelector(sel!(effectiveAppearance)) {
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

    fn owned_display_handle(&self) -> RootOwnedDisplayHandle {
        RootOwnedDisplayHandle { platform: OwnedDisplayHandle }
    }

    #[cfg(feature = "rwh_06")]
    fn rwh_06_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
}

#[cfg(feature = "rwh_06")]
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

    window_target: ActiveEventLoop,
    panic_info: Rc<PanicInfo>,

    // Since macOS 10.11, we no longer need to remove the observers before they are deallocated;
    // the system instead cleans it up next time it would have posted a notification to it.
    //
    // Though we do still need to keep the observers around to prevent them from being deallocated.
    _did_finish_launching_observer: Retained<NSObject>,
    _will_terminate_observer: Retained<NSObject>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) activation_policy: ActivationPolicy,
    pub(crate) default_menu: bool,
    pub(crate) activate_ignoring_other_apps: bool,
}

impl Default for PlatformSpecificEventLoopAttributes {
    fn default() -> Self {
        Self {
            activation_policy: Default::default(), // Regular
            default_menu: true,
            activate_ignoring_other_apps: true,
        }
    }
}

impl EventLoop {
    pub(crate) fn new(
        attributes: &PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopError> {
        let mtm = MainThreadMarker::new()
            .expect("on macOS, `EventLoop` must be created on the main thread!");

        let app: Retained<NSApplication> =
            unsafe { msg_send_id![WinitApplication::class(), sharedApplication] };

        if !app.is_kind_of::<WinitApplication>() {
            panic!(
                "`winit` requires control over the principal class. You must create the event \
                 loop before other parts of your application initialize NSApplication"
            );
        }

        let activation_policy = match attributes.activation_policy {
            ActivationPolicy::Regular => NSApplicationActivationPolicy::Regular,
            ActivationPolicy::Accessory => NSApplicationActivationPolicy::Accessory,
            ActivationPolicy::Prohibited => NSApplicationActivationPolicy::Prohibited,
        };

        let app_state = AppState::setup_global(
            mtm,
            activation_policy,
            attributes.default_menu,
            attributes.activate_ignoring_other_apps,
        );

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

        let panic_info: Rc<PanicInfo> = Default::default();
        setup_control_flow_observers(mtm, Rc::downgrade(&panic_info));

        Ok(EventLoop {
            app,
            app_state: app_state.clone(),
            window_target: ActiveEventLoop { app_state, mtm },
            panic_info,
            _did_finish_launching_observer,
            _will_terminate_observer,
        })
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.window_target
    }

    pub fn run<A: ApplicationHandler>(
        mut self,
        init_closure: impl FnOnce(&dyn RootActiveEventLoop) -> A,
    ) -> Result<(), EventLoopError> {
        self.run_on_demand(init_closure)
    }

    // NB: we don't base this on `pump_events` because for `MacOs` we can't support
    // `pump_events` elegantly (we just ask to run the loop for a "short" amount of
    // time and so a layered implementation would end up using a lot of CPU due to
    // redundant wake ups.
    pub fn run_on_demand<A: ApplicationHandler>(
        &mut self,
        init_closure: impl FnOnce(&dyn RootActiveEventLoop) -> A,
    ) -> Result<(), EventLoopError> {
        self.app_state.clear_exit();
        self.app_state.set_init_closure(init_closure, || {
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

                // SAFETY: We do not run the application re-entrantly
                unsafe { self.app.run() };

                // While the app is running it's possible that we catch a panic
                // to avoid unwinding across an objective-c ffi boundary, which
                // will lead to us stopping the `NSApplication` and saving the
                // `PanicInfo` so that we can resume the unwind at a controlled,
                // safe point in time.
                if let Some(panic) = self.panic_info.take() {
                    resume_unwind(panic);
                }

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
                // As a special case, if the application hasn't been launched yet then we at least
                // run the loop until it has fully launched.
                if !self.app_state.is_launched() {
                    debug_assert!(!self.app_state.is_running());

                    self.app_state.set_stop_on_launch();
                    // SAFETY: We do not run the application re-entrantly
                    unsafe { self.app.run() };

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
                    // SAFETY: We do not run the application re-entrantly
                    unsafe { self.app.run() };
                }

                // While the app is running it's possible that we catch a panic
                // to avoid unwinding across an objective-c ffi boundary, which
                // will lead to us stopping the application and saving the
                // `PanicInfo` so that we can resume the unwind at a controlled,
                // safe point in time.
                if let Some(panic) = self.panic_info.take() {
                    resume_unwind(panic);
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

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct OwnedDisplayHandle;

impl OwnedDisplayHandle {
    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::AppKitDisplayHandle::new().into())
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

/// Catches panics that happen inside `f` and when a panic
/// happens, stops the `sharedApplication`
#[inline]
pub fn stop_app_on_panic<F: FnOnce() -> R + UnwindSafe, R>(
    mtm: MainThreadMarker,
    panic_info: Weak<PanicInfo>,
    f: F,
) -> Option<R> {
    match catch_unwind(f) {
        Ok(r) => Some(r),
        Err(e) => {
            // It's important that we set the panic before requesting a `stop`
            // because some callback are still called during the `stop` message
            // and we need to know in those callbacks if the application is currently
            // panicking
            {
                let panic_info = panic_info.upgrade().unwrap();
                panic_info.set_panic(e);
            }
            let app = NSApplication::sharedApplication(mtm);
            stop_app_immediately(&app);
            None
        },
    }
}

pub struct EventLoopProxy {
    proxy_wake_up: Arc<AtomicBool>,
    source: CFRunLoopSourceRef,
}

unsafe impl Send for EventLoopProxy {}
unsafe impl Sync for EventLoopProxy {}

impl Drop for EventLoopProxy {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.source as _);
        }
    }
}

impl Clone for EventLoopProxy {
    fn clone(&self) -> Self {
        EventLoopProxy::new(self.proxy_wake_up.clone())
    }
}

impl EventLoopProxy {
    fn new(proxy_wake_up: Arc<AtomicBool>) -> Self {
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

            EventLoopProxy { proxy_wake_up, source }
        }
    }

    pub fn wake_up(&self) {
        self.proxy_wake_up.store(true, AtomicOrdering::Relaxed);
        unsafe {
            // let the main thread know there's a new event
            CFRunLoopSourceSignal(self.source);
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
    }
}
