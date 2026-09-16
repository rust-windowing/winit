#![deny(unused_results)]

use std::cell::{Cell, OnceCell};
use std::collections::HashSet;
use std::os::raw::c_void;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{fmt, ptr};

use dispatch2::MainThreadBound;
use dpi::PhysicalSize;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopTimer, CGRect, CGSize,
    kCFRunLoopCommonModes,
};
use objc2_ui_kit::{UIApplication, UICoordinateSpace, UIView};
use winit_common::core_foundation::EventLoopProxy;
use winit_common::event_handler::EventHandler;
use winit_core::application::ApplicationHandler;
use winit_core::event::{StartCause, SurfaceSizeWriter, WindowEvent};
use winit_core::event_loop::ControlFlow;
use winit_core::window::WindowId;

use crate::event_loop::ActiveEventLoop;
use crate::window::WinitUIWindow;

macro_rules! bug {
    ($($msg:tt)*) => {
        panic!("winit iOS bug, file an issue: {}", format!($($msg)*))
    };
}

/// Get the global event handler for the application.
///
/// This is stored separately from AppState, since AppState needs to be accessible while the handler
/// is executing.
fn get_handler(mtm: MainThreadMarker) -> &'static EventHandler {
    // SAFETY: Creating `StaticMainThreadBound` in a `const` context, where there is no concept
    // of the main thread.
    static GLOBAL: MainThreadBound<OnceCell<EventHandler>> =
        MainThreadBound::new(OnceCell::new(), unsafe { MainThreadMarker::new_unchecked() });

    GLOBAL.get(mtm).get_or_init(EventHandler::new)
}

#[derive(Debug)]
pub(crate) enum EventWrapper {
    Window { window_id: WindowId, event: WindowEvent },
    ScaleFactorChanged(ScaleFactorChanged),
}

#[derive(Debug)]
pub struct ScaleFactorChanged {
    pub(super) window: Retained<WinitUIWindow>,
    pub(super) suggested_size: PhysicalSize<u32>,
    pub(super) scale_factor: f64,
}

impl EventWrapper {
    fn is_redraw(&self) -> bool {
        matches!(self, Self::Window { event: WindowEvent::RedrawRequested, .. })
    }
}

// this is the state machine for the app lifecycle
#[derive(Clone, Copy, Debug)]
#[must_use = "dropping `AppStateImpl` without inspecting it is probably a bug"]
enum AppStateImpl {
    Initial,
    ProcessingEvents { active_control_flow: ControlFlow },
    ProcessingRedraws { active_control_flow: ControlFlow },
    Waiting { start: Instant },
    PollFinished,
    Terminated,
}

pub(crate) struct AppState {
    state: Cell<AppStateImpl>,
    control_flow: Cell<ControlFlow>,
    waker: EventLoopWaker,
    event_loop_proxy: Arc<EventLoopProxy>,
    queued_events: Cell<Vec<EventWrapper>>,
    queued_gpu_redraws: Cell<HashSet<Retained<WinitUIWindow>>>,
}

impl fmt::Debug for AppState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppState")
            .field("control_flow", &self.control_flow)
            .field("waker", &self.waker)
            .field("event_loop_proxy", &self.event_loop_proxy)
            .field("queued_events", &"Cell<...>")
            .field("queued_gpu_redraws", &"Cell<...>")
            .finish_non_exhaustive()
    }
}

// SAFETY: Creating `MainThreadBound` in a `const` context,
// where there is no concept of the main thread.
static GLOBAL: MainThreadBound<OnceCell<AppState>> =
    MainThreadBound::new(OnceCell::new(), unsafe { MainThreadMarker::new_unchecked() });

impl AppState {
    pub(crate) fn setup_global(mtm: MainThreadMarker) -> bool {
        let event_loop_proxy = Arc::new(EventLoopProxy::new(mtm, move || {
            get_handler(mtm).handle(|app| app.proxy_wake_up(&ActiveEventLoop { mtm }));
        }));
        GLOBAL
            .get(mtm)
            .set(Self {
                state: Cell::new(AppStateImpl::Initial),
                control_flow: Cell::new(ControlFlow::default()),
                waker: EventLoopWaker::new(CFRunLoop::main().unwrap()),
                event_loop_proxy,
                queued_events: Cell::new(Vec::new()),
                queued_gpu_redraws: Cell::new(HashSet::new()),
            })
            .is_ok()
    }

