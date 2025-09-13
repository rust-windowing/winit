use std::cell::{Cell, OnceCell, RefCell};
use std::mem;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use dispatch2::MainThreadBound;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy, NSRunningApplication};
use objc2_foundation::NSNotification;
use winit_common::core_foundation::{EventLoopProxy, MainRunLoop};
use winit_common::event_handler::EventHandler;
use winit_core::application::ApplicationHandler;
use winit_core::event::{StartCause, WindowEvent};
use winit_core::event_loop::ControlFlow;
use winit_core::window::WindowId;

use super::event_loop::{notify_windows_of_exit, stop_app_immediately, ActiveEventLoop};
use super::menu;
use super::observer::EventLoopWaker;

#[derive(Debug)]
pub(super) struct AppState {
    mtm: MainThreadMarker,
    activation_policy: Option<NSApplicationActivationPolicy>,
    default_menu: bool,
    activate_ignoring_other_apps: bool,
    run_loop: MainRunLoop,
    event_loop_proxy: Arc<EventLoopProxy>,
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

// SAFETY: Creating `MainThreadBound` in a `const` context, where there is no concept of the
// main thread.
static GLOBAL: MainThreadBound<OnceCell<Rc<AppState>>> =
    MainThreadBound::new(OnceCell::new(), unsafe { MainThreadMarker::new_unchecked() });

impl AppState {
    pub(super) fn setup_global(
        mtm: MainThreadMarker,
        activation_policy: Option<NSApplicationActivationPolicy>,
        default_menu: bool,
        activate_ignoring_other_apps: bool,
    ) -> Option<Rc<Self>> {
        let event_loop_proxy = Arc::new(EventLoopProxy::new(mtm, move || {
            Self::get(mtm).with_handler(|app, event_loop| app.proxy_wake_up(event_loop));
        }));

        let this = Rc::new(Self {
            mtm,
            activation_policy,
            default_menu,
            activate_ignoring_other_apps,
            run_loop: MainRunLoop::get(mtm),
            event_loop_proxy,
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

        GLOBAL.get(mtm).set(this.clone()).ok().and(Some(this))
    }

    pub fn get(mtm: MainThreadMarker) -> Rc<Self> {
        GLOBAL
            .get(mtm)
            .get()
            .expect("tried to get application state before it was registered")
            .clone()
    }

    // NOTE: This notification will, globally, only be emitted once,
    // no matter how many `EventLoop`s the user creates.
    pub fn did_finish_launching(self: &Rc<Self>, _notification: &NSNotification) {
        trace_scope!("NSApplicationDidFinishLaunchingNotification");
        self.is_launched.set(true);

        let app = NSApplication::sharedApplication(self.mtm);
        // We need to delay setting the activation policy and activating the app
        // until `applicationDidFinishLaunching` has been called. Otherwise the
        // menu bar is initially unresponsive on macOS 10.15.
        if let Some(activation_policy) = self.activation_policy {
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

        #[allow(deprecated)]
        app.activateIgnoringOtherApps(self.activate_ignoring_other_apps);

        if self.default_menu {
            // The menubar initialization should be before the `NewEvents` event, to allow
            // overriding of the default menu even if it's created
            menu::initialize(&app);
        }

        self.waker.borrow_mut().start();

        self.set_is_running(true);
        self.dispatch_init_events();

        // If the application is being launched via `EventLoop::pump_app_events()` then we'll
        // want to stop the app once it is launched (and return to the external loop)
        //
        // In this case we still want to consider Winit's `EventLoop` to be "running",
        // so we call `start_running()` above.
        if self.stop_on_launch.get() {
            // NOTE: the original idea had been to only stop the underlying `RunLoop`
            // for the app but that didn't work as expected (`-[NSApplication run]`
            // effectively ignored the attempt to stop the RunLoop and re-started it).
            //
            // So we return from `pump_events` by stopping the application.
            let app = NSApplication::sharedApplication(self.mtm);
            stop_app_immediately(&app);
        }
    }

    pub fn will_terminate(self: &Rc<Self>, _notification: &NSNotification) {
        trace_scope!("NSApplicationWillTerminateNotification");
        let app = NSApplication::sharedApplication(self.mtm);
        notify_windows_of_exit(&app);
        self.event_handler.terminate();
        self.internal_exit();
    }

    /// Place the event handler in the application state for the duration
    /// of the given closure.
    pub fn set_event_handler<R>(
        &self,
        handler: impl ApplicationHandler,
        closure: impl FnOnce() -> R,
    ) -> R {
        self.event_handler.set(Box::new(handler), closure)
    }

    pub fn event_loop_proxy(&self) -> &Arc<EventLoopProxy> {
        &self.event_loop_proxy
    }

    /// If `pump_events` is called to progress the event loop then we
    /// bootstrap the event loop via `-[NSApplication run]` but will use
    /// `CFRunLoopRunInMode` for subsequent calls to `pump_events`.
    pub fn set_stop_on_launch(&self) {
        self.stop_on_launch.set(true);
    }

    pub fn set_stop_before_wait(&self, value: bool) {
        self.stop_before_wait.set(value)
    }

    pub fn set_stop_after_wait(&self, value: bool) {
        self.stop_after_wait.set(value)
    }

    pub fn set_stop_on_redraw(&self, value: bool) {
        self.stop_on_redraw.set(value)
    }

    pub fn set_wait_timeout(&self, value: Option<Instant>) {
        self.wait_timeout.set(value)
    }

    /// Clears the `running` state and resets the `control_flow` state when an `EventLoop` exits.
    ///
    /// NOTE: that if the `NSApplication` has been launched then that state is preserved,
    /// and we won't need to re-launch the app if subsequent EventLoops are run.
    pub fn internal_exit(self: &Rc<Self>) {
        self.set_is_running(false);
        self.set_stop_on_redraw(false);
        self.set_stop_before_wait(false);
        self.set_stop_after_wait(false);
        self.set_wait_timeout(None);
    }

    pub fn is_launched(&self) -> bool {
        self.is_launched.get()
    }

    pub fn set_is_running(&self, value: bool) {
        self.is_running.set(value)
    }

    pub fn is_running(&self) -> bool {
        self.is_running.get()
    }

    pub fn exit(&self) {
        self.exit.set(true)
    }

    pub fn clear_exit(&self) {
        self.exit.set(false)
    }

    pub fn exiting(&self) -> bool {
        self.exit.get()
    }

    pub fn set_control_flow(&self, value: ControlFlow) {
        self.control_flow.set(value)
    }

    pub fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }

    pub fn handle_redraw(self: &Rc<Self>, window_id: WindowId) {
        // Redraw request might come out of order from the OS.
        // -> Don't go back into the event handler when our callstack originates from there
        if !self.event_handler.in_use() {
            self.with_handler(|app, event_loop| {
                app.window_event(event_loop, window_id, WindowEvent::RedrawRequested);
            });

            // `pump_events` will request to stop immediately _after_ dispatching RedrawRequested
            // events as a way to ensure that `pump_events` can't block an external loop
            // indefinitely
            if self.stop_on_redraw.get() {
                let app = NSApplication::sharedApplication(self.mtm);
                stop_app_immediately(&app);
            }
        }
    }

    pub fn queue_redraw(&self, window_id: WindowId) {
        let mut pending_redraw = self.pending_redraw.borrow_mut();
        if !pending_redraw.contains(&window_id) {
            pending_redraw.push(window_id);
        }
        self.run_loop.wake_up();
    }

    #[track_caller]
    pub fn maybe_queue_with_handler(
        self: &Rc<Self>,
        callback: impl FnOnce(&mut dyn ApplicationHandler, &ActiveEventLoop) + 'static,
    ) {
        // Most programmer actions in AppKit (e.g. change window fullscreen, set focused, etc.)
        // result in an event being queued, and applied at a later point.
        //
        // However, it is not documented which actions do this, and which ones are done immediately,
        // so to make sure that we don't encounter re-entrancy issues, we first check if we're
        // currently handling another event, and if we are, we queue the event instead.
        if !self.event_handler.in_use() {
            self.with_handler(callback);
        } else {
            tracing::debug!("had to queue event since another is currently being handled");
            let this = Rc::clone(self);
            self.run_loop.queue_closure(move || {
                this.with_handler(callback);
            });
        }
    }

    #[track_caller]
    fn with_handler(
        self: &Rc<Self>,
        callback: impl FnOnce(&mut dyn ApplicationHandler, &ActiveEventLoop),
    ) {
        let event_loop = ActiveEventLoop { app_state: Rc::clone(self), mtm: self.mtm };
        self.event_handler.handle(|app| callback(app, &event_loop));
    }

    /// dispatch `NewEvents(Init)` + `Resumed`
    pub fn dispatch_init_events(self: &Rc<Self>) {
        self.with_handler(|app, event_loop| app.new_events(event_loop, StartCause::Init));
        // NB: For consistency all platforms must call `can_create_surfaces` even though macOS
        // applications don't themselves have a formal surface destroy/create lifecycle.
        self.with_handler(|app, event_loop| app.can_create_surfaces(event_loop));
    }

    // Called by RunLoopObserver after finishing waiting for new events
    pub fn wakeup(self: &Rc<Self>) {
        // Return when in event handler due to https://github.com/rust-windowing/winit/issues/1779
        // (we have registered to observe all modes, including modal event loops).
        if !self.event_handler.ready() || !self.is_running() {
            return;
        }

        if self.stop_after_wait.get() {
            let app = NSApplication::sharedApplication(self.mtm);
            stop_app_immediately(&app);
        }

        let start = self.start_time.get().unwrap();
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

        self.with_handler(|app, event_loop| app.new_events(event_loop, cause));
    }

    // Called by RunLoopObserver before waiting for new events
    pub fn cleared(self: &Rc<Self>) {
        // Return when in event handler due to https://github.com/rust-windowing/winit/issues/1779
        // (we have registered to observe all modes, including modal event loops).
        if !self.event_handler.ready() || !self.is_running() {
            return;
        }

        let redraw = mem::take(&mut *self.pending_redraw.borrow_mut());
        for window_id in redraw {
            self.with_handler(|app, event_loop| {
                app.window_event(event_loop, window_id, WindowEvent::RedrawRequested);
            });
        }
        self.with_handler(|app, event_loop| {
            app.about_to_wait(event_loop);
        });

        if self.exiting() {
            let app = NSApplication::sharedApplication(self.mtm);
            stop_app_immediately(&app);
            notify_windows_of_exit(&app);
        }

        if self.stop_before_wait.get() {
            let app = NSApplication::sharedApplication(self.mtm);
            stop_app_immediately(&app);
        }
        self.start_time.set(Some(Instant::now()));
        let wait_timeout = self.wait_timeout.get(); // configured by pump_events
        let app_timeout = match self.control_flow() {
            ControlFlow::Wait => None,
            ControlFlow::Poll => Some(Instant::now()),
            ControlFlow::WaitUntil(instant) => Some(instant),
        };
        self.waker.borrow_mut().start_at(min_timeout(wait_timeout, app_timeout));
    }
}

/// Returns the minimum `Option<Instant>`, taking into account that `None`
/// equates to an infinite timeout, not a zero timeout (so can't just use
/// `Option::min`)
fn min_timeout(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    a.map_or(b, |a_timeout| b.map_or(Some(a_timeout), |b_timeout| Some(a_timeout.min(b_timeout))))
}
