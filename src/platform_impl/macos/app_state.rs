use std::cell::{Cell, RefCell};
use std::mem;
use std::rc::Weak;
use std::time::Instant;

use objc2::rc::Retained;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSRunningApplication,
};
use objc2_foundation::{MainThreadMarker, NSNotification, NSObject, NSObjectProtocol};

use super::event_handler::EventHandler;
use super::event_loop::{notify_windows_of_exit, stop_app_immediately, ActiveEventLoop, PanicInfo};
use super::observer::{EventLoopWaker, RunLoop};
use super::{menu, WindowId, DEVICE_ID};
use crate::event::{DeviceEvent, Event, StartCause, WindowEvent};
use crate::event_loop::{ActiveEventLoop as RootActiveEventLoop, ControlFlow};
use crate::window::WindowId as RootWindowId;

#[derive(Debug)]
pub(super) struct AppState {
    activation_policy: Option<NSApplicationActivationPolicy>,
    default_menu: bool,
    activate_ignoring_other_apps: bool,
    run_loop: RunLoop,
    event_handler: EventHandler,
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
    pending_redraw: RefCell<Vec<WindowId>>,
    // NOTE: This is strongly referenced by our `NSWindowDelegate` and our `NSView` subclass, and
    // as such should be careful to not add fields that, in turn, strongly reference those.
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
        type Ivars = AppState;
    }

    unsafe impl NSObjectProtocol for ApplicationDelegate {}

    unsafe impl NSApplicationDelegate for ApplicationDelegate {
        #[method(applicationDidFinishLaunching:)]
        fn app_did_finish_launching(&self, notification: &NSNotification) {
            self.did_finish_launching(notification)
        }

        #[method(applicationWillTerminate:)]
        fn app_will_terminate(&self, notification: &NSNotification) {
            self.will_terminate(notification)
        }
    }
);