    pub(crate) fn get(mtm: MainThreadMarker) -> &'static Self {
        GLOBAL.get(mtm).get().expect("tried to get application state before it was registered")
    }

    fn has_launched(&self) -> bool {
        !matches!(self.state.get(), AppStateImpl::Initial)
    }

    fn has_terminated(&self) -> bool {
        matches!(self.state.get(), AppStateImpl::Terminated)
    }

    fn did_finish_launching_transition(&self) {
        match self.state.get() {
            AppStateImpl::Initial => {},
            s => bug!("unexpected state {:?}", s),
        }
        self.state
            .set(AppStateImpl::ProcessingEvents { active_control_flow: self.control_flow.get() });
    }

    fn wakeup_transition(&self) -> Option<StartCause> {
        // before `AppState::did_finish_launching` is called, pretend there is no running
        // event loop.
        if !self.has_launched() || self.has_terminated() {
            return None;
        }

        let start_cause = match (self.control_flow.get(), self.state.get()) {
            (ControlFlow::Poll, AppStateImpl::PollFinished) => StartCause::Poll,
            (ControlFlow::Wait, AppStateImpl::Waiting { start }) => {
                StartCause::WaitCancelled { start, requested_resume: None }
            },
            (ControlFlow::WaitUntil(requested_resume), AppStateImpl::Waiting { start }) => {
                if Instant::now() >= requested_resume {
                    StartCause::ResumeTimeReached { start, requested_resume }
                } else {
                    StartCause::WaitCancelled { start, requested_resume: Some(requested_resume) }
                }
            },
            s => bug!("`EventHandler` unexpectedly woke up {:?}", s),
        };

        self.state
            .set(AppStateImpl::ProcessingEvents { active_control_flow: self.control_flow.get() });
        Some(start_cause)
    }

    fn main_events_cleared_transition(&self) {
        let active_control_flow = match self.state.get() {
            AppStateImpl::ProcessingEvents { active_control_flow } => active_control_flow,
            s => bug!("unexpected state {:?}", s),
        };
        self.state.set(AppStateImpl::ProcessingRedraws { active_control_flow });
    }

    fn events_cleared_transition(&self) {
        if !self.has_launched() || self.has_terminated() {
            return;
        }
        let old = match self.state.get() {
            AppStateImpl::ProcessingRedraws { active_control_flow } => active_control_flow,
            s => bug!("unexpected state {:?}", s),
        };

        let new = self.control_flow.get();
        match (old, new) {
            (ControlFlow::Wait, ControlFlow::Wait) => {
                let start = Instant::now();
                self.state.set(AppStateImpl::Waiting { start });
                self.waker.stop()
            },
            (ControlFlow::WaitUntil(old_instant), ControlFlow::WaitUntil(new_instant))
                if old_instant == new_instant =>
            {
                let start = Instant::now();
                self.state.set(AppStateImpl::Waiting { start });
            },
            (_, ControlFlow::Wait) => {
                let start = Instant::now();
                self.state.set(AppStateImpl::Waiting { start });
                self.waker.stop()
            },
            (_, ControlFlow::WaitUntil(new_instant)) => {
                let start = Instant::now();
                self.state.set(AppStateImpl::Waiting { start });
                self.waker.start_at(new_instant)
            },
            // Unlike on macOS, handle Poll to Poll transition here to call the waker
            (_, ControlFlow::Poll) => {
                self.state.set(AppStateImpl::PollFinished);
                self.waker.start()
            },
        }
    }

    fn terminated_transition(&self) {
        match self.state.replace(AppStateImpl::Terminated) {
            AppStateImpl::ProcessingEvents { .. } => {},
            s => bug!("terminated while not processing events {:?}", s),
        }
    }

    pub fn event_loop_proxy(&self) -> &Arc<EventLoopProxy> {
        &self.event_loop_proxy
    }

    pub(crate) fn set_control_flow(&self, control_flow: ControlFlow) {
        self.control_flow.set(control_flow);
    }

    pub(crate) fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }
}

pub(crate) fn queue_gl_or_metal_redraw(mtm: MainThreadMarker, window: Retained<WinitUIWindow>) {
    let this = AppState::get(mtm);
    match this.state.get() {
        AppStateImpl::Initial | AppStateImpl::ProcessingEvents { .. } => {
            let mut queued_gpu_redraws = this.queued_gpu_redraws.take();
            let _ = queued_gpu_redraws.insert(window);
            this.queued_gpu_redraws.set(queued_gpu_redraws);
        },
        s @ AppStateImpl::ProcessingRedraws { .. }
        | s @ AppStateImpl::Waiting { .. }
        | s @ AppStateImpl::PollFinished => bug!("unexpected state {:?}", s),
        AppStateImpl::Terminated => {
            panic!("Attempt to create a `Window` after the app has terminated")
        },
    }
}

pub(crate) fn launch<R>(
    mtm: MainThreadMarker,
    app: impl ApplicationHandler,
    run: impl FnOnce() -> R,
) -> R {
    get_handler(mtm).set(Box::new(app), run)
}

