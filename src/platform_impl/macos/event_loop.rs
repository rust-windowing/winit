use std::{
    any::Any,
    cell::{Cell, RefCell},
    collections::VecDeque,
    marker::PhantomData,
    mem,
    os::raw::c_void,
    panic::{catch_unwind, resume_unwind, AssertUnwindSafe, RefUnwindSafe, UnwindSafe},
    process, ptr,
    rc::{Rc, Weak},
    sync::mpsc,
};

use core_foundation::base::{CFIndex, CFRelease};
use core_foundation::runloop::{
    kCFRunLoopCommonModes, CFRunLoopAddSource, CFRunLoopGetMain, CFRunLoopSourceContext,
    CFRunLoopSourceCreate, CFRunLoopSourceRef, CFRunLoopSourceSignal, CFRunLoopWakeUp,
};
use objc2::foundation::is_main_thread;
use objc2::rc::{autoreleasepool, Id, Shared};
use objc2::{msg_send_id, ClassType};
use raw_window_handle::{AppKitDisplayHandle, RawDisplayHandle};

use super::appkit::{NSApp, NSApplicationActivationPolicy, NSEvent, NSWindow};
use crate::{
    error::RunLoopError,
    event::Event,
    event_loop::{ControlFlow, EventLoopClosed, EventLoopWindowTarget as RootWindowTarget},
    platform::{macos::ActivationPolicy, pump_events::PumpStatus},
    platform_impl::platform::{
        app::WinitApplication,
        app_delegate::ApplicationDelegate,
        app_state::{AppState, Callback},
        monitor::{self, MonitorHandle},
        observer::setup_control_flow_observers,
    },
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

pub struct EventLoopWindowTarget<T: 'static> {
    pub sender: mpsc::Sender<T>, // this is only here to be cloned elsewhere
    pub receiver: mpsc::Receiver<T>,
}

impl<T> Default for EventLoopWindowTarget<T> {
    fn default() -> Self {
        let (sender, receiver) = mpsc::channel();
        EventLoopWindowTarget { sender, receiver }
    }
}

impl<T: 'static> EventLoopWindowTarget<T> {
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
    pub fn raw_display_handle(&self) -> RawDisplayHandle {
        RawDisplayHandle::AppKit(AppKitDisplayHandle::empty())
    }
}

impl<T> EventLoopWindowTarget<T> {
    pub(crate) fn hide_application(&self) {
        NSApp().hide(None)
    }

    pub(crate) fn hide_other_applications(&self) {
        NSApp().hideOtherApplications(None)
    }

    pub(crate) fn set_allows_automatic_window_tabbing(&self, enabled: bool) {
        NSWindow::setAllowsAutomaticWindowTabbing(enabled)
    }

    pub(crate) fn allows_automatic_window_tabbing(&self) -> bool {
        NSWindow::allowsAutomaticWindowTabbing()
    }
}

