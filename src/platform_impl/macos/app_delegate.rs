use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::mem;
use std::rc::{Rc, Weak};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use icrate::AppKit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use icrate::Foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSSize};
use objc2::rc::Id;
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};

use super::event_loop::{stop_app_immediately, PanicInfo};
use super::observer::{EventLoopWaker, RunLoop};
use super::window::WinitWindow;
use super::{menu, WindowId, DEVICE_ID};
use crate::dpi::PhysicalSize;
use crate::event::{DeviceEvent, Event, InnerSizeWriter, StartCause, WindowEvent};
use crate::event_loop::{ControlFlow, EventLoopWindowTarget as RootWindowTarget};
use crate::window::WindowId as RootWindowId;

#[derive(Debug, Default)]
pub(super) struct State {
    activation_policy: NSApplicationActivationPolicy,
    default_menu: bool,
    activate_ignoring_other_apps: bool,
    /// Whether the application is currently executing a callback.
    in_callback: Cell<bool>,
    /// The lifetime-erased callback.
    callback: RefCell<Option<EventLoopHandler>>,
    stop_on_launch: Cell<bool>,
    stop_before_wait: Cell<bool>,
    stop_after_wait: Cell<bool>,
    stop_on_redraw: Cell<bool>,
    /// Whether `applicationDidFinishLaunching:` has been run or not.
    is_launched: Cell<bool>,
    /// Whether an `EventLoop` is currently running.
    is_running: Cell<bool>,
    /// Whether the user has requested the event loop to exit.
    exit: Cell<bool>,
    control_flow: Cell<ControlFlow>,
    waker: RefCell<EventLoopWaker>,
    start_time: Cell<Option<Instant>>,
    wait_timeout: Cell<Option<Instant>>,
    pending_events: RefCell<VecDeque<QueuedEvent>>,
    pending_redraw: RefCell<Vec<WindowId>>,
}

