use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use openharmony_ability::xcomponent::{Action, TouchEvent};
use tracing::{debug, trace, warn};

use openharmony_ability::{
    ime::KeyboardStatus, Configuration, Event as MainEvent, ImeEvent, InputEvent, OpenHarmonyApp,
    OpenHarmonyWaker, Rect,
};

use crate::cursor::Cursor;
use crate::dpi::{PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{self, EventLoopError};
use crate::event::{self, ElementState, Force, Ime, InnerSizeWriter, StartCause};
use crate::event_loop::{self, ActiveEventLoop as RootAEL, ControlFlow, DeviceEvents};
use crate::keyboard::{Key, KeyCode, KeyLocation, NamedKey, PhysicalKey};
use crate::platform::pump_events::PumpStatus;
use crate::window::{
    self, CursorGrabMode, CustomCursor, CustomCursorSource, Fullscreen, ImePurpose,
    ResizeDirection, Theme, WindowButtons, WindowLevel,
};

mod keycodes;

pub(crate) use crate::cursor::{
    NoCustomCursor as PlatformCustomCursor, NoCustomCursor as PlatformCustomCursorSource,
};
pub(crate) use crate::icon::NoIcon as PlatformIcon;

static HAS_FOCUS: AtomicBool = AtomicBool::new(true);
static HAS_EVENT: AtomicBool = AtomicBool::new(false);

struct PeekableReceiver<T> {
    recv: mpsc::Receiver<T>,
    first: Option<T>,
}

impl<T> PeekableReceiver<T> {
    pub fn from_recv(recv: mpsc::Receiver<T>) -> Self {
        Self { recv, first: None }
    }

    pub fn try_recv(&mut self) -> Result<T, mpsc::TryRecvError> {
        if let Some(first) = self.first.take() {
            return Ok(first);
        }
        self.recv.try_recv()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct KeyEventExtra {}

pub struct EventLoop<T: 'static> {
    pub(crate) openharmony_app: OpenHarmonyApp,
    window_target: event_loop::ActiveEventLoop,
    cause: StartCause,
    user_events_sender: mpsc::Sender<T>,
    user_events_receiver: PeekableReceiver<T>,
    event_loop: RefCell<Option<Box<dyn FnMut(event::Event<T>)>>>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct PlatformSpecificEventLoopAttributes {
    pub(crate) openharmony_app: Option<OpenHarmonyApp>,
}

impl Default for PlatformSpecificEventLoopAttributes {
    fn default() -> Self {
        Self { openharmony_app: Default::default() }
    }
}

impl<T: 'static> EventLoop<T> {
    pub(crate) fn new(
        attributes: &PlatformSpecificEventLoopAttributes,
    ) -> Result<Self, EventLoopError> {
        let (user_events_sender, user_events_receiver) = mpsc::channel();

        let openharmony_app = attributes.openharmony_app.as_ref().expect(
            "An `OpenHarmonyApp` as passed to lib is required to create an `EventLoop` on \
             OpenHarmony or HarmonyNext",
        );

        Ok(Self {
            openharmony_app: openharmony_app.clone(),
            window_target: event_loop::ActiveEventLoop {
                p: ActiveEventLoop {
                    app: openharmony_app.clone(),
                    control_flow: Cell::new(ControlFlow::default()),
                    exit: Cell::new(false),
                },
                _marker: PhantomData,
            },
            cause: StartCause::Init,
            user_events_sender,
            user_events_receiver: PeekableReceiver::from_recv(user_events_receiver),
            event_loop: RefCell::new(None),
        })
    }

    pub(crate) fn window_target(&self) -> &event_loop::ActiveEventLoop {
        &self.window_target
    }

    // TODO: For input event, we need some real examples to test it
    fn handle_input_event(&self, event: &InputEvent) {
        match event {
            InputEvent::TouchEvent(motion_event) => {
                let window_id = window::WindowId(WindowId);
                let device_id = event::DeviceId(DeviceId(motion_event.device_id as _));
                let action = motion_event.event_type;

                let phase = match motion_event.event_type {
                    TouchEvent::Down => Some(event::TouchPhase::Started),
                    TouchEvent::Up => Some(event::TouchPhase::Ended),
                    TouchEvent::Move => Some(event::TouchPhase::Moved),
                    TouchEvent::Cancel => Some(event::TouchPhase::Cancelled),
                    _ => {
                        None // TODO mouse events
                    },
                };

                if let Some(phase) = phase {
                    for pointer in motion_event.touch_points.iter() {
                        let position = PhysicalPosition { x: pointer.x as _, y: pointer.y as _ };
                        trace!(
                            "Input event {device_id:?}, {action:?}, loc={position:?}, \
                                 pointer={pointer:?}"
                        );

                        let event = event::Event::WindowEvent {
                            window_id,
                            event: event::WindowEvent::Touch(event::Touch {
                                device_id,
                                phase,
                                location: position,
                                id: pointer.id as u64,
                                force: Some(Force::Normalized(pointer.force as f64)),
                            }),
                        };
                        if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                            h(event);
                        }
                    }
                }
            },
            InputEvent::KeyEvent(key) => {
                match key.code {
                    keycode => {
                        let state = match key.action {
                            Action::Down => event::ElementState::Pressed,
                            Action::Up => event::ElementState::Released,
                            _ => event::ElementState::Released,
                        };

                        let event = event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::KeyboardInput {
                                device_id: event::DeviceId(DeviceId(key.device_id as _)),
                                event: event::KeyEvent {
                                    state,
                                    physical_key: keycodes::to_physical_key(keycode),
                                    logical_key: keycodes::to_logical(keycode),
                                    location: keycodes::to_location(keycode),
                                    // TODO
                                    repeat: false,
                                    text: None,
                                    platform_specific: KeyEventExtra {},
                                },
                                is_synthetic: false,
                            },
                        };
                        if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                            h(event);
                        }
                    },
                }
            },
            InputEvent::ImeEvent(data) => match data {
                ImeEvent::TextInputEvent(s) => {
                    let pre_edit = Ime::Preedit(s.text.clone(), Some((s.text.len(), s.text.len())));

                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Ime(pre_edit),
                        });

                        h(event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Ime(Ime::Commit(s.text.clone())),
                        });
                    }
                },
                ImeEvent::BackspaceEvent(_) => {
                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        // Mock keyboard input event
                        [ElementState::Pressed, ElementState::Released].map(|state| {
                            h(event::Event::WindowEvent {
                                window_id: window::WindowId(WindowId),
                                event: event::WindowEvent::KeyboardInput {
                                    device_id: event::DeviceId(DeviceId(0)),
                                    event: event::KeyEvent {
                                        state,
                                        logical_key: Key::Named(NamedKey::Backspace),
                                        physical_key: PhysicalKey::Code(KeyCode::Backspace),
                                        platform_specific: KeyEventExtra {},
                                        repeat: false,
                                        location: KeyLocation::Standard,
                                        text: None,
                                    },
                                    is_synthetic: false,
                                },
                            });
                        });
                    }
                },

                ImeEvent::ImeStatusEvent(s) => match s {
                    KeyboardStatus::Hide => {
                        if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                            // Mock keyboard input event that make sure egui can receive the event and trigger onblur event
                            [ElementState::Pressed, ElementState::Released].map(|state| {
                                h(event::Event::WindowEvent {
                                    window_id: window::WindowId(WindowId),
                                    event: event::WindowEvent::KeyboardInput {
                                        device_id: event::DeviceId(DeviceId(0)),
                                        event: event::KeyEvent {
                                            state,
                                            logical_key: Key::Named(NamedKey::Enter),
                                            physical_key: PhysicalKey::Code(KeyCode::Enter),
                                            platform_specific: KeyEventExtra {},
                                            repeat: false,
                                            location: KeyLocation::Standard,
                                            text: None,
                                        },
                                        is_synthetic: false,
                                    },
                                });
                            });

                            h(event::Event::WindowEvent {
                                window_id: window::WindowId(WindowId),
                                event: event::WindowEvent::Ime(Ime::Disabled),
                            });
                        }
                    },
                    _ => {
                        warn!("Unknown openharmony_ability ime status event {s:?}")
                    },
                },
            },
            _ => {
                warn!("Unknown openharmony_ability input event {event:?}")
            },
        }
    }

    pub fn run<F>(self, event_handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(event::Event<T>, &event_loop::ActiveEventLoop),
    {
        let event_looper = Box::leak(Box::new(self));
        event_looper.run_on_demand(event_handler)
    }

    pub fn run_on_demand<F>(&mut self, event_handler: F) -> Result<(), EventLoopError>
    where
        F: FnMut(event::Event<T>, &event_loop::ActiveEventLoop),
    {
        match self.pump_events(None, event_handler) {
            PumpStatus::Continue => Ok(()),
            PumpStatus::Exit(code) => Err(EventLoopError::ExitFailure(code)),
        }
    }

    pub fn pump_events<F>(&mut self, _timeout: Option<Duration>, mut event_handle: F) -> PumpStatus
    where
        F: FnMut(event::Event<T>, &RootAEL),
    {
        if HAS_EVENT.load(Ordering::SeqCst) {
            trace!("EventLoop is already running");
        }
        trace!("Mainloop iteration");
        let cause = self.cause;
        let target = RootAEL { p: self.window_target.p.clone(), _marker: PhantomData };

        {
            let handle = unsafe {
                std::mem::transmute::<
                    Box<dyn FnMut(event::Event<T>)>,
                    Box<dyn FnMut(event::Event<T>)>,
                >(Box::new(move |e| {
                    event_handle(e, &target);
                    // We need to dispatch it after every event callbacks.
                    event_handle(event::Event::AboutToWait, &target);
                }))
            };
            self.event_loop.replace(Some(handle));
            if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                h(event::Event::NewEvents(cause));
            }
        }

        self.openharmony_app.clone().run_loop(|event| {
            match event {
                MainEvent::SurfaceCreate { .. } => {
                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event::Event::Resumed);
                    }
                },
                MainEvent::SurfaceDestroy { .. } => {
                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event::Event::Suspended);
                    }
                },
                MainEvent::WindowResize { .. } => {
                    let win = self.openharmony_app.native_window();
                    let size = if let Some(win) = win {
                        PhysicalSize::new(win.width() as _, win.height() as _)
                    } else {
                        PhysicalSize::new(0, 0)
                    };
                    let event = event::Event::WindowEvent {
                        window_id: window::WindowId(WindowId),
                        event: event::WindowEvent::Resized(size),
                    };

                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event);
                    }
                },
                MainEvent::WindowRedraw { .. } => {
                    let event = event::Event::WindowEvent {
                        window_id: window::WindowId(WindowId),
                        event: event::WindowEvent::RedrawRequested,
                    };

                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event);
                    }
                },
                MainEvent::ContentRectChange { .. } => {
                    warn!("TODO: find a way to notify application of content rect change");
                },
                MainEvent::GainedFocus => {
                    HAS_FOCUS.store(true, Ordering::Relaxed);

                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Focused(true),
                        });
                    }
                },
                MainEvent::LostFocus => {
                    HAS_FOCUS.store(false, Ordering::Relaxed);

                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Focused(true),
                        });
                    }
                },
                MainEvent::ConfigChanged { .. } => {
                    let win = self.openharmony_app.native_window();
                    if let Some(win) = win {
                        let scale = self.openharmony_app.scale();
                        let width = win.width();
                        let height = win.height();
                        let new_surface_size =
                            Arc::new(Mutex::new(PhysicalSize::new(width as _, height as _)));
                        let event = event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::ScaleFactorChanged {
                                inner_size_writer: InnerSizeWriter::new(Arc::downgrade(
                                    &new_surface_size,
                                )),
                                scale_factor: scale as _,
                            },
                        };

                        if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                            h(event);
                        }
                    }
                },
                MainEvent::LowMemory => {
                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event::Event::MemoryWarning);
                    }
                },
                MainEvent::Start => {
                    // XXX: how to forward this state to applications?
                    warn!("TODO: forward onStart notification to application");
                },
                MainEvent::Resume { .. } => {
                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        h(event::Event::Resumed);
                    }
                },
                MainEvent::SaveState { .. } => {
                    // XXX: how to forward this state to applications?
                    // XXX: also how do we expose state restoration to apps?
                    warn!("TODO: forward saveState notification to application");
                },
                MainEvent::Pause => {
                    debug!("App Paused - stopped running");
                    // TODO: This is incorrect - will be solved in https://github.com/rust-windowing/winit/pull/3897
                    // self.running = false;
                },
                MainEvent::WindowDestroy => {
                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        let e = event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::CloseRequested,
                        };
                        h(e);
                    }
                },
                MainEvent::Destroy => {
                    // XXX: maybe exit mainloop to drop things before being
                    // killed by the OS?
                    warn!("TODO: forward onDestroy notification to application");
                },
                MainEvent::Input(input_event) => {
                    self.handle_input_event(&input_event);
                },
                MainEvent::UserEvent { .. } => {
                    if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                        if let Ok(event) = self.user_events_receiver.try_recv() {
                            let event = event::Event::UserEvent(event);
                            h(event);
                        }
                    }
                },
                unknown => {
                    trace!("Unknown MainEvent {unknown:?} (ignored)");
                },
            };

            if self.window_target.p.exit.get() {
                if let Some(ref mut h) = *self.event_loop.borrow_mut() {
                    h(event::Event::LoopExiting);
                    self.openharmony_app.exit(0);
                }
            }
        });

        PumpStatus::Continue
    }

    pub fn create_proxy(&self) -> EventLoopProxy<T> {
        EventLoopProxy {
            user_events_sender: self.user_events_sender.clone(),
            waker: self.openharmony_app.create_waker(),
        }
    }
}

