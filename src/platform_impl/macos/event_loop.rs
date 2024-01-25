use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::VecDeque,
    marker::PhantomData,
    mem,
    os::raw::c_void,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe, RefUnwindSafe, UnwindSafe},
    ptr,
    rc::{Rc, Weak},
    sync::mpsc,
    task::{RawWaker, RawWakerVTable, Waker},
    time::{Duration, Instant},
};

use core_foundation::base::{CFIndex, CFRelease};
use core_foundation::runloop::{
    kCFRunLoopCommonModes, CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopSourceContext,
    CFRunLoopSourceCreate, CFRunLoopSourceInvalidate, CFRunLoopSourceRef, CFRunLoopSourceSignal,
    CFRunLoopWakeUp,
};
use icrate::AppKit::{
    NSApplication, NSApplicationActivationPolicyAccessory, NSApplicationActivationPolicyProhibited,
    NSApplicationActivationPolicyRegular, NSWindow,
};
use icrate::Foundation::{MainThreadMarker, NSObjectProtocol};
use objc2::{msg_send_id, ClassType};
use objc2::{
    rc::{autoreleasepool, Id},
    runtime::ProtocolObject,
};

use super::event::dummy_event;
use super::{
    app::WinitApplication,
    app_delegate::{ApplicationDelegate, HandlePendingUserEvents},
    monitor::{self, MonitorHandle},
    observer::setup_control_flow_observers,
};
use crate::{
    error::EventLoopError,
    event::Event,
    event_loop::{
        ControlFlow, DeviceEvents, EventLoopClosed, EventLoopWindowTarget as RootWindowTarget,
    },
    platform::{macos::ActivationPolicy, pump_events::PumpStatus},
};

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
    /// Overwrites the curret state if the current state is not panicking
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
pub struct EventLoopWindowTarget {
    delegate: Id<ApplicationDelegate>,
    pub(super) mtm: MainThreadMarker,
}

impl EventLoopWindowTarget {
    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        monitor::available_monitors()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        let monitor = monitor::primary_monitor();
        Some(monitor)
    }

    #[inline]
    pub fn listen_device_events(&self, _allowed: DeviceEvents) {}

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::RawDisplayHandle::AppKit(rwh_05::AppKitDisplayHandle::empty())
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::AppKit(
            rwh_06::AppKitDisplayHandle::new(),
        ))
    }

    pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
        self.delegate.set_control_flow(control_flow)
    }

    pub(crate) fn control_flow(&self) -> ControlFlow {
        self.delegate.control_flow()
    }

    pub(crate) fn exit(&self) {
        self.delegate.exit()
    }

    pub(crate) fn clear_exit(&self) {
        self.delegate.clear_exit()
    }

    pub(crate) fn exiting(&self) -> bool {
        self.delegate.exiting()
    }

    pub(crate) fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle
    }
}

impl EventLoopWindowTarget {
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

#[allow(deprecated)]
fn map_user_event<T: 'static>(
    mut handler: impl FnMut(Event<T>, &RootWindowTarget),
    receiver: Rc<mpsc::Receiver<T>>,
) -> impl FnMut(Event<HandlePendingUserEvents>, &RootWindowTarget) {
    move |event, window_target| match event.map_nonuser_event() {
        Ok(event) => (handler)(event, window_target),
        Err(_) => {
            for event in receiver.try_iter() {
                (handler)(Event::UserEvent(event), window_target);
            }
        }
    }
}

