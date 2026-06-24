use std::any::Any;
use std::cell::{Cell, Ref, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use std::{fmt, mem, panic};

use dpi::PhysicalSize;
use windows_sys::Win32::Foundation::{DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, HWND};
use windows_sys::Win32::System::Ole::{
    DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, DROPEFFECT_NONE, DoDragDrop,
};
use winit_core::application::ApplicationHandler;
use winit_core::data_transfer::DataTransferId;
use winit_core::event::{DeviceEvent, DeviceId, StartCause, SurfaceSizeWriter, WindowEvent};
use winit_core::event_loop::{ActiveEventLoop as RootActiveEventLoop, DndAction};
use winit_core::window::WindowId;

use super::{ActiveEventLoop, ControlFlow, EventLoopThreadExecutor};
use crate::dnd::{DataObject, DropEffect, DropSource, SourceDataObject, drop_effect_to_dnd_action};
use crate::event_loop::{GWL_USERDATA, WindowData};
use crate::util::get_window_long;

type EventHandler = Cell<Option<&'static mut (dyn ApplicationHandler + 'static)>>;

/// State for the single drag-and-drop transfer currently in flight (OLE guarantees at most one
/// active drag per process).
#[derive(Debug)]
pub(super) struct DragState {
    pub(super) id: DataTransferId,
    pub(super) window_id: WindowId,
    pub(super) data: Arc<DataObject>,
    pub(super) actions: Vec<DndAction>,
}

pub(super) struct PendingDrag {
    pub(super) window_id: WindowId,
    pub(super) data_object: SourceDataObject,
    pub(super) drop_source: DropSource,
    pub(super) allowed_effects: DropEffect,
    pub(super) id: DataTransferId,
}

/// Set while `DoDragDrop` is on the call stack - i.e., this process is the source of an active
/// drag. The target-side `IDropTarget` checks this to recognize self-drops and reuse the source's
/// id + allowed actions instead of waiting for the (buffered) app `DragEntered` handler.
#[derive(Copy, Clone)]
pub(crate) struct SourceDrag {
    pub(crate) id: DataTransferId,
}

pub(crate) struct EventLoopRunner {
    pub(super) thread_id: u32,

    // The event loop's win32 handles
    pub(super) thread_msg_target: HWND,

    // Setting this will ensure pump_events will return to the external
    // loop asap. E.g. set after each RedrawRequested to ensure pump_events
    // can't stall an external loop beyond a frame
    pub(super) interrupt_msg_dispatch: Cell<bool>,

    control_flow: Cell<ControlFlow>,
    exit: Cell<Option<i32>>,
    runner_state: Cell<RunnerState>,
    last_events_cleared: Cell<Instant>,
    event_handler: Rc<EventHandler>,
    event_buffer: RefCell<VecDeque<Event>>,

    /// The currently in-flight drag transfer, if any, alive between `DragEntered` and
    /// `DragLeft`/`DragDropped`.
    pub(super) drag_state: RefCell<Option<DragState>>,

    /// `Some(_)` while `start_drag` has `DoDragDrop` on the call stack.
    pub(crate) source_drag: Cell<Option<SourceDrag>>,

    /// `DoDragDrop` is blocking and synchronous, so we wait until after the application returns
    /// control to winit before actually calling into the OS to initiate the drag. This prevents
    /// the event loop from being re-entrant if we are doing an internal drag operation, since if
    /// we handled this inside `ActiveEventLoop::start_drag` then all the `WindowEvent::Drag*`
    /// events would be buffered until `DoDragDrop` returns, preventing the application from
    /// handling those messages.
    pub(super) pending_drag: RefCell<Option<PendingDrag>>,

    /// For self-drops, target-side `IDropTarget::Drop` can't release the cached `DragState`
    /// before its `DragDropped` `WindowEvent` is delivered - the event is buffered (the outer app
    /// handler holds `event_handler` for the duration of `DoDragDrop`) and `data_transfer(id)`
    /// would return `UnknownDataTransfer` if cleanup ran synchronously. So we stash the id here
    /// and drain it at the end of `dispatch_buffered_events`, after the app's buffered handler
    /// has had its chance to read the data.
    pending_source_drag_cleanup: Cell<Option<DataTransferId>>,

    panic_error: Cell<Option<PanicError>>,
}

impl fmt::Debug for EventLoopRunner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLoopRunner")
            .field("thread_msg_target", &self.thread_msg_target)
            .finish_non_exhaustive()
    }
}

pub type PanicError = Box<dyn Any + Send + 'static>;

