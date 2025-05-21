#![deny(unused_results)]

use std::cell::{OnceCell, RefCell, RefMut};
use std::collections::HashSet;
use std::os::raw::c_void;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{mem, ptr};

use dispatch2::MainThreadBound;
use dpi::PhysicalSize;
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_core_foundation::{
    kCFRunLoopCommonModes, CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopTimer, CGRect,
    CGSize,
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

macro_rules! bug_assert {
    ($test:expr, $($msg:tt)*) => {
        assert!($test, "winit iOS bug, file an issue: {}", format!($($msg)*))
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
#[derive(Debug)]
#[must_use = "dropping `AppStateImpl` without inspecting it is probably a bug"]
enum AppStateImpl {
    Initial {
        queued_gpu_redraws: HashSet<Retained<WinitUIWindow>>,
    },
    ProcessingEvents {
        queued_gpu_redraws: HashSet<Retained<WinitUIWindow>>,
        active_control_flow: ControlFlow,
    },
    ProcessingRedraws {
        active_control_flow: ControlFlow,
    },
    Waiting {
        start: Instant,
    },
    PollFinished,
    Terminated,
}

pub(crate) struct AppState {
    // This should never be `None`, except for briefly during a state transition.
    app_state: Option<AppStateImpl>,
    control_flow: ControlFlow,
    waker: EventLoopWaker,
    event_loop_proxy: Arc<EventLoopProxy>,
    queued_events: Vec<EventWrapper>,
}

impl AppState {
    pub(crate) fn get_mut(mtm: MainThreadMarker) -> RefMut<'static, AppState> {
        // basically everything in UIKit requires the main thread, so it's pointless to use the
        // std::sync APIs.
        // must be mut because plain `static` requires `Sync`
        static mut APP_STATE: RefCell<Option<AppState>> = RefCell::new(None);

        #[allow(unknown_lints)] // New lint below
        #[allow(static_mut_refs)] // TODO: Use `MainThreadBound` instead.
        let mut guard = unsafe { APP_STATE.borrow_mut() };
        if guard.is_none() {
            #[inline(never)]
            #[cold]
            fn init_guard(guard: &mut RefMut<'static, Option<AppState>>, mtm: MainThreadMarker) {
                let waker = EventLoopWaker::new(CFRunLoop::main().unwrap());
                let event_loop_proxy = Arc::new(EventLoopProxy::new(mtm, move || {
                    get_handler(mtm).handle(|app| app.proxy_wake_up(&ActiveEventLoop { mtm }));
                }));

                **guard = Some(AppState {
                    app_state: Some(AppStateImpl::Initial { queued_gpu_redraws: HashSet::new() }),
                    control_flow: ControlFlow::default(),
                    waker,
                    event_loop_proxy,
                    queued_events: Vec::new(),
                });
            }
            init_guard(&mut guard, mtm);
        }
        RefMut::map(guard, |state| state.as_mut().unwrap())
    }

    fn state(&self) -> &AppStateImpl {
        match &self.app_state {
            Some(ref state) => state,
            None => bug!("`AppState` previously failed a state transition"),
        }
    }

    fn state_mut(&mut self) -> &mut AppStateImpl {
        match &mut self.app_state {
            Some(ref mut state) => state,
            None => bug!("`AppState` previously failed a state transition"),
        }
    }

    fn take_state(&mut self) -> AppStateImpl {
        match self.app_state.take() {
            Some(state) => state,
            None => bug!("`AppState` previously failed a state transition"),
        }
    }

    fn set_state(&mut self, new_state: AppStateImpl) {
        bug_assert!(
            self.app_state.is_none(),
            "attempted to set an `AppState` without calling `take_state` first {:?}",
            self.app_state
        );
        self.app_state = Some(new_state)
    }

    fn replace_state(&mut self, new_state: AppStateImpl) -> AppStateImpl {
        match &mut self.app_state {
            Some(ref mut state) => mem::replace(state, new_state),
            None => bug!("`AppState` previously failed a state transition"),
        }
    }

    fn has_launched(&self) -> bool {
        !matches!(self.state(), AppStateImpl::Initial { .. })
    }

    fn has_terminated(&self) -> bool {
        matches!(self.state(), AppStateImpl::Terminated)
    }

    fn did_finish_launching_transition(&mut self) {
        let queued_gpu_redraws = match self.take_state() {
            AppStateImpl::Initial { queued_gpu_redraws } => queued_gpu_redraws,
            s => bug!("unexpected state {:?}", s),
        };
        self.set_state(AppStateImpl::ProcessingEvents {
            active_control_flow: self.control_flow,
            queued_gpu_redraws,
        });
    }

    fn wakeup_transition(&mut self) -> Option<StartCause> {
        // before `AppState::did_finish_launching` is called, pretend there is no running
        // event loop.
        if !self.has_launched() || self.has_terminated() {
            return None;
        }

        let start_cause = match (self.control_flow, self.take_state()) {
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

        self.set_state(AppStateImpl::ProcessingEvents {
            queued_gpu_redraws: Default::default(),
            active_control_flow: self.control_flow,
        });
        Some(start_cause)
    }

    fn main_events_cleared_transition(&mut self) -> HashSet<Retained<WinitUIWindow>> {
        let (queued_gpu_redraws, active_control_flow) = match self.take_state() {
            AppStateImpl::ProcessingEvents { queued_gpu_redraws, active_control_flow } => {
                (queued_gpu_redraws, active_control_flow)
            },
            s => bug!("unexpected state {:?}", s),
        };
        self.set_state(AppStateImpl::ProcessingRedraws { active_control_flow });
        queued_gpu_redraws
    }

    fn events_cleared_transition(&mut self) {
        if !self.has_launched() || self.has_terminated() {
            return;
        }
        let old = match self.take_state() {
            AppStateImpl::ProcessingRedraws { active_control_flow } => active_control_flow,
            s => bug!("unexpected state {:?}", s),
        };

        let new = self.control_flow;
        match (old, new) {
            (ControlFlow::Wait, ControlFlow::Wait) => {
                let start = Instant::now();
                self.set_state(AppStateImpl::Waiting { start });
                self.waker.stop()
            },
            (ControlFlow::WaitUntil(old_instant), ControlFlow::WaitUntil(new_instant))
                if old_instant == new_instant =>
            {
                let start = Instant::now();
                self.set_state(AppStateImpl::Waiting { start });
            },
            (_, ControlFlow::Wait) => {
                let start = Instant::now();
                self.set_state(AppStateImpl::Waiting { start });
                self.waker.stop()
            },
            (_, ControlFlow::WaitUntil(new_instant)) => {
                let start = Instant::now();
                self.set_state(AppStateImpl::Waiting { start });
                self.waker.start_at(new_instant)
            },
            // Unlike on macOS, handle Poll to Poll transition here to call the waker
            (_, ControlFlow::Poll) => {
                self.set_state(AppStateImpl::PollFinished);
                self.waker.start()
            },
        }
    }

    fn terminated_transition(&mut self) {
        match self.replace_state(AppStateImpl::Terminated) {
            AppStateImpl::ProcessingEvents { .. } => {},
            s => bug!("terminated while not processing events {:?}", s),
        }
    }

    pub fn event_loop_proxy(&self) -> &Arc<EventLoopProxy> {
        &self.event_loop_proxy
    }

    pub(crate) fn set_control_flow(&mut self, control_flow: ControlFlow) {
        self.control_flow = control_flow;
    }

    pub(crate) fn control_flow(&self) -> ControlFlow {
        self.control_flow
    }
}

pub(crate) fn queue_gl_or_metal_redraw(mtm: MainThreadMarker, window: Retained<WinitUIWindow>) {
    let mut this = AppState::get_mut(mtm);
    match this.state_mut() {
        &mut AppStateImpl::Initial { ref mut queued_gpu_redraws, .. }
        | &mut AppStateImpl::ProcessingEvents { ref mut queued_gpu_redraws, .. } => {
            let _ = queued_gpu_redraws.insert(window);
        },
        s @ &mut AppStateImpl::ProcessingRedraws { .. }
        | s @ &mut AppStateImpl::Waiting { .. }
        | s @ &mut AppStateImpl::PollFinished => bug!("unexpected state {:?}", s),
        &mut AppStateImpl::Terminated => {
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
    let mut this = AppState::get_mut(mtm);

    this.waker.start();

    // have to drop RefMut because the window setup code below can trigger new events
    drop(this);

    AppState::get_mut(mtm).did_finish_launching_transition();

    get_handler(mtm).handle(|app| app.new_events(&ActiveEventLoop { mtm }, StartCause::Init));
    get_handler(mtm).handle(|app| app.can_create_surfaces(&ActiveEventLoop { mtm }));
    handle_nonuser_events(mtm, []);
}

// AppState::did_finish_launching handles the special transition `Init`
pub fn handle_wakeup_transition(mtm: MainThreadMarker) {
    let mut this = AppState::get_mut(mtm);
    let cause = match this.wakeup_transition() {
        None => return,
        Some(cause) => cause,
    };
    drop(this);

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
    let mut this = AppState::get_mut(mtm);
    if this.has_terminated() {
        return;
    }

    if !get_handler(mtm).ready() {
        // Prevent re-entrancy; queue the events up for once we're done handling the event instead.
        this.queued_events.extend(events);
        return;
    }

    let processing_redraws = matches!(this.state(), AppStateImpl::ProcessingRedraws { .. });
    drop(this);

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
        let mut this = AppState::get_mut(mtm);
        let queued_events = mem::take(&mut this.queued_events);
        if queued_events.is_empty() {
            break;
        }
        drop(this);

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
    let this = AppState::get_mut(mtm);
    if matches!(this.state(), AppStateImpl::ProcessingRedraws { .. }) {
        bug!("user events attempted to be sent out while `ProcessingRedraws`");
    }
    drop(this);

    loop {
        let mut this = AppState::get_mut(mtm);
        let queued_events = mem::take(&mut this.queued_events);
        if queued_events.is_empty() {
            break;
        }
        drop(this);

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
    let mut this = AppState::get_mut(mtm);
    if !this.has_launched() || this.has_terminated() {
        return;
    }
    match this.state_mut() {
        AppStateImpl::ProcessingEvents { .. } => {},
        _ => bug!("`ProcessingRedraws` happened unexpectedly"),
    };
    drop(this);

    handle_user_events(mtm);

    let mut this = AppState::get_mut(mtm);
    let redraw_events: Vec<EventWrapper> = this
        .main_events_cleared_transition()
        .into_iter()
        .map(|window| EventWrapper::Window {
            window_id: window.id(),
            event: WindowEvent::RedrawRequested,
        })
        .collect();
    drop(this);

    handle_nonuser_events(mtm, redraw_events);
    get_handler(mtm).handle(|app| app.about_to_wait(&ActiveEventLoop { mtm }));
    handle_nonuser_events(mtm, []);
}

pub fn handle_events_cleared(mtm: MainThreadMarker) {
    AppState::get_mut(mtm).events_cleared_transition();
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

    let mut this = AppState::get_mut(mtm);
    this.terminated_transition();
    // Prevent EventLoopProxy from firing again.
    this.event_loop_proxy.invalidate();
    drop(this);

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
        app.window_event(&ActiveEventLoop { mtm }, window.id(), WindowEvent::ScaleFactorChanged {
            scale_factor,
            surface_size_writer: SurfaceSizeWriter::new(Arc::downgrade(&new_surface_size)),
        });
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

    fn stop(&mut self) {
        self.timer.set_next_fire_date(f64::MAX);
    }

    fn start(&mut self) {
        self.timer.set_next_fire_date(f64::MIN);
    }

    fn start_at(&mut self, instant: Instant) {
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