pub struct EventLoop<T: 'static> {
    /// Store a reference to the application for convenience.
    ///
    /// We intentially don't store `WinitApplication` since we want to have
    /// the possiblity of swapping that out at some point.
    app: Id<NSApplication>,
    /// The application delegate that we've registered.
    ///
    /// The delegate is only weakly referenced by NSApplication, so we must
    /// keep it around here as well.
    delegate: Id<ApplicationDelegate>,

    // Event sender and receiver, used for EventLoopProxy.
    sender: mpsc::Sender<T>,
    receiver: Rc<mpsc::Receiver<T>>,

    window_target: Rc<RootWindowTarget>,
    panic_info: Rc<PanicInfo>,

    /// We make sure that the callback closure is dropped during a panic
    /// by making the event loop own it.
    ///
    /// Every other reference should be a Weak reference which is only upgraded
    /// into a strong reference in order to call the callback but then the
    /// strong reference should be dropped as soon as possible.
    #[allow(clippy::type_complexity)]
    _callback: Option<Rc<RefCell<dyn FnMut(Event<HandlePendingUserEvents>, &RootWindowTarget)>>>,
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

impl<T> EventLoop<T> {
    pub(crate) fn new(
        attributes: &PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopError> {
        let mtm = MainThreadMarker::new()
            .expect("on macOS, `EventLoop` must be created on the main thread!");

        let app: Id<NSApplication> =
            unsafe { msg_send_id![WinitApplication::class(), sharedApplication] };

        if !app.is_kind_of::<WinitApplication>() {
            panic!("`winit` requires control over the principal class. You must create the event loop before other parts of your application initialize NSApplication");
        }

        let activation_policy = match attributes.activation_policy {
            ActivationPolicy::Regular => NSApplicationActivationPolicyRegular,
            ActivationPolicy::Accessory => NSApplicationActivationPolicyAccessory,
            ActivationPolicy::Prohibited => NSApplicationActivationPolicyProhibited,
        };
        let delegate = ApplicationDelegate::new(
            mtm,
            activation_policy,
            attributes.default_menu,
            attributes.activate_ignoring_other_apps,
        );

        autoreleasepool(|_| {
            app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        });

        let panic_info: Rc<PanicInfo> = Default::default();
        setup_control_flow_observers(Rc::downgrade(&panic_info));

        let (sender, receiver) = mpsc::channel();
        Ok(EventLoop {
            app,
            delegate: delegate.clone(),
            sender,
            receiver: Rc::new(receiver),
            window_target: Rc::new(RootWindowTarget {
                p: EventLoopWindowTarget { delegate, mtm },
                _marker: PhantomData,
            }),
            panic_info,
            _callback: None,
        })
    }

    pub fn window_target(&self) -> &RootWindowTarget {
        &self.window_target
    }

    pub fn run<F>(mut self, callback: F) -> Result<(), EventLoopError>
    where
        F: FnMut(Event<T>, &RootWindowTarget),
    {
        self.run_on_demand(callback)
    }

    // NB: we don't base this on `pump_events` because for `MacOs` we can't support
    // `pump_events` elegantly (we just ask to run the loop for a "short" amount of
    // time and so a layered implementation would end up using a lot of CPU due to
    // redundant wake ups.
    pub fn run_on_demand<F>(&mut self, callback: F) -> Result<(), EventLoopError>
    where
        F: FnMut(Event<T>, &RootWindowTarget),
    {
        if self.delegate.is_running() {
            return Err(EventLoopError::AlreadyRunning);
        }

        let callback = map_user_event(callback, self.receiver.clone());

        // # Safety
        // We are erasing the lifetime of the application callback here so that we
        // can (temporarily) store it within 'static app delegate that's
        // accessible to objc delegate callbacks.
        //
        // The safety of this depends on on making sure to also clear the callback
        // from the app delegate before we return from here, ensuring that we don't
        // retain a reference beyond the real lifetime of the callback.
        let callback = unsafe {
            mem::transmute::<
                Rc<RefCell<dyn FnMut(Event<HandlePendingUserEvents>, &RootWindowTarget)>>,
                Rc<RefCell<dyn FnMut(Event<HandlePendingUserEvents>, &RootWindowTarget)>>,
            >(Rc::new(RefCell::new(callback)))
        };

        self._callback = Some(Rc::clone(&callback));

        autoreleasepool(|_| {
            // A bit of juggling with the callback references to make sure
            // that `self.callback` is the only owner of the callback.
            let weak_cb: Weak<_> = Rc::downgrade(&callback);
            drop(callback);

            // # Safety
            // We make sure to call `delegate.clear_callback` before returning
            unsafe {
                self.delegate
                    .set_callback(weak_cb, Rc::clone(&self.window_target));
            }

            // catch panics to make sure we can't unwind without clearing the set callback
            // (which would leave the app delegate in an undefined, unsafe state)
            let catch_result = catch_unwind(AssertUnwindSafe(|| {
                // clear / normalize pump_events state
                self.delegate.set_wait_timeout(None);
                self.delegate.set_stop_before_wait(false);
                self.delegate.set_stop_after_wait(false);
                self.delegate.set_stop_on_redraw(false);

                if self.delegate.is_launched() {
                    debug_assert!(!self.delegate.is_running());
                    self.delegate.set_is_running(true);
                    self.delegate.dispatch_init_events();
                }
                unsafe { self.app.run() };

                // While the app is running it's possible that we catch a panic
                // to avoid unwinding across an objective-c ffi boundary, which
                // will lead to us stopping the `NSApplication` and saving the
                // `PanicInfo` so that we can resume the unwind at a controlled,
                // safe point in time.
                if let Some(panic) = self.panic_info.take() {
                    resume_unwind(panic);
                }

                self.delegate.internal_exit()
            }));

            // # Safety
            // This pairs up with the `unsafe` call to `set_callback` above and ensures that
            // we always clear the application callback from the app delegate before returning.
            drop(self._callback.take());
            self.delegate.clear_callback();

            if let Err(payload) = catch_result {
                resume_unwind(payload)
            }
        });

        Ok(())
    }

    pub fn pump_events<F>(&mut self, timeout: Option<Duration>, callback: F) -> PumpStatus
    where
        F: FnMut(Event<T>, &RootWindowTarget),
    {
        let callback = map_user_event(callback, self.receiver.clone());

        // # Safety
        // We are erasing the lifetime of the application callback here so that we
        // can (temporarily) store it within 'static global app delegate that's
        // accessible to objc delegate callbacks.
        //
        // The safety of this depends on on making sure to also clear the callback
        // from the app delegate before we return from here, ensuring that we don't
        // retain a reference beyond the real lifetime of the callback.

        let callback = unsafe {
            mem::transmute::<
                Rc<RefCell<dyn FnMut(Event<HandlePendingUserEvents>, &RootWindowTarget)>>,
                Rc<RefCell<dyn FnMut(Event<HandlePendingUserEvents>, &RootWindowTarget)>>,
            >(Rc::new(RefCell::new(callback)))
        };

        self._callback = Some(Rc::clone(&callback));

        autoreleasepool(|_| {
            // A bit of juggling with the callback references to make sure
            // that `self.callback` is the only owner of the callback.
            let weak_cb: Weak<_> = Rc::downgrade(&callback);
            drop(callback);

            // # Safety
            // We will make sure to call `delegate.clear_callback` before returning
            // to ensure that we don't hold on to the callback beyond its (erased)
            // lifetime
            unsafe {
                self.delegate
                    .set_callback(weak_cb, Rc::clone(&self.window_target));
            }

            // catch panics to make sure we can't unwind without clearing the set callback
            // (which would leave the app delegate in an undefined, unsafe state)
            let catch_result = catch_unwind(AssertUnwindSafe(|| {
                // As a special case, if the application hasn't been launched yet then we at least run
                // the loop until it has fully launched.
                if !self.delegate.is_launched() {
                    debug_assert!(!self.delegate.is_running());

                    self.delegate.set_stop_on_launch();
                    unsafe {
                        self.app.run();
                    }

                    // Note: we dispatch `NewEvents(Init)` + `Resumed` events after the application has launched
                } else if !self.delegate.is_running() {
                    // Even though the application may have been launched, it's possible we aren't running
                    // if the `EventLoop` was run before and has since exited. This indicates that
                    // we just starting to re-run the same `EventLoop` again.
                    self.delegate.set_is_running(true);
                    self.delegate.dispatch_init_events();
                } else {
                    // Only run for as long as the given `Duration` allows so we don't block the external loop.
                    match timeout {
                        Some(Duration::ZERO) => {
                            self.delegate.set_wait_timeout(None);
                            self.delegate.set_stop_before_wait(true);
                        }
                        Some(duration) => {
                            self.delegate.set_stop_before_wait(false);
                            let timeout = Instant::now() + duration;
                            self.delegate.set_wait_timeout(Some(timeout));
                            self.delegate.set_stop_after_wait(true);
                        }
                        None => {
                            self.delegate.set_wait_timeout(None);
                            self.delegate.set_stop_before_wait(false);
                            self.delegate.set_stop_after_wait(true);
                        }
                    }
                    self.delegate.set_stop_on_redraw(true);
                    unsafe {
                        self.app.run();
                    }
                }

                // While the app is running it's possible that we catch a panic
                // to avoid unwinding across an objective-c ffi boundary, which
                // will lead to us stopping the application and saving the
                // `PanicInfo` so that we can resume the unwind at a controlled,
                // safe point in time.
                if let Some(panic) = self.panic_info.take() {
                    resume_unwind(panic);
                }

                if self.delegate.exiting() {
                    self.delegate.internal_exit();
                    PumpStatus::Exit(0)
                } else {
                    PumpStatus::Continue
                }
            }));

            // # Safety
            // This pairs up with the `unsafe` call to `set_callback` above and ensures that
            // we always clear the application callback from the app delegate before returning
            self.delegate.clear_callback();
            drop(self._callback.take());

            match catch_result {
                Ok(pump_status) => pump_status,
                Err(payload) => resume_unwind(payload),
            }
        })
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            sender: self.sender.clone(),
            waker: waker(),
        }
    }
}