/// See `move_state_to` function for details on how the state loop works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RunnerState {
    /// The event loop has just been created, and an `Init` event must be sent.
    Uninitialized,
    /// The event loop is idling.
    Idle,
    /// The event loop is handling the OS's events and sending them to the user's callback.
    /// `NewEvents` has been sent, and `AboutToWait` hasn't.
    HandlingMainEvents,
    /// The event loop has been destroyed. No other events will be emitted.
    Destroyed,
}

#[derive(Debug, Clone)]
pub(crate) enum Event {
    Device { device_id: DeviceId, event: DeviceEvent },
    Window { window_id: WindowId, event: WindowEvent },
    BufferedScaleFactorChanged(HWND, f64, PhysicalSize<u32>),
    // FIXME(madsmtm): Coalesce these into a flag (or similar) instead of handling them as events.
    // https://github.com/rust-windowing/winit/pull/3687
    WakeUp,
}

impl EventLoopRunner {
    pub(crate) fn new(thread_id: u32, thread_msg_target: HWND) -> Self {
        Self {
            thread_id,
            thread_msg_target,
            interrupt_msg_dispatch: Cell::new(false),
            runner_state: Cell::new(RunnerState::Uninitialized),
            control_flow: Cell::new(ControlFlow::default()),
            exit: Cell::new(None),
            panic_error: Cell::new(None),
            last_events_cleared: Cell::new(Instant::now()),
            event_handler: Rc::new(Cell::new(None)),
            event_buffer: RefCell::new(VecDeque::new()),
            drag_state: RefCell::new(None),
            source_drag: Cell::new(None),
            pending_drag: RefCell::new(None),
            pending_source_drag_cleanup: Cell::new(None),
        }
    }