impl ApplicationDelegate {
    pub(super) fn new(
        mtm: MainThreadMarker,
        activation_policy: Option<NSApplicationActivationPolicy>,
        default_menu: bool,
        activate_ignoring_other_apps: bool,
    ) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(AppState {
            activation_policy,
            default_menu,
            activate_ignoring_other_apps,
            run_loop: RunLoop::main(mtm),
            event_handler: EventHandler::new(),
            stop_on_launch: Cell::new(false),
            stop_before_wait: Cell::new(false),
            stop_after_wait: Cell::new(false),
            stop_on_redraw: Cell::new(false),
            is_launched: Cell::new(false),
            is_running: Cell::new(false),
            exit: Cell::new(false),
            control_flow: Cell::new(ControlFlow::default()),
            waker: RefCell::new(EventLoopWaker::new()),
            start_time: Cell::new(None),
            wait_timeout: Cell::new(None),
            pending_redraw: RefCell::new(vec![]),
        });
        unsafe { msg_send_id![super(this), init] }
    }

    // NOTE: This will, globally, only be run once, no matter how many
    // `EventLoop`s the user creates.
    fn did_finish_launching(&self, _notification: &NSNotification) {
        trace_scope!("applicationDidFinishLaunching:");
        self.ivars().is_launched.set(true);

        let mtm = MainThreadMarker::from(self);
        let app = NSApplication::sharedApplication(mtm);
        // We need to delay setting the activation policy and activating the app
        // until `applicationDidFinishLaunching` has been called. Otherwise the
        // menu bar is initially unresponsive on macOS 10.15.
        // If no activation policy is explicitly provided, do not set it at all
        // to allow the package manifest to define behavior via LSUIElement.
        if let Some(activation_policy) = self.ivars().activation_policy {
            app.setActivationPolicy(activation_policy);
        } else {
            // If no activation policy is explicitly provided, and the application
            // is bundled, do not set the activation policy at all, to allow the
            // package manifest to define the behavior via LSUIElement.
            //
            // See:
            // - https://github.com/rust-windowing/winit/issues/261
            // - https://github.com/rust-windowing/winit/issues/3958
            let is_bundled =
                unsafe { NSRunningApplication::currentApplication().bundleIdentifier().is_some() };
            if !is_bundled {
                app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
            }
        }

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

        // If the application is being launched via `EventLoop::pump_app_events()` then we'll
        // want to stop the app once it is launched (and return to the external loop)
        //
        // In this case we still want to consider Winit's `EventLoop` to be "running",
        // so we call `start_running()` above.
        if self.ivars().stop_on_launch.get() {
            // NOTE: the original idea had been to only stop the underlying `RunLoop`
            // for the app but that didn't work as expected (`-[NSApplication run]`
            // effectively ignored the attempt to stop the RunLoop and re-started it).
            //
            // So we return from `pump_events` by stopping the application.
            let app = NSApplication::sharedApplication(mtm);
            stop_app_immediately(&app);
        }
    }

    fn will_terminate(&self, _notification: &NSNotification) {
        trace_scope!("applicationWillTerminate:");
        let mtm = MainThreadMarker::from(self);
        let app = NSApplication::sharedApplication(mtm);
        notify_windows_of_exit(&app);
        self.internal_exit();
    }

    pub fn get(mtm: MainThreadMarker) -> Retained<Self> {
        let app = NSApplication::sharedApplication(mtm);
        let delegate =
            unsafe { app.delegate() }.expect("a delegate was not configured on the application");
        if delegate.is_kind_of::<Self>() {
            // SAFETY: Just checked that the delegate is an instance of `ApplicationDelegate`
            unsafe { Retained::cast(delegate) }
        } else {
            panic!("tried to get a delegate that was not the one Winit has registered")
        }
    }

    /// Place the event handler in the application delegate for the duration
    /// of the given closure.
    pub fn set_event_handler<R>(
        &self,
        handler: impl FnMut(Event<HandlePendingUserEvents>, &RootActiveEventLoop),
        closure: impl FnOnce() -> R,
    ) -> R {
        self.ivars().event_handler.set(handler, closure)
    }

    /// If `pump_events` is called to progress the event loop then we
    /// bootstrap the event loop via `-[NSApplication run]` but will use
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
    /// NOTE: that if the `NSApplication` has been launched then that state is preserved,
    /// and we won't need to re-launch the app if subsequent EventLoops are run.
    pub fn internal_exit(&self) {
        self.handle_event(Event::LoopExiting);

        self.set_is_running(false);
        self.set_stop_on_redraw(false);
        self.set_stop_before_wait(false);
        self.set_stop_after_wait(false);
        self.set_wait_timeout(None);
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

    pub fn set_control_flow(&self, value: ControlFlow) {
        self.ivars().control_flow.set(value)
    }

    pub fn control_flow(&self) -> ControlFlow {
        self.ivars().control_flow.get()
    }

    pub fn maybe_queue_window_event(&self, window_id: WindowId, event: WindowEvent) {
        self.maybe_queue_event(Event::WindowEvent { window_id: RootWindowId(window_id), event });
    }

    pub fn handle_window_event(&self, window_id: WindowId, event: WindowEvent) {
        self.handle_event(Event::WindowEvent { window_id: RootWindowId(window_id), event });
    }

    pub fn maybe_queue_device_event(&self, event: DeviceEvent) {
        self.maybe_queue_event(Event::DeviceEvent { device_id: DEVICE_ID, event });
    }

    pub fn handle_redraw(&self, window_id: WindowId) {
        let mtm = MainThreadMarker::from(self);
        // Redraw request might come out of order from the OS.
        // -> Don't go back into the event handler when our callstack originates from there
        if !self.ivars().event_handler.in_use() {
            self.handle_event(Event::WindowEvent {
                window_id: RootWindowId(window_id),
                event: WindowEvent::RedrawRequested,
            });

            // `pump_events` will request to stop immediately _after_ dispatching RedrawRequested
            // events as a way to ensure that `pump_events` can't block an external loop
            // indefinitely
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
        self.ivars().run_loop.wakeup();
    }

    #[track_caller]
    fn maybe_queue_event(&self, event: Event<HandlePendingUserEvents>) {
        // Most programmer actions in AppKit (e.g. change window fullscreen, set focused, etc.)
        // result in an event being queued, and applied at a later point.
        //
        // However, it is not documented which actions do this, and which ones are done immediately,
        // so to make sure that we don't encounter re-entrancy issues, we first check if we're
        // currently handling another event, and if we are, we queue the event instead.
        if !self.ivars().event_handler.in_use() {
            self.handle_event(event);
        } else {
            tracing::debug!(?event, "had to queue event since another is currently being handled");
            let this = self.retain();
            self.ivars().run_loop.queue_closure(move || this.handle_event(event));
        }
    }

    #[track_caller]
    fn handle_event(&self, event: Event<HandlePendingUserEvents>) {
        self.ivars().event_handler.handle_event(event, &ActiveEventLoop::new_root(self.retain()))
    }

    /// dispatch `NewEvents(Init)` + `Resumed`
    pub fn dispatch_init_events(&self) {
        self.handle_event(Event::NewEvents(StartCause::Init));
        // NB: For consistency all platforms must emit a 'resumed' event even though macOS
        // applications don't themselves have a formal suspend/resume lifecycle.
        self.handle_event(Event::Resumed);
    }

    // Called by RunLoopObserver after finishing waiting for new events
    pub fn wakeup(&self, panic_info: Weak<PanicInfo>) {
        let mtm = MainThreadMarker::from(self);
        let panic_info = panic_info
            .upgrade()
            .expect("The panic info must exist here. This failure indicates a developer error.");

        // Return when in event handler due to https://github.com/rust-windowing/winit/issues/1779
        if panic_info.is_panicking() || !self.ivars().event_handler.ready() || !self.is_running() {
            return;
        }

        if self.ivars().stop_after_wait.get() {
            let app = NSApplication::sharedApplication(mtm);
            stop_app_immediately(&app);
        }

        let start = self.ivars().start_time.get().unwrap();
        let cause = match self.control_flow() {
            ControlFlow::Poll => StartCause::Poll,
            ControlFlow::Wait => StartCause::WaitCancelled { start, requested_resume: None },
            ControlFlow::WaitUntil(requested_resume) => {
                if Instant::now() >= requested_resume {
                    StartCause::ResumeTimeReached { start, requested_resume }
                } else {
                    StartCause::WaitCancelled { start, requested_resume: Some(requested_resume) }
                }
            },
        };

        self.handle_event(Event::NewEvents(cause));
    }

    // Called by RunLoopObserver before waiting for new events
    pub fn cleared(&self, panic_info: Weak<PanicInfo>) {
        let mtm = MainThreadMarker::from(self);
        let panic_info = panic_info
            .upgrade()
            .expect("The panic info must exist here. This failure indicates a developer error.");

        // Return when in event handler due to https://github.com/rust-windowing/winit/issues/1779
        // XXX: how does it make sense that `event_handler.ready()` can ever return `false` here if
        // we're about to return to the `CFRunLoop` to poll for new events?
        if panic_info.is_panicking() || !self.ivars().event_handler.ready() || !self.is_running() {
            return;
        }

        self.handle_event(Event::UserEvent(HandlePendingUserEvents));

        let redraw = mem::take(&mut *self.ivars().pending_redraw.borrow_mut());
        for window_id in redraw {
            self.handle_event(Event::WindowEvent {
                window_id: RootWindowId(window_id),
                event: WindowEvent::RedrawRequested,
            });
        }

        self.handle_event(Event::AboutToWait);

        if self.exiting() {
            let app = NSApplication::sharedApplication(mtm);
            stop_app_immediately(&app);
            notify_windows_of_exit(&app);
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
        self.ivars().waker.borrow_mut().start_at(min_timeout(wait_timeout, app_timeout));
    }
}

#[derive(Debug)]
pub(crate) struct HandlePendingUserEvents;

/// Returns the minimum `Option<Instant>`, taking into account that `None`
/// equates to an infinite timeout, not a zero timeout (so can't just use
/// `Option::min`)
fn min_timeout(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    a.map_or(b, |a_timeout| b.map_or(Some(a_timeout), |b_timeout| Some(a_timeout.min(b_timeout))))
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
        // This way we preserve the user's desired initial visibility status
        // TODO: Also filter on the type/"level" of the window, and maybe other things?
        if window.isVisible() {
            tracing::trace!("Activating visible window");
            window.makeKeyAndOrderFront(None);
        } else {
            tracing::trace!("Skipping activating invisible window");
        }
    })
}