pub struct EventLoop<T: 'static> {
    /// The delegate is only weakly referenced by NSApplication, so we keep
    /// it around here as well.
    _delegate: Id<ApplicationDelegate, Shared>,

    window_target: Rc<RootWindowTarget<T>>,
    panic_info: Rc<PanicInfo>,

    /// We make sure that the callback closure is dropped during a panic
    /// by making the event loop own it.
    ///
    /// Every other reference should be a Weak reference which is only upgraded
    /// into a strong reference in order to call the callback but then the
    /// strong reference should be dropped as soon as possible.
    _callback: Option<Rc<Callback<T>>>,
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
    pub(crate) fn new(attributes: &PlatformSpecificEventLoopAttributes) -> Self {
        if !is_main_thread() {
            panic!("On macOS, `EventLoop` must be created on the main thread!");
        }

        // This must be done before `NSApp()` (equivalent to sending
        // `sharedApplication`) is called anywhere else, or we'll end up
        // with the wrong `NSApplication` class and the wrong thread could
        // be marked as main.
        let app: Id<WinitApplication, Shared> =
            unsafe { msg_send_id![WinitApplication::class(), sharedApplication] };

        use NSApplicationActivationPolicy::*;
        let activation_policy = match attributes.activation_policy {
            ActivationPolicy::Regular => NSApplicationActivationPolicyRegular,
            ActivationPolicy::Accessory => NSApplicationActivationPolicyAccessory,
            ActivationPolicy::Prohibited => NSApplicationActivationPolicyProhibited,
        };
        let delegate = ApplicationDelegate::new(
            activation_policy,
            attributes.default_menu,
            attributes.activate_ignoring_other_apps,
        );

        autoreleasepool(|_| {
            app.setDelegate(&delegate);
        });

        let panic_info: Rc<PanicInfo> = Default::default();
        setup_control_flow_observers(Rc::downgrade(&panic_info));
        EventLoop {
            _delegate: delegate,
            window_target: Rc::new(RootWindowTarget {
                p: Default::default(),
                _marker: PhantomData,
            }),
            panic_info,
            _callback: None,
        }
    }

    pub fn window_target(&self) -> &RootWindowTarget<T> {
        &self.window_target
    }

    pub fn run<F>(mut self, callback: F) -> !
    where
        F: 'static + FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow),
    {
        let exit_code = match self.run_ondemand(callback) {
            Err(RunLoopError::ExitFailure(code)) => code,
            Err(_err) => 1,
            Ok(_) => 0,
        };
        process::exit(exit_code);
    }

    // NB: we don't base this on `pump_events` because for `MacOs` we can't support
    // `pump_events` elegantly (we just ask to run the loop for a "short" amount of
    // time and so a layered implementation would end up using a lot of CPU due to
    // redundant wake ups.
    pub fn run_ondemand<F>(&mut self, callback: F) -> Result<(), RunLoopError>
    where
        F: FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow),
    {
        if AppState::is_running() {
            return Err(RunLoopError::AlreadyRunning);
        }

        // # Safety
        // We are erasing the lifetime of the application callback here so that we
        // can (temporarily) store it within 'static global `AppState` that's
        // accessible to objc delegate callbacks.
        //
        // The safety of this depends on on making sure to also clear the callback
        // from the global `AppState` before we return from here, ensuring that
        // we don't retain a reference beyond the real lifetime of the callback.

        let callback = unsafe {
            mem::transmute::<
                Rc<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>,
                Rc<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>,
            >(Rc::new(RefCell::new(callback)))
        };

        self._callback = Some(Rc::clone(&callback));

        let exit_code = autoreleasepool(|_| {
            let app = NSApp();

            // A bit of juggling with the callback references to make sure
            // that `self.callback` is the only owner of the callback.
            let weak_cb: Weak<_> = Rc::downgrade(&callback);
            drop(callback);

            // # Safety
            // We make sure to call `AppState::clear_callback` before returning
            unsafe {
                AppState::set_callback(weak_cb, Rc::clone(&self.window_target));
            }

            // catch panics to make sure we can't unwind without clearing the set callback
            // (which would leave the global `AppState` in an undefined, unsafe state)
            let catch_result = catch_unwind(AssertUnwindSafe(|| {
                if AppState::is_launched() {
                    debug_assert!(!AppState::is_running());
                    AppState::start_running(); // Set is_running = true + dispatch `NewEvents(Init)` + `Resumed`
                }
                AppState::set_stop_app_before_wait(false);
                unsafe { app.run() };

                // While the app is running it's possible that we catch a panic
                // to avoid unwinding across an objective-c ffi boundary, which
                // will lead to us stopping the `NSApp` and saving the
                // `PanicInfo` so that we can resume the unwind at a controlled,
                // safe point in time.
                if let Some(panic) = self.panic_info.take() {
                    resume_unwind(panic);
                }

                AppState::exit()
            }));

            // # Safety
            // This pairs up with the `unsafe` call to `set_callback` above and ensures that
            // we always clear the application callback from the global `AppState` before
            // returning
            drop(self._callback.take());
            AppState::clear_callback();

            match catch_result {
                Ok(exit_code) => exit_code,
                Err(payload) => resume_unwind(payload),
            }
        });

        if exit_code == 0 {
            Ok(())
        } else {
            Err(RunLoopError::ExitFailure(exit_code))
        }
    }

    pub fn pump_events<F>(&mut self, callback: F) -> PumpStatus
    where
        F: FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow),
    {
        // # Safety
        // We are erasing the lifetime of the application callback here so that we
        // can (temporarily) store it within 'static global `AppState` that's
        // accessible to objc delegate callbacks.
        //
        // The safety of this depends on on making sure to also clear the callback
        // from the global `AppState` before we return from here, ensuring that
        // we don't retain a reference beyond the real lifetime of the callback.

        let callback = unsafe {
            mem::transmute::<
                Rc<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>,
                Rc<RefCell<dyn FnMut(Event<'_, T>, &RootWindowTarget<T>, &mut ControlFlow)>>,
            >(Rc::new(RefCell::new(callback)))
        };

        self._callback = Some(Rc::clone(&callback));

        autoreleasepool(|_| {
            let app = NSApp();

            // A bit of juggling with the callback references to make sure
            // that `self.callback` is the only owner of the callback.
            let weak_cb: Weak<_> = Rc::downgrade(&callback);
            drop(callback);

            // # Safety
            // We will make sure to call `AppState::clear_callback` before returning
            // to ensure that we don't hold on to the callback beyond its (erased)
            // lifetime
            unsafe {
                AppState::set_callback(weak_cb, Rc::clone(&self.window_target));
            }

            // catch panics to make sure we can't unwind without clearing the set callback
            // (which would leave the global `AppState` in an undefined, unsafe state)
            let catch_result = catch_unwind(AssertUnwindSafe(|| {
                // As a special case, if the `NSApp` hasn't been launched yet then we at least run
                // the loop until it has fully launched.
                if !AppState::is_launched() {
                    debug_assert!(!AppState::is_running());

                    AppState::request_stop_on_launch();
                    unsafe {
                        app.run();
                    }

                    // Note: we dispatch `NewEvents(Init)` + `Resumed` events after the `NSApp` has launched
                } else if !AppState::is_running() {
                    // Even though the NSApp may have been launched, it's possible we aren't running
                    // if the `EventLoop` was run before and has since exited. This indicates that
                    // we just starting to re-run the same `EventLoop` again.
                    AppState::start_running(); // Set is_running = true + dispatch `NewEvents(Init)` + `Resumed`
                } else {
                    // Make sure we can't block any external loop indefinitely by stopping the NSApp
                    // and returning after dispatching any `RedrawRequested` event or whenever the
                    // `RunLoop` needs to wait for new events from the OS
                    AppState::set_stop_app_on_redraw_requested(true);
                    AppState::set_stop_app_before_wait(true);
                    unsafe {
                        app.run();
                    }
                }

                // While the app is running it's possible that we catch a panic
                // to avoid unwinding across an objective-c ffi boundary, which
                // will lead to us stopping the `NSApp` and saving the
                // `PanicInfo` so that we can resume the unwind at a controlled,
                // safe point in time.
                if let Some(panic) = self.panic_info.take() {
                    resume_unwind(panic);
                }

                if let ControlFlow::ExitWithCode(code) = AppState::control_flow() {
                    AppState::exit();
                    PumpStatus::Exit(code)
                } else {
                    PumpStatus::Continue
                }
            }));

            // # Safety
            // This pairs up with the `unsafe` call to `set_callback` above and ensures that
            // we always clear the application callback from the global `AppState` before
            // returning
            AppState::clear_callback();
            drop(self._callback.take());

            match catch_result {
                Ok(pump_status) => pump_status,
                Err(payload) => resume_unwind(payload),
            }
        })
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy::new(self.window_target.p.sender.clone())
    }
}