pub struct EventLoopProxy<T: 'static> {
    user_events_sender: mpsc::Sender<T>,
    waker: OpenHarmonyWaker,
}

impl<T: 'static> EventLoopProxy<T> {
    pub fn send_event(&self, event: T) -> Result<(), event_loop::EventLoopClosed<T>> {
        self.user_events_sender.send(event).map_err(|err| event_loop::EventLoopClosed(err.0))?;
        self.waker.wake();
        Ok(())
    }
}

impl<T: 'static> Clone for EventLoopProxy<T> {
    fn clone(&self) -> Self {
        EventLoopProxy {
            user_events_sender: self.user_events_sender.clone(),
            waker: self.waker.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ActiveEventLoop {
    pub(crate) app: OpenHarmonyApp,
    control_flow: Cell<ControlFlow>,
    exit: Cell<bool>,
}

impl ActiveEventLoop {
    pub fn create_custom_cursor(&self, source: CustomCursorSource) -> CustomCursor {
        let _ = source.inner;
        CustomCursor { inner: PlatformCustomCursor }
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle::new(self.app.clone()));
        v
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(self.app.clone()))
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        unreachable!("rwh_05 is not supported on OpenHarmony");
    }

    pub fn system_theme(&self) -> Option<Theme> {
        None
    }

    pub fn listen_device_events(&self, _allowed: DeviceEvents) {}

    pub fn set_control_flow(&self, control_flow: ControlFlow) {
        self.control_flow.set(control_flow)
    }

    pub fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
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

    pub(crate) fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Ohos(rwh_06::OhosDisplayHandle::new()))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct OwnedDisplayHandle;