    pub(super) fn try_execute_drag_drop(self: &Rc<Self>) {
        let Some(PendingDrag { data_object, drop_source, id, allowed_effects, window_id }) =
            self.pending_drag.take()
        else {
            return;
        };

        // Make the drag visible to our own target-side `IDropTarget` so it can recognize
        // self-drops and reuse this id + action mask without going through the (buffered) app
        // handler. The guard ensures the flag is cleared on any exit path - if anything between
        // here and `DoDragDrop`'s return panics, the stale flag would otherwise permanently
        // disable `WM_PAINT` dispatch and misclassify all future external drags as self-drops.
        struct ClearOnDrop<'a>(&'a Cell<Option<SourceDrag>>);
        impl Drop for ClearOnDrop<'_> {
            fn drop(&mut self) {
                self.0.set(None);
            }
        }
        self.source_drag.set(Some(SourceDrag { id }));
        let _guard = ClearOnDrop(&self.source_drag);

        let mut effect_out: u32 = DROPEFFECT_NONE;
        let hr = unsafe {
            DoDragDrop(
                data_object.interface_ptr(),
                drop_source.interface_ptr(),
                allowed_effects,
                &mut effect_out,
            )
        };

        if hr == DRAGDROP_S_DROP {
            let action = drop_effect_to_dnd_action(effect_out);

            self.send_event(Event::Window {
                window_id,
                event: WindowEvent::OutgoingDragDropped { id, action },
            });
        } else if hr == DRAGDROP_S_CANCEL {
            self.send_event(Event::Window {
                window_id,
                event: WindowEvent::OutgoingDragCanceled { id },
            });
        } else {
            tracing::error!("DoDragDrop failed: 0x{hr:08x}");
            return;
        }

        // Both `DRAGDROP_S_DROP` and `DRAGDROP_S_CANCEL` are success codes for us - the app
        // will hear about the outcome via the buffered `DragDropped`/`DragLeft` events
        // (target-side translates `effect_out == DROPEFFECT_NONE` to `DragLeft`).
        // Log the negotiated effect so cross-process drops, which have no target-side event
        // in this process, leave a debuggable trace of what action the remote target performed.
        tracing::trace!(
            "DoDragDrop completed: hr=0x{hr:08x} effect_out={effect_out} (COPY={DROPEFFECT_COPY}, \
             MOVE={DROPEFFECT_MOVE}, LINK={DROPEFFECT_LINK})",
        );
    }

    pub(crate) fn defer_source_drag_cleanup(&self, id: DataTransferId) {
        self.pending_source_drag_cleanup.set(Some(id));
    }

    pub(crate) fn register_data_transfer(
        &self,
        id: DataTransferId,
        window_id: WindowId,
        data: Arc<DataObject>,
    ) {
        // By default, no actions have been set as valid by the target.
        *self.drag_state.borrow_mut() =
            Some(DragState { id, window_id, data, actions: Default::default() });
    }

    pub(crate) fn remove_data_transfer(&self, id: DataTransferId) {
        let mut state = self.drag_state.borrow_mut();
        if state.as_ref().is_some_and(|s| s.id == id) {
            *state = None;
        }
    }

    pub(super) fn drag_state(&self, id: DataTransferId) -> Option<Ref<'_, DragState>> {
        Ref::filter_map(self.drag_state.borrow(), |state| state.as_ref().filter(|s| s.id == id))
            .ok()
    }

    pub(crate) fn current_drag_actions(&self, id: DataTransferId) -> Ref<'_, [DndAction]> {
        Ref::map(self.drag_state.borrow(), |state| {
            state.as_ref().filter(|s| s.id == id).map(|s| &s.actions[..]).unwrap_or_default()
        })
    }

    pub(crate) fn proposed_dnd_action(
        &self,
        id: DataTransferId,
        effects: DropEffect,
    ) -> Option<DndAction> {
        self.current_drag_actions(id).iter().copied().find(|action| {
            let effect = match action {
                DndAction::Move => DROPEFFECT_MOVE,
                DndAction::Copy => DROPEFFECT_COPY,
                DndAction::Link => DROPEFFECT_LINK,
                _ => return false,
            };

            (effects | effect) != 0
        })
    }

    /// Associate the application's event handler with the runner.
    ///
    /// # Safety
    ///
    /// The returned type must not be leaked (as that would allow the application to be associated
    /// with the runner for too long).
    pub(crate) unsafe fn set_app<'app>(
        &self,
        app: &'app mut (dyn ApplicationHandler + 'app),
    ) -> impl Drop + 'app {
        // Erase app lifetime, to allow storing on the event loop runner.
        //
        // SAFETY: Caller upholds that the lifetime of the closure is upheld, by not dropping the
        // return type which resets it.
        let f = unsafe {
            mem::transmute::<
                &'app mut (dyn ApplicationHandler + 'app),
                &'static mut (dyn ApplicationHandler + 'static),
            >(app)
        };

        let old_event_handler = self.event_handler.replace(Some(f));

        assert!(old_event_handler.is_none());

        struct Resetter(Rc<EventHandler>);

        impl Drop for Resetter {
            fn drop(&mut self) {
                self.0.set(None);
            }
        }

        Resetter(self.event_handler.clone())
    }

    pub(crate) fn reset_runner(&self) {
        let Self {
            thread_id: _,
            thread_msg_target: _,
            interrupt_msg_dispatch,
            runner_state,
            panic_error,
            control_flow: _,
            exit,
            last_events_cleared: _,
            event_handler,
            event_buffer: _,
            drag_state,
            source_drag,
            pending_drag,
            pending_source_drag_cleanup,
        } = self;
        interrupt_msg_dispatch.set(false);
        runner_state.set(RunnerState::Uninitialized);
        panic_error.set(None);
        exit.set(None);
        event_handler.set(None);
        drag_state.take();
        source_drag.set(None);
        pending_drag.take();
        pending_source_drag_cleanup.set(None);
    }
}

/// State retrieval functions.
impl EventLoopRunner {
    #[allow(unused)]
    pub fn thread_msg_target(&self) -> HWND {
        self.thread_msg_target
    }

    pub fn take_panic_error(&self) -> Result<(), PanicError> {
        match self.panic_error.take() {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.control_flow.set(control_flow)
    }

    pub fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }

    pub fn set_exit_code(&self, code: i32) {
        self.exit.set(Some(code))
    }

    pub fn exit_code(&self) -> Option<i32> {
        self.exit.get()
    }

    pub fn clear_exit(&self) {
        self.exit.set(None);
    }

    pub fn should_buffer(&self) -> bool {
        let handler = self.event_handler.take();
        let should_buffer = handler.is_none();
        self.event_handler.set(handler);
        should_buffer
    }
}

/// Misc. functions
impl EventLoopRunner {
    pub fn catch_unwind<R>(&self, f: impl FnOnce() -> R) -> Option<R> {
        let panic_error = self.panic_error.take();
        if panic_error.is_none() {
            let result = panic::catch_unwind(panic::AssertUnwindSafe(f));

            // Check to see if the panic error was set in a re-entrant call to catch_unwind inside
            // of `f`. If it was, that error takes priority. If it wasn't, check if our call to
            // catch_unwind caught any panics and set panic_error appropriately.
            match self.panic_error.take() {
                None => match result {
                    Ok(r) => Some(r),
                    Err(e) => {
                        self.panic_error.set(Some(e));
                        None
                    },
                },
                Some(e) => {
                    self.panic_error.set(Some(e));
                    None
                },
            }
        } else {
            self.panic_error.set(panic_error);
            None
        }
    }

