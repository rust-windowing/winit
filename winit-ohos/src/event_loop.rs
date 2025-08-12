use std::cell::Cell;
use std::fmt;
use std::hash::Hash;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use openharmony_ability::xcomponent::{Action, KeyCode as Keycode, TouchEvent};
use tracing::{debug, trace, warn};

use openharmony_ability::{
    ime::KeyboardStatus, Configuration, Event as MainEvent, ImeEvent, InputEvent, OpenHarmonyApp,
    OpenHarmonyWaker, Rect,
};

use dpi::{PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use winit_core::application::ApplicationHandler;
use winit_core::cursor::{Cursor, CustomCursor, CustomCursorSource};
use winit_core::error::{EventLoopError, NotSupportedError, RequestError};
use winit_core::event::{
    self, DeviceId, ElementState, FingerId, Force, Ime, StartCause, SurfaceSizeWriter,
};
use winit_core::event_loop::pump_events::PumpStatus;
use winit_core::event_loop::{
    ActiveEventLoop as RootActiveEventLoop, ControlFlow, DeviceEvents,
    EventLoopProxy as CoreEventLoopProxy, EventLoopProxyProvider,
    OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use winit_core::keyboard::KeyLocation;
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};
use winit_core::window::{
    self, CursorGrabMode, ImeCapabilities, ImePurpose, ImeRequest, ImeRequestError,
    ResizeDirection, Theme, Window as CoreWindow, WindowAttributes, WindowButtons, WindowId,
    WindowLevel,
};

use crate::keycodes;

static HAS_FOCUS: AtomicBool = AtomicBool::new(true);
static HAS_EVENT: AtomicBool = AtomicBool::new(false);

const GLOBAL_WINDOW: WindowId = WindowId::from_raw(0);

#[allow(dead_code)]
#[derive(Debug)]
pub struct EventLoop {
    pub openharmony_app: OpenHarmonyApp,
    window_target: ActiveEventLoop,
    cause: StartCause,
    primary_pointer: Option<FingerId>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct PlatformSpecificEventLoopAttributes {
    pub openharmony_app: Option<OpenHarmonyApp>,
}

impl Default for PlatformSpecificEventLoopAttributes {
    fn default() -> Self {
        Self { openharmony_app: Default::default() }
    }
}

impl EventLoop {
    pub fn new(attributes: &PlatformSpecificEventLoopAttributes) -> Result<Self, EventLoopError> {
        static EVENT_LOOP_CREATED: AtomicBool = AtomicBool::new(false);
        if EVENT_LOOP_CREATED.swap(true, Ordering::Relaxed) {
            // For better cross-platformness.
            return Err(EventLoopError::RecreationAttempt);
        }

        let openharmony_app = attributes.openharmony_app.as_ref().expect(
            "An `OpenHarmonyApp` as passed to lib is required to create an `EventLoop` on \
             OpenHarmony or HarmonyNext",
        );

        let event_loop_proxy = Arc::new(EventLoopProxy::new(openharmony_app.create_waker()));

        Ok(Self {
            openharmony_app: openharmony_app.clone(),
            window_target: ActiveEventLoop {
                app: openharmony_app.clone(),
                control_flow: Cell::new(ControlFlow::default()),
                exit: Cell::new(false),
                event_loop_proxy,
            },
            cause: StartCause::Init,
            primary_pointer: None,
        })
    }

    pub fn window_target(&self) -> &dyn RootActiveEventLoop {
        &self.window_target
    }

    fn handle_input_event<A: ApplicationHandler>(
        &mut self,
        event: &InputEvent,
        app: &mut A,
        target: &ActiveEventLoop,
    ) {
        match event {
            InputEvent::TouchEvent(motion_event) => {
                let device_id = Some(DeviceId::from_raw(motion_event.device_id as i64));
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

                if let Some(_phase) = phase {
                    for pointer in motion_event.touch_points.iter() {
                        let position = PhysicalPosition { x: pointer.x as _, y: pointer.y as _ };
                        trace!(
                            "Input event {device_id:?}, {action:?}, loc={position:?}, \
                                 pointer={pointer:?}"
                        );

                        let finger_id = FingerId::from_raw(pointer.id as usize);
                        let force = Some(Force::Normalized(pointer.force as f64));

                        match action {
                            TouchEvent::Down => {
                                let primary = action == TouchEvent::Down;
                                if primary {
                                    self.primary_pointer = Some(finger_id);
                                }
                                let event = event::WindowEvent::PointerEntered {
                                    device_id,
                                    primary,
                                    position,
                                    // TODO: support mouse events
                                    kind: event::PointerKind::Touch(finger_id),
                                };
                                app.window_event(target, GLOBAL_WINDOW, event);
                                let event = event::WindowEvent::PointerButton {
                                    device_id,
                                    primary,
                                    state: event::ElementState::Pressed,
                                    position,
                                    // TODO: support mouse events
                                    button: event::ButtonSource::Touch { finger_id, force },
                                };
                                app.window_event(target, GLOBAL_WINDOW, event);
                            },
                            TouchEvent::Move => {
                                let primary = self.primary_pointer == Some(finger_id);
                                let event = event::WindowEvent::PointerMoved {
                                    device_id,
                                    primary,
                                    position,
                                    // TODO: support mouse events
                                    source: event::PointerSource::Touch { finger_id, force },
                                };
                                app.window_event(target, GLOBAL_WINDOW, event);
                            },
                            TouchEvent::Up | TouchEvent::Cancel => {
                                let primary = action == TouchEvent::Up
                                    || (action == TouchEvent::Cancel
                                        && self.primary_pointer == Some(finger_id));

                                if primary {
                                    self.primary_pointer = None;
                                }

                                if let TouchEvent::Up = action {
                                    let event = event::WindowEvent::PointerButton {
                                        device_id,
                                        primary,
                                        state: event::ElementState::Released,
                                        position,
                                        // TODO: support mouse events
                                        button: event::ButtonSource::Touch { finger_id, force },
                                    };
                                    app.window_event(target, GLOBAL_WINDOW, event);
                                }

                                let event = event::WindowEvent::PointerLeft {
                                    device_id,
                                    primary,
                                    position: Some(position),
                                    // TODO: support mouse events
                                    kind: event::PointerKind::Touch(finger_id),
                                };
                                app.window_event(target, GLOBAL_WINDOW, event);
                            },
                            _ => unreachable!(),
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

                        let event = event::WindowEvent::KeyboardInput {
                            device_id: Some(DeviceId::from_raw(key.device_id as i64)),
                            event: event::KeyEvent {
                                state,
                                physical_key: keycodes::to_physical_key(keycode),
                                logical_key: keycodes::to_logical(keycode),
                                location: keycodes::to_location(keycode),
                                // TODO
                                repeat: false,
                                text: None,
                                text_with_all_modifiers: None,
                                key_without_modifiers: keycodes::to_logical(keycode),
                            },
                            is_synthetic: false,
                        };
                        app.window_event(target, GLOBAL_WINDOW, event);
                    },
                }
            },
            InputEvent::ImeEvent(data) => match data {
                ImeEvent::TextInputEvent(s) => {
                    let pre_edit = Ime::Preedit(s.text.clone(), Some((s.text.len(), s.text.len())));

                    app.window_event(target, GLOBAL_WINDOW, event::WindowEvent::Ime(pre_edit));
                    app.window_event(
                        target,
                        GLOBAL_WINDOW,
                        event::WindowEvent::Ime(Ime::Commit(s.text.clone())),
                    );
                },
                ImeEvent::BackspaceEvent(_) => {
                    // Mock keyboard input event
                    [ElementState::Pressed, ElementState::Released].map(|state| {
                        let event = event::WindowEvent::KeyboardInput {
                            device_id: Some(DeviceId::from_raw(0 as i64)),
                            event: event::KeyEvent {
                                state,
                                logical_key: keycodes::to_logical(Keycode::Back),
                                physical_key: keycodes::to_physical_key(Keycode::Back),
                                repeat: false,
                                location: KeyLocation::Standard,
                                text: None,
                                text_with_all_modifiers: None,
                                key_without_modifiers: keycodes::to_logical(Keycode::Back),
                            },
                            is_synthetic: false,
                        };
                        app.window_event(target, GLOBAL_WINDOW, event);
                    });
                },

                ImeEvent::ImeStatusEvent(s) => match s {
                    KeyboardStatus::Hide => {
                        // Mock keyboard input event that make sure egui can receive the event and trigger onblur event
                        [ElementState::Pressed, ElementState::Released].map(|state| {
                            let event = event::WindowEvent::KeyboardInput {
                                device_id: Some(DeviceId::from_raw(0 as i64)),
                                event: event::KeyEvent {
                                    state,
                                    logical_key: keycodes::to_logical(Keycode::Enter),
                                    physical_key: keycodes::to_physical_key(Keycode::Enter),
                                    text_with_all_modifiers: None,
                                    key_without_modifiers: keycodes::to_logical(Keycode::Enter),
                                    repeat: false,
                                    location: KeyLocation::Standard,
                                    text: None,
                                },
                                is_synthetic: false,
                            };
                            app.window_event(target, GLOBAL_WINDOW, event);
                        });

                        let event = event::WindowEvent::Ime(Ime::Disabled);
                        app.window_event(target, GLOBAL_WINDOW, event);
                    },
                    _ => {
                        warn!("Unknown openharmony_ability ime status event {s:?}")
                    },
                },
            },
        }
    }

    pub fn run_app<A: ApplicationHandler>(self, app: A) -> Result<(), EventLoopError> {
        let event_looper = Box::leak(Box::new(self));
        event_looper.run_app_on_demand(app)
    }

    pub fn run_app_on_demand<A: ApplicationHandler>(
        &mut self,
        app: A,
    ) -> Result<(), EventLoopError> {
        match self.pump_app_events(None, app) {
            PumpStatus::Continue => Ok(()),
            PumpStatus::Exit(code) => Err(EventLoopError::ExitFailure(code)),
        }
    }

    pub fn pump_app_events<A: ApplicationHandler>(
        &mut self,
        _timeout: Option<Duration>,
        app: A,
    ) -> PumpStatus {
        if HAS_EVENT.load(Ordering::SeqCst) {
            trace!("EventLoop is already running");
        }
        trace!("Mainloop iteration");
        let event_app = Box::leak(Box::new(app));
        let target = Box::leak(Box::new(ActiveEventLoop {
            app: self.openharmony_app.clone(),
            control_flow: self.window_target.control_flow.clone(),
            exit: self.window_target.exit.clone(),
            event_loop_proxy: self.window_target.event_loop_proxy.clone(),
        }));

        self.openharmony_app.clone().run_loop(|event| {
            match event {
                MainEvent::SurfaceCreate { .. } => {
                    event_app.new_events(target, StartCause::Init);
                    event_app.can_create_surfaces(target);
                },
                MainEvent::SurfaceDestroy { .. } => {
                    event_app.destroy_surfaces(target);
                },
                MainEvent::WindowResize { .. } => {
                    let win = self.openharmony_app.native_window();
                    let size = if let Some(win) = win {
                        PhysicalSize::new(win.width() as _, win.height() as _)
                    } else {
                        PhysicalSize::new(0, 0)
                    };

                    event_app.window_event(
                        target,
                        GLOBAL_WINDOW,
                        event::WindowEvent::SurfaceResized(size),
                    );
                },
                MainEvent::WindowRedraw { .. } => {
                    event_app.window_event(
                        target,
                        GLOBAL_WINDOW,
                        event::WindowEvent::RedrawRequested,
                    );
                },
                MainEvent::ContentRectChange { .. } => {
                    warn!("TODO: find a way to notify application of content rect change");
                },
                MainEvent::GainedFocus => {
                    HAS_FOCUS.store(true, Ordering::Relaxed);

                    event_app.window_event(
                        target,
                        GLOBAL_WINDOW,
                        event::WindowEvent::Focused(true),
                    );
                },
                MainEvent::LostFocus => {
                    HAS_FOCUS.store(false, Ordering::Relaxed);

                    event_app.window_event(
                        target,
                        GLOBAL_WINDOW,
                        event::WindowEvent::Focused(false),
                    );
                },
                MainEvent::ConfigChanged { .. } => {
                    let win = self.openharmony_app.native_window();
                    if let Some(_win) = win {
                        let old_scale_factor = scale_factor(&self.openharmony_app);
                        let scale_factor = scale_factor(&self.openharmony_app);
                        if (scale_factor - old_scale_factor).abs() < f64::EPSILON {
                            let new_surface_size =
                                Arc::new(Mutex::new(screen_size(&self.openharmony_app)));
                            let event = event::WindowEvent::ScaleFactorChanged {
                                surface_size_writer: SurfaceSizeWriter::new(Arc::downgrade(
                                    &new_surface_size,
                                )),
                                scale_factor,
                            };

                            event_app.window_event(target, GLOBAL_WINDOW, event);
                        }
                    }
                },
                MainEvent::LowMemory => {
                    event_app.memory_warning(target);
                },
                MainEvent::Start => {
                    // XXX: how to forward this state to applications?
                    warn!("TODO: forward onStart notification to application");
                },
                MainEvent::Resume { .. } => {
                    event_app.resumed(target);
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
                    event_app.window_event(
                        target,
                        GLOBAL_WINDOW,
                        event::WindowEvent::CloseRequested,
                    );
                },
                MainEvent::Destroy => {
                    // XXX: maybe exit mainloop to drop things before being
                    // killed by the OS?
                    warn!("TODO: forward onDestroy notification to application");
                },
                MainEvent::Input(input_event) => {
                    self.handle_input_event(&input_event, event_app, target);
                },
                MainEvent::UserEvent { .. } => {
                    event_app.proxy_wake_up(target);
                },
                unknown => {
                    trace!("Unknown MainEvent {unknown:?} (ignored)");
                },
            };

            event_app.about_to_wait(target);

            if self.window_target.exit.get() {
                self.openharmony_app.exit(0);
            }
        });

        PumpStatus::Continue
    }

    pub fn create_proxy(&self) -> EventLoopProxy {
        EventLoopProxy {
            wake_up: AtomicBool::new(false),
            waker: self.openharmony_app.create_waker(),
        }
    }
}

pub struct EventLoopProxy {
    wake_up: AtomicBool,
    waker: OpenHarmonyWaker,
}

impl fmt::Debug for EventLoopProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLoopProxy").field("wake_up", &self.wake_up).finish_non_exhaustive()
    }
}

impl EventLoopProxy {
    pub fn new(waker: OpenHarmonyWaker) -> Self {
        Self { wake_up: AtomicBool::new(false), waker }
    }
}

impl EventLoopProxyProvider for EventLoopProxy {
    fn wake_up(&self) {
        self.wake_up.store(true, Ordering::Relaxed);
        self.waker.wake();
    }
}

#[derive(Debug)]
pub struct ActiveEventLoop {
    pub(crate) app: OpenHarmonyApp,
    control_flow: Cell<ControlFlow>,
    exit: Cell<bool>,
    event_loop_proxy: Arc<EventLoopProxy>,
}

#[allow(dead_code)]
impl ActiveEventLoop {
    fn clear_exit(&self) {
        self.exit.set(false);
    }
}

impl RootActiveEventLoop for ActiveEventLoop {
    fn create_proxy(&self) -> CoreEventLoopProxy {
        CoreEventLoopProxy::new(self.event_loop_proxy.clone())
    }

    fn create_window(
        &self,
        window_attributes: WindowAttributes,
    ) -> Result<Box<dyn CoreWindow>, RequestError> {
        Ok(Box::new(Window::new(self, window_attributes)?))
    }

    fn create_custom_cursor(
        &self,
        _source: CustomCursorSource,
    ) -> Result<CustomCursor, RequestError> {
        Err(NotSupportedError::new("create_custom_cursor is not supported").into())
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        None
    }

    fn system_theme(&self) -> Option<Theme> {
        None
    }

    fn listen_device_events(&self, _allowed: DeviceEvents) {}

    fn set_control_flow(&self, control_flow: ControlFlow) {
        self.control_flow.set(control_flow)
    }

    fn control_flow(&self) -> ControlFlow {
        self.control_flow.get()
    }

    fn exit(&self) {
        self.exit.set(true)
    }

    fn exiting(&self) -> bool {
        self.exit.get()
    }

    fn owned_display_handle(&self) -> CoreOwnedDisplayHandle {
        CoreOwnedDisplayHandle::new(Arc::new(OwnedDisplayHandle))
    }

    fn rwh_06_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }
}