pub fn did_finish_launching(mtm: MainThreadMarker) {
    let this = AppState::get(mtm);

    this.waker.start();
    this.did_finish_launching_transition();

    get_handler(mtm).handle(|app| app.new_events(&ActiveEventLoop { mtm }, StartCause::Init));
    get_handler(mtm).handle(|app| app.can_create_surfaces(&ActiveEventLoop { mtm }));
    handle_nonuser_events(mtm, []);
}

// AppState::did_finish_launching handles the special transition `Init`
pub fn handle_wakeup_transition(mtm: MainThreadMarker) {
    let this = AppState::get(mtm);
    let cause = match this.wakeup_transition() {
        None => return,
        Some(cause) => cause,
    };

    get_handler(mtm).handle(|app| app.new_events(&ActiveEventLoop { mtm }, cause));
    handle_nonuser_events(mtm, []);
}

pub(crate) fn handle_nonuser_event(mtm: MainThreadMarker, event: EventWrapper) {
    handle_nonuser_events(mtm, std::iter::once(event))
}

pub(crate) fn handle_nonuser_events<I: IntoIterator<Item = EventWrapper>>(
    mtm: MainThreadMarker,
    events: I,
) {
    let this = AppState::get(mtm);
    if this.has_terminated() {
        return;
    }

    if !get_handler(mtm).ready() {
        // Prevent re-entrancy; queue the events up for once we're done handling the event instead.
        let mut queued_events = this.queued_events.take();
        queued_events.extend(events);
        this.queued_events.set(queued_events);
        return;
    }

    let processing_redraws = matches!(this.state.get(), AppStateImpl::ProcessingRedraws { .. });

    for event in events {
        if !processing_redraws && event.is_redraw() {
            tracing::info!("processing `RedrawRequested` during the main event loop");
        } else if processing_redraws && !event.is_redraw() {
            tracing::warn!(
                "processing non `RedrawRequested` event after the main event loop: {:#?}",
                event
            );
        }
        handle_wrapped_event(mtm, event)
    }

    loop {
        let queued_events = this.queued_events.take();
        if queued_events.is_empty() {
            break;
        }

        for event in queued_events {
            if !processing_redraws && event.is_redraw() {
                tracing::info!("processing `RedrawRequested` during the main event loop");
            } else if processing_redraws && !event.is_redraw() {
                tracing::warn!(
                    "processing non-`RedrawRequested` event after the main event loop: {:#?}",
                    event
                );
            }
            handle_wrapped_event(mtm, event);
        }
    }
}

fn handle_user_events(mtm: MainThreadMarker) {
    let this = AppState::get(mtm);
    if matches!(this.state.get(), AppStateImpl::ProcessingRedraws { .. }) {
        bug!("user events attempted to be sent out while `ProcessingRedraws`");
    }

    loop {
        let queued_events = this.queued_events.take();
        if queued_events.is_empty() {
            break;
        }

        for event in queued_events {
            handle_wrapped_event(mtm, event);
        }
    }
}

pub(crate) fn send_occluded_event_for_all_windows(application: &UIApplication, occluded: bool) {
    let mtm = MainThreadMarker::from(application);

    let mut events = Vec::new();
    #[allow(deprecated)]
    for window in application.windows().iter() {
        if let Ok(window) = window.downcast::<WinitUIWindow>() {
            events.push(EventWrapper::Window {
                window_id: window.id(),
                event: WindowEvent::Occluded(occluded),
            });
        }
    }
    handle_nonuser_events(mtm, events);
}

pub fn handle_main_events_cleared(mtm: MainThreadMarker) {
    let this = AppState::get(mtm);
    if !this.has_launched() || this.has_terminated() {
        return;
    }
    match this.state.get() {
        AppStateImpl::ProcessingEvents { .. } => {},
        _ => bug!("`ProcessingRedraws` happened unexpectedly"),
    };

    handle_user_events(mtm);

    this.main_events_cleared_transition();
    let queued_gpu_redraws = this.queued_gpu_redraws.take();
    let redraw_events = queued_gpu_redraws.into_iter().map(|window| EventWrapper::Window {
        window_id: window.id(),
        event: WindowEvent::RedrawRequested,
    });

    handle_nonuser_events(mtm, redraw_events);
    get_handler(mtm).handle(|app| app.about_to_wait(&ActiveEventLoop { mtm }));
    handle_nonuser_events(mtm, []);
}

pub fn handle_events_cleared(mtm: MainThreadMarker) {
    AppState::get(mtm).events_cleared_transition();
}