impl OwnedDisplayHandle {
    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        unreachable!("rwh_05 is not supported on OpenHarmony");
    }

    #[cfg(feature = "rwh_06")]
    #[inline]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::OhosDisplayHandle::new().into())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct WindowId;

impl WindowId {
    pub const fn dummy() -> Self {
        WindowId
    }
}

impl From<WindowId> for u64 {
    fn from(_: WindowId) -> Self {
        0
    }
}

impl From<u64> for WindowId {
    fn from(_: u64) -> Self {
        Self
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceId(i32);

impl DeviceId {
    pub const fn dummy() -> Self {
        DeviceId(0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSpecificWindowAttributes;

pub(crate) struct Window {
    app: OpenHarmonyApp,
}

impl Window {
    pub(crate) fn new(
        el: &ActiveEventLoop,
        _window_attrs: window::WindowAttributes,
    ) -> Result<Self, error::OsError> {
        // FIXME this ignores requested window attributes

        Ok(Self { app: el.app.clone() })
    }

    pub fn request_redraw(&self) {}

    pub(crate) fn maybe_queue_on_main(&self, f: impl FnOnce(&Self) + Send + 'static) {
        f(self)
    }

    pub(crate) fn maybe_wait_on_main<R: Send>(&self, f: impl FnOnce(&Self) -> R + Send) -> R {
        f(self)
    }

    pub fn id(&self) -> WindowId {
        WindowId
    }

    pub fn scale_factor(&self) -> f64 {
        self.app.scale() as f64
    }

    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(self.app.clone()))
    }

    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle::new(self.app.clone()));
        v
    }
    pub fn current_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle::new(self.app.clone()))
    }

    pub fn pre_present_notify(&self) {}

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.outer_size()
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    pub fn set_outer_position(&self, _position: Position) {
        // no effect
    }

    pub fn request_inner_size(&self, _size: Size) -> Option<PhysicalSize<u32>> {
        Some(self.inner_size())
    }

    pub fn outer_size(&self) -> PhysicalSize<u32> {
        MonitorHandle::new(self.app.clone()).size()
    }

    pub fn set_min_inner_size(&self, _: Option<Size>) {}

    pub fn set_max_inner_size(&self, _: Option<Size>) {}

    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    pub fn set_resize_increments(&self, _increments: Option<Size>) {}

    pub fn set_title(&self, _title: &str) {}

    pub fn set_transparent(&self, _transparent: bool) {}

    pub fn set_blur(&self, _blur: bool) {}

    pub fn set_visible(&self, _visibility: bool) {}

    pub fn is_visible(&self) -> Option<bool> {
        None
    }

    pub fn set_resizable(&self, _resizeable: bool) {}

    pub fn is_resizable(&self) -> bool {
        false
    }

    pub fn set_enabled_buttons(&self, _buttons: WindowButtons) {}

    pub fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    pub fn set_minimized(&self, _minimized: bool) {}

    pub fn is_minimized(&self) -> Option<bool> {
        None
    }

    pub fn set_maximized(&self, _maximized: bool) {}

    pub fn is_maximized(&self) -> bool {
        false
    }

    pub fn set_fullscreen(&self, _monitor: Option<Fullscreen>) {
        warn!("Cannot set fullscreen on HarmonyOS");
    }

    pub fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    pub fn set_decorations(&self, _decorations: bool) {}

    pub fn is_decorated(&self) -> bool {
        true
    }

    pub fn set_window_level(&self, _level: WindowLevel) {}

    pub fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {}

    pub fn set_ime_cursor_area(&self, _position: Position, _size: Size) {}

    pub fn set_ime_allowed(&self, allowed: bool) {
        if allowed {
            self.app.show_keyboard();
        } else {
            self.app.hide_keyboard();
        }
    }

    pub fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    pub fn focus_window(&self) {}

    pub fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    pub fn set_cursor(&self, _: Cursor) {}

    pub fn set_cursor_position(&self, _: Position) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    pub fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    pub fn set_cursor_visible(&self, _: bool) {}
    pub fn drag_window(&self) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    pub fn drag_resize_window(
        &self,
        _direction: ResizeDirection,
    ) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    #[inline]
    pub fn show_window_menu(&self, _position: Position) {}

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    pub fn set_theme(&self, _theme: Option<Theme>) {}

    pub fn theme(&self) -> Option<Theme> {
        None
    }

    pub fn set_content_protected(&self, _protected: bool) {}

    pub fn has_focus(&self) -> bool {
        HAS_FOCUS.load(Ordering::Relaxed)
    }

    pub fn title(&self) -> String {
        String::new()
    }

    pub fn reset_dead_keys(&self) {}

    #[cfg(feature = "rwh_04")]
    pub fn raw_window_handle_rwh_04(&self) -> rwh_04::RawWindowHandle {
        unreachable!("rwh_04 is not supported on OpenHarmony");
    }

    #[cfg(feature = "rwh_05")]
    pub fn raw_window_handle_rwh_05(&self) -> rwh_05::RawWindowHandle {
        unreachable!("rwh_05 is not supported on OpenHarmony");
    }

    #[cfg(feature = "rwh_05")]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        unreachable!("rwh_05 is not supported on OpenHarmony");
    }

    #[cfg(feature = "rwh_06")]
    // Allow the usage of HasRawWindowHandle inside this function
    #[allow(deprecated)]
    pub fn raw_window_handle_rwh_06(&self) -> Result<rwh_06::RawWindowHandle, rwh_06::HandleError> {
        if let Some(native_window) = self.app.native_window().as_ref() {
            if let Some(win) = native_window.raw_window_handle() {
                return Ok(win);
            }
            tracing::error!(
                "Cannot get the native window, it's null and will always be null before \
                 Event::Resumed and after Event::Suspended. Make sure you only call this function \
                 between those events."
            );
            Err(rwh_06::HandleError::Unavailable)
        } else {
            tracing::error!(
                "Cannot get the native window, it's null and will always be null before \
                 Event::Resumed and after Event::Suspended. Make sure you only call this function \
                 between those events."
            );
            Err(rwh_06::HandleError::Unavailable)
        }
    }

    #[cfg(feature = "rwh_06")]
    pub fn raw_display_handle_rwh_06(
        &self,
    ) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Ohos(rwh_06::OhosDisplayHandle::new()))
    }

    pub fn config(&self) -> Configuration {
        self.app.config()
    }

    pub fn content_rect(&self) -> Rect {
        self.app.content_rect()
    }
}