#[derive(Clone)]
pub(crate) struct OwnedDisplayHandle;

impl OwnedDisplayHandle {
    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        rwh_05::AppKitDisplayHandle::empty().into()
    }

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
        }
    }
}

pub fn waker() -> Waker {
    fn new_raw_waker() -> RawWaker {
        // just wake up the eventloop
        extern "C" fn event_loop_proxy_handler(_: *const c_void) {}

        // adding a Source to the main CFRunLoop lets us wake it up and
        // process user events through the normal OS EventLoop mechanisms.
        let rl = unsafe { CFRunLoopGetMain() };
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
        let source = unsafe {
            CFRunLoopSourceCreate(ptr::null_mut(), CFIndex::max_value() - 1, &mut context)
        };
        unsafe { CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes) };
        unsafe { CFRunLoopWakeUp(rl) };
        RawWaker::new(
            source as *const (),
            &RawWakerVTable::new(clone_waker, wake, wake_by_ref, drop_waker),
        )
    }

    unsafe fn clone_waker(waker: *const ()) -> RawWaker {
        let _source = waker as CFRunLoopSourceRef;
        new_raw_waker()
    }

    unsafe fn wake(waker: *const ()) {
        unsafe { wake_by_ref(waker) };
        unsafe { drop_waker(waker) };
    }

    unsafe fn wake_by_ref(waker: *const ()) {
        let source = waker as CFRunLoopSourceRef;
        unsafe {
            // let the main thread know there's a new event
            CFRunLoopSourceSignal(source);
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
    }

    unsafe fn drop_waker(waker: *const ()) {
        let source = waker as CFRunLoopSourceRef;
        unsafe { CFRunLoopSourceInvalidate(source) };
        unsafe { CFRelease(source as _) };
    }

    unsafe { Waker::from_raw(new_raw_waker()) }
}

pub struct EventLoopProxy<T> {
    sender: mpsc::Sender<T>,
    waker: Waker,
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            waker: self.waker.clone(),
        }
    }
}

impl<T> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender
            .send(event)
            .map_err(|mpsc::SendError(x)| EventLoopClosed(x))?;
        self.waker.wake_by_ref();
        Ok(())
    }

    pub fn waker(self) -> Waker {
        self.waker
    }
}