    #[inline(always)]
    pub(crate) fn create_thread_executor(&self) -> EventLoopThreadExecutor {
        EventLoopThreadExecutor { thread_id: self.thread_id, target_window: self.thread_msg_target }
    }
}

/// Event dispatch functions.
impl EventLoopRunner {
    pub(crate) fn prepare_wait(self: &Rc<Self>) {
        self.move_state_to(RunnerState::Idle);
    }

    pub(crate) fn wakeup(self: &Rc<Self>) {
        self.move_state_to(RunnerState::HandlingMainEvents);
    }

    pub(crate) fn send_event(self: &Rc<Self>, event: Event) {
        if let Event::Window { event: WindowEvent::RedrawRequested, .. } = event {
            self.call_event_handler(|app, event_loop| event.dispatch_event(app, event_loop));
            // As a rule, to ensure that `pump_events` can't block an external event loop
            // for too long, we always guarantee that `pump_events` will return control to
            // the external loop asap after a `RedrawRequested` event is dispatched.
            self.interrupt_msg_dispatch.set(true);
        } else if self.should_buffer() {
            // If the runner is already borrowed, we're in the middle of an event loop invocation.
            // Add the event to a buffer to be processed later.
            self.event_buffer.borrow_mut().push_back(event.buffer_scale_factor())
        } else {
            self.call_event_handler(|app, event_loop| event.dispatch_event(app, event_loop));
            self.dispatch_buffered_events();
        }
    }

    pub(crate) fn loop_destroyed(self: &Rc<Self>) {
        self.move_state_to(RunnerState::Destroyed);
    }

    fn call_event_handler(
        self: &Rc<Self>,
        closure: impl FnOnce(&mut dyn ApplicationHandler, &dyn RootActiveEventLoop),
    ) {
        self.catch_unwind(|| {
            let event_handler = self.event_handler.take().expect(
                "either event handler is re-entrant (likely), or no event handler is registered \
                 (very unlikely)",
            );

            closure(event_handler, ActiveEventLoop::from_ref(self));

            assert!(self.event_handler.replace(Some(event_handler)).is_none());
        });
    }

    fn dispatch_buffered_events(self: &Rc<Self>) {
        loop {
            // We do this instead of using a `while let` loop because if we use a `while let`
            // loop the reference returned `borrow_mut()` doesn't get dropped until the end
            // of the loop's body and attempts to add events to the event buffer while in
            // `process_event` will fail.
            let buffered_event_opt = self.event_buffer.borrow_mut().pop_front();
            match buffered_event_opt {
                Some(e) => {
                    self.call_event_handler(|app, event_loop| e.dispatch_event(app, event_loop))
                },
                None => break,
            }
        }
        // The app's buffered `DragDropped` handler (if any) has now had its chance to call
        // `data_transfer(id)`; safe to release the cached `DragState` for a deferred self-drop.
        if let Some(id) = self.pending_source_drag_cleanup.take() {
            self.remove_data_transfer(id);
        }
    }