impl rwh_06::HasDisplayHandle for ActiveEventLoop {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::OhosDisplayHandle::new();
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw.into()) })
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct OwnedDisplayHandle;

impl rwh_06::HasDisplayHandle for OwnedDisplayHandle {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = rwh_06::OhosDisplayHandle::new();
        Ok(unsafe { rwh_06::DisplayHandle::borrow_raw(raw.into()) })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlatformSpecificWindowAttributes;

#[derive(Debug)]
pub struct Window {
    app: OpenHarmonyApp,
    ime_capabilities: Mutex<Option<ImeCapabilities>>,
}

impl Window {
    pub(crate) fn new(
        el: &ActiveEventLoop,
        _window_attrs: window::WindowAttributes,
    ) -> Result<Self, RequestError> {
        // FIXME this ignores requested window attributes

        Ok(Self { app: el.app.clone(), ime_capabilities: Default::default() })
    }

    pub fn request_redraw(&self) {}

    pub fn scale_factor(&self) -> f64 {
        self.app.scale() as f64
    }

    pub fn config(&self) -> Configuration {
        self.app.config()
    }

    pub fn content_rect(&self) -> Rect {
        self.app.content_rect()
    }

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

    fn raw_display_handle_rwh_06(&self) -> Result<rwh_06::RawDisplayHandle, rwh_06::HandleError> {
        Ok(rwh_06::RawDisplayHandle::Ohos(rwh_06::OhosDisplayHandle::new()))
    }
}

impl rwh_06::HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<rwh_06::DisplayHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_display_handle_rwh_06()?;
        unsafe { Ok(rwh_06::DisplayHandle::borrow_raw(raw)) }
    }
}