declare_class!(
    #[derive(Debug)]
    pub(super) struct ApplicationDelegate;

    unsafe impl ClassType for ApplicationDelegate {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    impl DeclaredClass for ApplicationDelegate {
        type Ivars = State;
    }

    unsafe impl NSObjectProtocol for ApplicationDelegate {}

    unsafe impl NSApplicationDelegate for ApplicationDelegate {
        // Note: This will, globally, only be run once, no matter how many
        // `EventLoop`s the user creates.
        #[method(applicationDidFinishLaunching:)]
        fn did_finish_launching(&self, _sender: Option<&AnyObject>) {
            trace_scope!("applicationDidFinishLaunching:");
            self.ivars().is_launched.set(true);

            let mtm = MainThreadMarker::from(self);
            let app = NSApplication::sharedApplication(mtm);
            // We need to delay setting the activation policy and activating the app
            // until `applicationDidFinishLaunching` has been called. Otherwise the
            // menu bar is initially unresponsive on macOS 10.15.
            app.setActivationPolicy(self.ivars().activation_policy);

            window_activation_hack(&app);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(self.ivars().activate_ignoring_other_apps);

            if self.ivars().default_menu {
                // The menubar initialization should be before the `NewEvents` event, to allow
                // overriding of the default menu even if it's created
                menu::initialize(&app);
            }

            self.ivars().waker.borrow_mut().start();

            self.set_is_running(true);
            self.dispatch_init_events();

            // If the application is being launched via `EventLoop::pump_events()` then we'll
            // want to stop the app once it is launched (and return to the external loop)
            //
            // In this case we still want to consider Winit's `EventLoop` to be "running",
            // so we call `start_running()` above.
            if self.ivars().stop_on_launch.get() {
                // Note: the original idea had been to only stop the underlying `RunLoop`
                // for the app but that didn't work as expected (`-[NSApplication run]`
                // effectively ignored the attempt to stop the RunLoop and re-started it).
                //
                // So we return from `pump_events` by stopping the application.
                let app = NSApplication::sharedApplication(mtm);
                stop_app_immediately(&app);
            }
        }

        #[method(applicationWillTerminate:)]
        fn will_terminate(&self, _sender: Option<&AnyObject>) {
            trace_scope!("applicationWillTerminate:");
            // TODO: Notify every window that it will be destroyed, like done in iOS?
            self.internal_exit();
        }
    }
);

impl ApplicationDelegate {
    pub(super) fn new(
        mtm: MainThreadMarker,
        activation_policy: NSApplicationActivationPolicy,
        default_menu: bool,
        activate_ignoring_other_apps: bool,
    ) -> Id<Self> {
        let this = mtm.alloc().set_ivars(State {
            activation_policy,
            default_menu,
            activate_ignoring_other_apps,
            ..Default::default()
        });
        unsafe { msg_send_id![super(this), init] }
    }

    pub fn get(mtm: MainThreadMarker) -> Id<Self> {
        let app = NSApplication::sharedApplication(mtm);
        let delegate =
            unsafe { app.delegate() }.expect("a delegate was not configured on the application");
        if delegate.is_kind_of::<Self>() {
            // SAFETY: Just checked that the delegate is an instance of `ApplicationDelegate`
            unsafe { Id::cast(delegate) }
        } else {
            panic!("tried to get a delegate that was not the one Winit has registered")
        }
    }

    /// Associate the application's event callback with the application delegate.
    ///
    /// # Safety
    /// This is ignoring the lifetime of the application callback (which may not be 'static)
    /// and can lead to undefined behaviour if the callback is not cleared before the end of
    /// its real lifetime.
    ///
    /// All public APIs that take an event callback (`run`, `run_on_demand`,
    /// `pump_events`) _must_ pair a call to `set_callback` with
    /// a call to `clear_callback` before returning to avoid undefined behaviour.
    #[allow(clippy::type_complexity)]
    pub unsafe fn set_callback(
        &self,
        callback: Weak<RefCell<dyn FnMut(Event<HandlePendingUserEvents>, &RootWindowTarget)>>,
        window_target: Rc<RootWindowTarget>,
    ) {
        *self.ivars().callback.borrow_mut() = Some(EventLoopHandler {
            callback,
            window_target,
        });
    }

    pub fn clear_callback(&self) {
        self.ivars().callback.borrow_mut().take();
    }

    fn have_callback(&self) -> bool {
        self.ivars().callback.borrow().is_some()
    }

    /// If `pump_events` is called to progress the event loop then we
    /// bootstrap the event loop via `-[NSAppplication run]` but will use
    /// `CFRunLoopRunInMode` for subsequent calls to `pump_events`.
    pub fn set_stop_on_launch(&self) {
        self.ivars().stop_on_launch.set(true);
    }

    pub fn set_stop_before_wait(&self, value: bool) {
        self.ivars().stop_before_wait.set(value)
    }

    pub fn set_stop_after_wait(&self, value: bool) {
        self.ivars().stop_after_wait.set(value)
    }

    pub fn set_stop_on_redraw(&self, value: bool) {
        self.ivars().stop_on_redraw.set(value)
    }

    pub fn set_wait_timeout(&self, value: Option<Instant>) {
        self.ivars().wait_timeout.set(value)
    }

    /// Clears the `running` state and resets the `control_flow` state when an `EventLoop` exits.
    ///
    /// Note: that if the `NSApplication` has been launched then that state is preserved,
    /// and we won't need to re-launch the app if subsequent EventLoops are run.
    pub fn internal_exit(&self) {
        self.set_in_callback(true);
        self.handle_event(Event::LoopExiting);
        self.set_in_callback(false);

        self.set_is_running(false);
        self.set_stop_on_redraw(false);
        self.set_stop_before_wait(false);
        self.set_stop_after_wait(false);
        self.set_wait_timeout(None);
        self.clear_callback();
    }

    pub fn is_launched(&self) -> bool {
        self.ivars().is_launched.get()
    }

    pub fn set_is_running(&self, value: bool) {
        self.ivars().is_running.set(value)
    }

    pub fn is_running(&self) -> bool {
        self.ivars().is_running.get()
    }

    pub fn exit(&self) {
        self.ivars().exit.set(true)
    }

    pub fn clear_exit(&self) {
        self.ivars().exit.set(false)
    }

    pub fn exiting(&self) -> bool {
        self.ivars().exit.get()
    }

    fn set_in_callback(&self, value: bool) {
        self.ivars().in_callback.set(value)
    }

    pub fn set_control_flow(&self, value: ControlFlow) {
        self.ivars().control_flow.set(value)
    }

    pub fn control_flow(&self) -> ControlFlow {
        self.ivars().control_flow.get()
    }

    pub fn queue_window_event(&self, window_id: WindowId, event: WindowEvent) {
        self.ivars()
            .pending_events
            .borrow_mut()
            .push_back(QueuedEvent::WindowEvent(window_id, event));
    }

    pub fn queue_device_event(&self, event: DeviceEvent) {
        self.ivars()
            .pending_events
            .borrow_mut()
            .push_back(QueuedEvent::DeviceEvent(event));
    }

    pub fn queue_static_scale_factor_changed_event(
        &self,
        window: Id<WinitWindow>,
        suggested_size: PhysicalSize<u32>,
        scale_factor: f64,
    ) {
        self.ivars()
            .pending_events
            .borrow_mut()
            .push_back(QueuedEvent::ScaleFactorChanged {
                window,
                suggested_size,
                scale_factor,
            });
    }

    pub fn handle_redraw(&self, window_id: WindowId) {
        let mtm = MainThreadMarker::from(self);
        // Redraw request might come out of order from the OS.
        // -> Don't go back into the callback when our callstack originates from there
        if !self.ivars().in_callback.get() {
            self.handle_event(Event::WindowEvent {
                window_id: RootWindowId(window_id),
                event: WindowEvent::RedrawRequested,
            });
            self.ivars().in_callback.set(false);

            // `pump_events` will request to stop immediately _after_ dispatching RedrawRequested events
            // as a way to ensure that `pump_events` can't block an external loop indefinitely
            if self.ivars().stop_on_redraw.get() {
                let app = NSApplication::sharedApplication(mtm);
                stop_app_immediately(&app);
            }
        }
    }

    pub fn queue_redraw(&self, window_id: WindowId) {
        let mut pending_redraw = self.ivars().pending_redraw.borrow_mut();
        if !pending_redraw.contains(&window_id) {
            pending_redraw.push(window_id);
        }
        unsafe { RunLoop::get() }.wakeup();
    }

    fn handle_event(&self, event: Event<HandlePendingUserEvents>) {
        if let Some(ref mut callback) = *self.ivars().callback.borrow_mut() {
            callback.handle_event(event)
        }
    }

    /// dispatch `NewEvents(Init)` + `Resumed`
    pub fn dispatch_init_events(&self) {
        self.set_in_callback(true);
        self.handle_event(Event::NewEvents(StartCause::Init));
        // NB: For consistency all platforms must emit a 'resumed' event even though macOS
        // applications don't themselves have a formal suspend/resume lifecycle.
        self.handle_event(Event::Resumed);
        self.set_in_callback(false);
    }

    // Called by RunLoopObserver after finishing waiting for new events
    pub fn wakeup(&self, panic_info: Weak<PanicInfo>) {
        let mtm = MainThreadMarker::from(self);
        let panic_info = panic_info
            .upgrade()
            .expect("The panic info must exist here. This failure indicates a developer error.");

        // Return when in callback due to https://github.com/rust-windowing/winit/issues/1779
        if panic_info.is_panicking()
            || self.ivars().in_callback.get()
            || !self.have_callback()
            || !self.is_running()
        {
            return;
        }

        if self.ivars().stop_after_wait.get() {
            let app = NSApplication::sharedApplication(mtm);
            stop_app_immediately(&app);
        }

        let start = self.ivars().start_time.get().unwrap();
        let cause = match self.control_flow() {
            ControlFlow::Poll => StartCause::Poll,
            ControlFlow::Wait => StartCause::WaitCancelled {
                start,
                requested_resume: None,
            },
            ControlFlow::WaitUntil(requested_resume) => {
                if Instant::now() >= requested_resume {
                    StartCause::ResumeTimeReached {
                        start,
                        requested_resume,
                    }
                } else {
                    StartCause::WaitCancelled {
                        start,
                        requested_resume: Some(requested_resume),
                    }
                }
            }
        };

        self.set_in_callback(true);
        self.handle_event(Event::NewEvents(cause));
        self.set_in_callback(false);
    }

    // Called by RunLoopObserver before waiting for new events
    pub fn cleared(&self, panic_info: Weak<PanicInfo>) {
        let mtm = MainThreadMarker::from(self);
        let panic_info = panic_info
            .upgrade()
            .expect("The panic info must exist here. This failure indicates a developer error.");

        // Return when in callback due to https://github.com/rust-windowing/winit/issues/1779
        // XXX: how does it make sense that `in_callback()` can ever return `true` here if we're
        // about to return to the `CFRunLoop` to poll for new events?
        if panic_info.is_panicking()
            || self.ivars().in_callback.get()
            || !self.have_callback()
            || !self.is_running()
        {
            return;
        }

        self.set_in_callback(true);
        #[allow(deprecated)]
        self.handle_event(Event::UserEvent(HandlePendingUserEvents));

        let events = mem::take(&mut *self.ivars().pending_events.borrow_mut());
        for event in events {
            match event {
                QueuedEvent::WindowEvent(window_id, event) => {
                    self.handle_event(Event::WindowEvent {
                        window_id: RootWindowId(window_id),
                        event,
                    });
                }
                QueuedEvent::DeviceEvent(event) => {
                    self.handle_event(Event::DeviceEvent {
                        device_id: DEVICE_ID,
                        event,
                    });
                }
                QueuedEvent::ScaleFactorChanged {
                    window,
                    suggested_size,
                    scale_factor,
                } => {
                    if let Some(ref mut callback) = *self.ivars().callback.borrow_mut() {
                        let new_inner_size = Arc::new(Mutex::new(suggested_size));
                        let scale_factor_changed_event = Event::WindowEvent {
                            window_id: RootWindowId(window.id()),
                            event: WindowEvent::ScaleFactorChanged {
                                scale_factor,
                                inner_size_writer: InnerSizeWriter::new(Arc::downgrade(
                                    &new_inner_size,
                                )),
                            },
                        };

                        callback.handle_event(scale_factor_changed_event);

                        let physical_size = *new_inner_size.lock().unwrap();
                        drop(new_inner_size);
                        let logical_size = physical_size.to_logical(scale_factor);
                        let size = NSSize::new(logical_size.width, logical_size.height);
                        window.setContentSize(size);

                        let resized_event = Event::WindowEvent {
                            window_id: RootWindowId(window.id()),
                            event: WindowEvent::Resized(physical_size),
                        };
                        callback.handle_event(resized_event);
                    }
                }
            }
        }

        let redraw = mem::take(&mut *self.ivars().pending_redraw.borrow_mut());
        for window_id in redraw {
            self.handle_event(Event::WindowEvent {
                window_id: RootWindowId(window_id),
                event: WindowEvent::RedrawRequested,
            });
        }

        self.handle_event(Event::AboutToWait);
        self.set_in_callback(false);

        if self.exiting() {
            let app = NSApplication::sharedApplication(mtm);
            stop_app_immediately(&app);
        }

        if self.ivars().stop_before_wait.get() {
            let app = NSApplication::sharedApplication(mtm);
            stop_app_immediately(&app);
        }
        self.ivars().start_time.set(Some(Instant::now()));
        let wait_timeout = self.ivars().wait_timeout.get(); // configured by pump_events
        let app_timeout = match self.control_flow() {
            ControlFlow::Wait => None,
            ControlFlow::Poll => Some(Instant::now()),
            ControlFlow::WaitUntil(instant) => Some(instant),
        };
        self.ivars()
            .waker
            .borrow_mut()
            .start_at(min_timeout(wait_timeout, app_timeout));
    }
}

#[derive(Debug)]
pub(crate) enum QueuedEvent {
    WindowEvent(WindowId, WindowEvent),
    DeviceEvent(DeviceEvent),
    ScaleFactorChanged {
        window: Id<WinitWindow>,
        suggested_size: PhysicalSize<u32>,
        scale_factor: f64,
    },
}

/// The event loop may have queued user events ready.
#[derive(Debug)]
pub(crate) struct HandlePendingUserEvents;

#[derive(Debug)]
struct EventLoopHandler {
    #[allow(clippy::type_complexity)]
    callback: Weak<RefCell<dyn FnMut(Event<HandlePendingUserEvents>, &RootWindowTarget)>>,
    window_target: Rc<RootWindowTarget>,
}

impl EventLoopHandler {
    fn handle_event(&mut self, event: Event<HandlePendingUserEvents>) {
        // `NSApplication` and our app delegate are global state and so it's possible
        // that we could get a delegate callback after the application has exit an
        // `EventLoop`. If the loop has been exit then our weak `self.callback`
        // will fail to upgrade.
        //
        // We don't want to panic or output any verbose logging if we fail to
        // upgrade the weak reference since it might be valid that the application
        // re-starts the `NSApplication` after exiting a Winit `EventLoop`
        if let Some(callback) = self.callback.upgrade() {
            let mut callback = callback.borrow_mut();
            (callback)(event, &self.window_target);
        }
    }
}

/// Returns the minimum `Option<Instant>`, taking into account that `None`
/// equates to an infinite timeout, not a zero timeout (so can't just use
/// `Option::min`)
fn min_timeout(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    a.map_or(b, |a_timeout| {
        b.map_or(Some(a_timeout), |b_timeout| Some(a_timeout.min(b_timeout)))
    })
}

/// A hack to make activation of multiple windows work when creating them before
/// `applicationDidFinishLaunching:` / `Event::Event::NewEvents(StartCause::Init)`.
///
/// Alternative to this would be the user calling `window.set_visible(true)` in
/// `StartCause::Init`.
///
/// If this becomes too bothersome to maintain, it can probably be removed
/// without too much damage.
fn window_activation_hack(app: &NSApplication) {
    // TODO: Proper ordering of the windows
    app.windows().into_iter().for_each(|window| {
        // Call `makeKeyAndOrderFront` if it was called on the window in `WinitWindow::new`
        // This way we preserve the user's desired initial visiblity status
        // TODO: Also filter on the type/"level" of the window, and maybe other things?
        if window.isVisible() {
            log::trace!("Activating visible window");
            window.makeKeyAndOrderFront(None);
        } else {
            log::trace!("Skipping activating invisible window");
        }
    })
}