    /// Dispatch control flow events (`NewEvents`, `AboutToWait`, and
    /// `LoopExiting`) as necessary to bring the internal `RunnerState` to the
    /// new runner state.
    ///
    /// The state transitions are defined as follows:
    ///
    /// ```text
    ///    Uninitialized
    ///          |
    ///          V
    ///        Idle
    ///       ^    |
    ///       |    V
    /// HandlingMainEvents
    ///         |
    ///         V
    ///     Destroyed
    /// ```
    ///
    /// Attempting to transition back to `Uninitialized` will result in a panic. Attempting to
    /// transition *from* `Destroyed` will also result in a panic. Transitioning to the current
    /// state is a no-op. Even if the `new_runner_state` isn't the immediate next state in the
    /// runner state machine (e.g. `self.runner_state == HandlingMainEvents` and
    /// `new_runner_state == Idle`), the intermediate state transitions will still be executed.
    fn move_state_to(self: &Rc<Self>, new_runner_state: RunnerState) {
        use RunnerState::{Destroyed, HandlingMainEvents, Idle, Uninitialized};

        match (self.runner_state.replace(new_runner_state), new_runner_state) {
            (Uninitialized, Uninitialized)
            | (Idle, Idle)
            | (HandlingMainEvents, HandlingMainEvents)
            | (Destroyed, Destroyed) => (),

            // State transitions that initialize the event loop.
            (Uninitialized, HandlingMainEvents) => {
                self.call_new_events(true);
            },
            (Uninitialized, Idle) => {
                self.call_new_events(true);
                self.call_event_handler(|app, event_loop| app.about_to_wait(event_loop));
                self.last_events_cleared.set(Instant::now());
            },
            (Uninitialized, Destroyed) => {
                self.call_new_events(true);
                self.call_event_handler(|app, event_loop| app.about_to_wait(event_loop));
                self.last_events_cleared.set(Instant::now());
            },
            (_, Uninitialized) => panic!("cannot move state to Uninitialized"),

            // State transitions that start the event handling process.
            (Idle, HandlingMainEvents) => {
                self.call_new_events(false);
            },
            (Idle, Destroyed) => {},

            (HandlingMainEvents, Idle) => {
                // This is always the last event we dispatch before waiting for new events
                self.call_event_handler(|app, event_loop| app.about_to_wait(event_loop));
                self.last_events_cleared.set(Instant::now());
            },
            (HandlingMainEvents, Destroyed) => {
                self.call_event_handler(|app, event_loop| app.about_to_wait(event_loop));
                self.last_events_cleared.set(Instant::now());
            },

            (Destroyed, _) => panic!("cannot move state from Destroyed"),
        }
    }

    fn call_new_events(self: &Rc<Self>, init: bool) {
        let start_cause = match (init, self.control_flow(), self.exit.get()) {
            (true, ..) => StartCause::Init,
            (false, ControlFlow::Poll, None) => StartCause::Poll,
            (false, _, Some(_)) | (false, ControlFlow::Wait, None) => StartCause::WaitCancelled {
                requested_resume: None,
                start: self.last_events_cleared.get(),
            },
            (false, ControlFlow::WaitUntil(requested_resume), None) => {
                if Instant::now() < requested_resume {
                    StartCause::WaitCancelled {
                        requested_resume: Some(requested_resume),
                        start: self.last_events_cleared.get(),
                    }
                } else {
                    StartCause::ResumeTimeReached {
                        requested_resume,
                        start: self.last_events_cleared.get(),
                    }
                }
            },
        };
        self.call_event_handler(|app, event_loop| app.new_events(event_loop, start_cause));
        // NB: For consistency all platforms must call `can_create_surfaces` even though Windows
        // applications don't themselves have a formal surface destroy/create lifecycle.
        if init {
            self.call_event_handler(|app, event_loop| app.can_create_surfaces(event_loop));
        }
        self.dispatch_buffered_events();
    }
}

impl Event {
    /// Mark ScaleFactorChanged as being buffered (which forces us to re-handle when the user set a
    /// new size).
    pub fn buffer_scale_factor(self) -> Self {
        match self {
            Self::Window {
                event: WindowEvent::ScaleFactorChanged { scale_factor, surface_size_writer },
                window_id,
            } => Event::BufferedScaleFactorChanged(
                window_id.into_raw() as HWND,
                scale_factor,
                surface_size_writer.surface_size().unwrap(),
            ),
            event => event,
        }
    }

    pub fn dispatch_event(
        self,
        app: &mut dyn ApplicationHandler,
        event_loop: &dyn RootActiveEventLoop,
    ) {
        match self {
            Self::Window { window_id, event } => app.window_event(event_loop, window_id, event),
            Self::Device { device_id, event } => {
                app.device_event(event_loop, Some(device_id), event)
            },
            Self::BufferedScaleFactorChanged(window, scale_factor, new_surface_size) => {
                let user_new_surface_size = Arc::new(Mutex::new(new_surface_size));
                app.window_event(
                    event_loop,
                    WindowId::from_raw(window as usize),
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        surface_size_writer: SurfaceSizeWriter::new(Arc::downgrade(
                            &user_new_surface_size,
                        )),
                    },
                );
                let surface_size = *user_new_surface_size.lock().unwrap();

                drop(user_new_surface_size);

                if surface_size != new_surface_size {
                    let window_flags = unsafe {
                        let userdata = get_window_long(window, GWL_USERDATA) as *mut WindowData;
                        (*userdata).window_state_lock().window_flags
                    };

                    window_flags.set_size(window, surface_size);
                }
            },
            Self::WakeUp => app.proxy_wake_up(event_loop),
        }
    }
}