impl rwh_06::HasWindowHandle for Window {
    fn window_handle(&self) -> Result<rwh_06::WindowHandle<'_>, rwh_06::HandleError> {
        let raw = self.raw_window_handle_rwh_06()?;
        unsafe { Ok(rwh_06::WindowHandle::borrow_raw(raw)) }
    }
}

impl CoreWindow for Window {
    fn id(&self) -> WindowId {
        GLOBAL_WINDOW
    }

    fn primary_monitor(&self) -> Option<CoreMonitorHandle> {
        None
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = CoreMonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn current_monitor(&self) -> Option<CoreMonitorHandle> {
        None
    }

    fn scale_factor(&self) -> f64 {
        scale_factor(&self.app)
    }

    fn request_redraw(&self) {}

    fn pre_present_notify(&self) {}

    fn surface_position(&self) -> PhysicalPosition<i32> {
        (0, 0).into()
    }

    fn outer_position(&self) -> Result<PhysicalPosition<i32>, RequestError> {
        Err(NotSupportedError::new("outer_position is not supported").into())
    }

    fn set_outer_position(&self, _position: Position) {
        // no effect
    }

    fn surface_size(&self) -> PhysicalSize<u32> {
        self.outer_size()
    }

    fn request_surface_size(&self, _size: Size) -> Option<PhysicalSize<u32>> {
        Some(self.surface_size())
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        screen_size(&self.app)
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    fn set_min_surface_size(&self, _: Option<Size>) {}

    fn set_max_surface_size(&self, _: Option<Size>) {}

    fn surface_resize_increments(&self) -> Option<PhysicalSize<u32>> {
        None
    }

    fn set_surface_resize_increments(&self, _increments: Option<Size>) {}

    fn set_title(&self, _title: &str) {}

    fn set_transparent(&self, _transparent: bool) {}

    fn set_blur(&self, _blur: bool) {}

    fn set_visible(&self, _visibility: bool) {}

    fn is_visible(&self) -> Option<bool> {
        None
    }

    fn set_resizable(&self, _resizeable: bool) {}

    fn is_resizable(&self) -> bool {
        false
    }

    fn set_enabled_buttons(&self, _buttons: WindowButtons) {}

    fn enabled_buttons(&self) -> WindowButtons {
        WindowButtons::all()
    }

    fn set_minimized(&self, _minimized: bool) {}

    fn is_minimized(&self) -> Option<bool> {
        None
    }

    fn set_maximized(&self, _maximized: bool) {}

    fn is_maximized(&self) -> bool {
        false
    }

    fn set_fullscreen(&self, _monitor: Option<Fullscreen>) {
        warn!("Cannot set fullscreen on Android");
    }

    fn fullscreen(&self) -> Option<Fullscreen> {
        None
    }

    fn set_decorations(&self, _decorations: bool) {}

    fn is_decorated(&self) -> bool {
        true
    }

    fn set_window_level(&self, _level: WindowLevel) {}

    fn set_window_icon(&self, _window_icon: Option<winit_core::icon::Icon>) {}

    fn set_ime_cursor_area(&self, _position: Position, _size: Size) {}

    fn request_ime_update(&self, request: ImeRequest) -> Result<(), ImeRequestError> {
        let mut current_caps = self.ime_capabilities.lock().unwrap();
        match request {
            ImeRequest::Enable(enable) => {
                let (capabilities, _) = enable.into_raw();
                if current_caps.is_some() {
                    return Err(ImeRequestError::AlreadyEnabled);
                }
                *current_caps = Some(capabilities);
                self.app.show_keyboard();
            },
            ImeRequest::Update(_) => {
                if current_caps.is_none() {
                    return Err(ImeRequestError::NotEnabled);
                }
            },
            ImeRequest::Disable => {
                *current_caps = None;
                self.app.hide_keyboard();
            },
        }

        Ok(())
    }

    fn ime_capabilities(&self) -> Option<ImeCapabilities> {
        *self.ime_capabilities.lock().unwrap()
    }

    fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    fn focus_window(&self) {}

    fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    fn set_cursor(&self, _: Cursor) {}

    fn set_cursor_position(&self, _: Position) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_position is not supported").into())
    }

    fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_grab is not supported").into())
    }