#[derive(Default, Clone, Debug)]
pub struct OsError;

use std::fmt::{self, Display, Formatter};
impl Display for OsError {
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        write!(fmt, "OpenHarmony OS Error")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MonitorHandle {
    app: OpenHarmonyApp,
}

impl MonitorHandle {
    pub(crate) fn new(app: OpenHarmonyApp) -> Self {
        Self { app }
    }

    pub fn name(&self) -> Option<String> {
        Some("OpenHarmony Device".to_owned())
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        if let Some(native_window) = self.app.native_window() {
            PhysicalSize::new(native_window.width() as _, native_window.height() as _)
        } else {
            PhysicalSize::new(0, 0)
        }
    }

    pub fn position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    pub fn scale_factor(&self) -> f64 {
        self.app.scale() as f64
    }

    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        // FIXME no way to get real refresh rate for now.
        None
    }

    pub fn video_modes(&self) -> impl Iterator<Item = VideoModeHandle> {
        let size = self.size().into();
        // FIXME this is not the real refresh rate
        // (it is guaranteed to support 32 bit color though)
        std::iter::once(VideoModeHandle {
            size,
            bit_depth: 32,
            refresh_rate_millihertz: 60000,
            monitor: self.clone(),
        })
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct VideoModeHandle {
    size: (u32, u32),
    bit_depth: u16,
    refresh_rate_millihertz: u32,
    monitor: MonitorHandle,
}

impl VideoModeHandle {
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size.into()
    }

    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.monitor.clone()
    }
}