pub(crate) fn handle_resumed(mtm: MainThreadMarker) {
    get_handler(mtm).handle(|app| app.resumed(&ActiveEventLoop { mtm }));
    handle_nonuser_events(mtm, []);
}

pub(crate) fn handle_suspended(mtm: MainThreadMarker) {
    get_handler(mtm).handle(|app| app.suspended(&ActiveEventLoop { mtm }));
    handle_nonuser_events(mtm, []);
}

pub(crate) fn handle_memory_warning(mtm: MainThreadMarker) {
    get_handler(mtm).handle(|app| app.memory_warning(&ActiveEventLoop { mtm }));
    handle_nonuser_events(mtm, []);
}

pub(crate) fn terminated(application: &UIApplication) {
    let mtm = MainThreadMarker::from(application);

    let mut events = Vec::new();
    #[allow(deprecated)]
    for window in application.windows().iter() {
        if let Ok(window) = window.downcast::<WinitUIWindow>() {
            events.push(EventWrapper::Window {
                window_id: window.id(),
                event: WindowEvent::Destroyed,
            });
        }
    }
    handle_nonuser_events(mtm, events);

    let this = AppState::get(mtm);
    this.terminated_transition();
    // Prevent EventLoopProxy from firing again.
    this.event_loop_proxy.invalidate();

    get_handler(mtm).terminate();
}

fn handle_wrapped_event(mtm: MainThreadMarker, event: EventWrapper) {
    match event {
        EventWrapper::Window { window_id, event } => get_handler(mtm)
            .handle(|app| app.window_event(&ActiveEventLoop { mtm }, window_id, event)),
        EventWrapper::ScaleFactorChanged(event) => handle_hidpi_proxy(mtm, event),
    }
}

fn handle_hidpi_proxy(mtm: MainThreadMarker, event: ScaleFactorChanged) {
    let ScaleFactorChanged { suggested_size, scale_factor, window } = event;
    let new_surface_size = Arc::new(Mutex::new(suggested_size));
    get_handler(mtm).handle(|app| {
        app.window_event(
            &ActiveEventLoop { mtm },
            window.id(),
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                surface_size_writer: SurfaceSizeWriter::new(Arc::downgrade(&new_surface_size)),
            },
        );
    });
    let (view, screen_frame) = get_view_and_screen_frame(&window);
    let physical_size = *new_surface_size.lock().unwrap();
    drop(new_surface_size);
    let logical_size = physical_size.to_logical(scale_factor);
    let size = CGSize::new(logical_size.width, logical_size.height);
    let new_frame: CGRect = CGRect::new(screen_frame.origin, size);
    view.setFrame(new_frame);
}

fn get_view_and_screen_frame(window: &WinitUIWindow) -> (Retained<UIView>, CGRect) {
    let view_controller = window.rootViewController().unwrap();
    let view = view_controller.view().unwrap();
    let bounds = window.bounds();
    let screen = window.screen();
    let screen_space = screen.coordinateSpace();
    let screen_frame = window.convertRect_toCoordinateSpace(bounds, &screen_space);
    (view, screen_frame)
}

#[derive(Debug)]
struct EventLoopWaker {
    timer: CFRetained<CFRunLoopTimer>,
}

impl Drop for EventLoopWaker {
    fn drop(&mut self) {
        self.timer.invalidate();
    }
}

impl EventLoopWaker {
    fn new(rl: CFRetained<CFRunLoop>) -> EventLoopWaker {
        extern "C-unwind" fn wakeup_main_loop(_timer: *mut CFRunLoopTimer, _info: *mut c_void) {}
        unsafe {
            // Create a timer with a 0.1µs interval (1ns does not work) to mimic polling.
            // It is initially setup with a first fire time really far into the
            // future, but that gets changed to fire immediately in did_finish_launching
            let timer = CFRunLoopTimer::new(
                None,
                f64::MAX,
                0.000_000_1,
                0,
                0,
                Some(wakeup_main_loop),
                ptr::null_mut(),
            )
            .unwrap();
            rl.add_timer(Some(&timer), kCFRunLoopCommonModes);

            EventLoopWaker { timer }
        }
    }

    fn stop(&self) {
        self.timer.set_next_fire_date(f64::MAX);
    }

    fn start(&self) {
        self.timer.set_next_fire_date(f64::MIN);
    }

    fn start_at(&self, instant: Instant) {
        let now = Instant::now();
        if now >= instant {
            self.start();
        } else {
            let current = CFAbsoluteTimeGetCurrent();
            let duration = instant - now;
            let fsecs =
                duration.subsec_nanos() as f64 / 1_000_000_000.0 + duration.as_secs() as f64;
            self.timer.set_next_fire_date(current + fsecs);
        }
    }
}