    fn set_cursor_visible(&self, _: bool) {}

    fn drag_window(&self) -> Result<(), RequestError> {
        Err(NotSupportedError::new("drag_window is not supported").into())
    }

    fn drag_resize_window(&self, _direction: ResizeDirection) -> Result<(), RequestError> {
        Err(NotSupportedError::new("drag_resize_window").into())
    }

    #[inline]
    fn show_window_menu(&self, _position: Position) {}

    fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), RequestError> {
        Err(NotSupportedError::new("set_cursor_hittest is not supported").into())
    }

    fn set_theme(&self, _theme: Option<Theme>) {}

    fn theme(&self) -> Option<Theme> {
        None
    }

    fn set_content_protected(&self, _protected: bool) {}

    fn has_focus(&self) -> bool {
        HAS_FOCUS.load(Ordering::Relaxed)
    }

    fn title(&self) -> String {
        String::new()
    }

    fn reset_dead_keys(&self) {}

    fn rwh_06_display_handle(&self) -> &dyn rwh_06::HasDisplayHandle {
        self
    }

    fn rwh_06_window_handle(&self) -> &dyn rwh_06::HasWindowHandle {
        self
    }
}

fn screen_size(app: &OpenHarmonyApp) -> PhysicalSize<u32> {
    if let Some(native_window) = app.native_window() {
        PhysicalSize::new(native_window.width() as _, native_window.height() as _)
    } else {
        PhysicalSize::new(0, 0)
    }
}

fn scale_factor(app: &OpenHarmonyApp) -> f64 {
    app.scale() as f64
}