/// Catches panics that happen inside `f` and when a panic
/// happens, stops the `sharedApplication`
#[inline]
pub fn stop_app_on_panic<F: FnOnce() -> R + UnwindSafe, R>(
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
            let app = NSApp();
            app.stop(None);
            // Posting a dummy event to get `stop` to take effect immediately.
            // See: https://stackoverflow.com/questions/48041279/stopping-the-nsapplication-main-event-loop/48064752#48064752
            app.postEvent_atStart(&NSEvent::dummy(), true);
            None
        }
    }
}

pub struct EventLoopProxy<T> {
    sender: mpsc::Sender<T>,
    source: CFRunLoopSourceRef,
}

unsafe impl<T: Send> Send for EventLoopProxy<T> {}

impl<T> Drop for EventLoopProxy<T> {
    fn drop(&mut self) {
        unsafe {
            CFRelease(self.source as _);
        }
    }
}

impl<T> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy::new(self.sender.clone())
    }
}

impl<T> EventLoopProxy<T> {
    fn new(sender: mpsc::Sender<T>) -> Self {
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
            let source =
                CFRunLoopSourceCreate(ptr::null_mut(), CFIndex::max_value() - 1, &mut context);
            CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
            CFRunLoopWakeUp(rl);

            EventLoopProxy { sender, source }
        }
    }

    pub fn send_event(&self, event: T) -> Result<(), EventLoopClosed<T>> {
        self.sender
            .send(event)
            .map_err(|mpsc::SendError(x)| EventLoopClosed(x))?;
        unsafe {
            // let the main thread know there's a new event
            CFRunLoopSourceSignal(self.source);
            let rl = CFRunLoopGetMain();
            CFRunLoopWakeUp(rl);
        }
        Ok(())
    }
}
