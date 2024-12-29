use std::cell::Cell;
use std::collections::VecDeque;
use std::hash::Hash;
use std::marker::PhantomData;
use std::num::{NonZeroU16, NonZeroU32};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use openharmony_ability::xcomponent::{Action, KeyCode, TouchEvent};
use tracing::{debug, trace, warn};

use openharmony_ability::{
    Configuration, Event as MainEvent, InputEvent, OpenHarmonyApp, OpenHarmonyWaker, Rect,
};

use crate::application::ApplicationHandler;
use crate::cursor::Cursor;
use crate::dpi::{PhysicalInsets, PhysicalPosition, PhysicalSize, Position, Size};
use crate::error::{self, EventLoopError, NotSupportedError};
use crate::event::{self, Force, InnerSizeWriter, StartCause};
use crate::event_loop::{
    self, ActiveEventLoop as RootAEL, ControlFlow, DeviceEvents,
    EventLoopProxy as CoreEventLoopProxy, OwnedDisplayHandle as CoreOwnedDisplayHandle,
};
use crate::monitor::MonitorHandle as RootMonitorHandle;
use crate::window::{
    self, CursorGrabMode, CustomCursor, CustomCursorSource, Fullscreen, ImePurpose,
    ResizeDirection, Theme, Window as CoreWindow, WindowAttributes, WindowButtons, WindowLevel,
};

mod keycodes;

pub(crate) use crate::cursor::{
    NoCustomCursor as PlatformCustomCursor, NoCustomCursor as PlatformCustomCursorSource,
};
pub(crate) use crate::icon::NoIcon as PlatformIcon;

static HAS_FOCUS: AtomicBool = AtomicBool::new(true);

struct PeekableReceiver<T> {
    recv: mpsc::Receiver<T>,
    first: Option<T>,
}

impl<T> PeekableReceiver<T> {
    pub fn from_recv(recv: mpsc::Receiver<T>) -> Self {
        Self { recv, first: None }
    }

    pub fn has_incoming(&mut self) -> bool {
        if self.first.is_some() {
            return true;
        }
        match self.recv.try_recv() {
            Ok(v) => {
                self.first = Some(v);
                true
            },
            Err(mpsc::TryRecvError::Empty) => false,
            Err(mpsc::TryRecvError::Disconnected) => {
                warn!("Channel was disconnected when checking incoming");
                false
            },
        }
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
    running: bool,
    cause: StartCause,
    combining_accent: Option<char>,
    user_events_sender: mpsc::Sender<T>,
    user_events_receiver: PeekableReceiver<T>,
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
            running: false,
            cause: StartCause::Init,
            combining_accent: None,
            user_events_sender,
            user_events_receiver: PeekableReceiver::from_recv(user_events_receiver),
        })
    }

    pub(crate) fn window_target(&self) -> &event_loop::ActiveEventLoop {
        &self.window_target
    }

    fn handle_input_event<F>(
        &mut self,
        openharmony_app: &OpenHarmonyApp,
        event: &InputEvent,
        callback: &mut F,
    ) where
        F: FnMut(event::Event<T>, &RootAEL),
    {
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
                        // TODO
                        let tool_type = "unknown";
                        let position = PhysicalPosition { x: pointer.x as _, y: pointer.y as _ };
                        trace!(
                            "Input event {device_id:?}, {action:?}, loc={position:?}, \
                                 pointer={pointer:?}, tool_type={tool_type:?}"
                        );
                        let force = Some(Force::Normalized(pointer.force as f64));
    
                        match action {
                            TouchEvent::Down => {
                                let event = event::Event::WindowEvent {
                                    window_id,
                                    event: event::WindowEvent::Touch(
                                        event::Touch {
                                            device_id,
                                            phase,
                                            location,
                                            id: pointer.id as u64,
                                            force: Some(Force::Normalized(pointer.force as f64)),
                                        })
                                    };
                                app.window_event(&self.window_target, GLOBAL_WINDOW, event);
                                let event = event::WindowEvent::PointerButton {
                                    device_id,
                                    state: event::ElementState::Pressed,
                                    position,
                                    primary,
                                    button: match tool_type {
                                        // TODO
                                        // android_activity::input::ToolType::Finger => {
                                        //     event::ButtonSource::Touch { finger_id, force }
                                        // },
                                        // // TODO mouse events
                                        // android_activity::input::ToolType::Mouse => continue,
                                        _ => event::ButtonSource::Unknown(0),
                                    },
                                };
                                app.window_event(&self.window_target, GLOBAL_WINDOW, event);
                            },
                            TouchEvent::Move => {
                                let primary = self.primary_pointer == Some(finger_id);
                                let event = event::WindowEvent::PointerMoved {
                                    device_id,
                                    position,
                                    primary,
                                    source: match tool_type {
                                        // TODO
                                        // android_activity::input::ToolType::Finger => {
                                        //     event::PointerSource::Touch { finger_id, force }
                                        // },
                                        // // TODO mouse events
                                        // android_activity::input::ToolType::Mouse => continue,
                                        _ => event::PointerSource::Unknown,
                                    },
                                };
                                app.window_event(&self.window_target, GLOBAL_WINDOW, event);
                            },
                            TouchEvent::Up | TouchEvent::Cancel => {
                                let primary = self.primary_pointer == Some(finger_id);
                                if let TouchEvent::Up = action {
                                    let event = event::WindowEvent::PointerButton {
                                        device_id,
                                        state: event::ElementState::Released,
                                        position,
                                        primary,
                                        button: match tool_type {
                                            //
                                            // android_activity::input::ToolType::Finger => {
                                            //     event::ButtonSource::Touch { finger_id, force }
                                            // },
                                            // // TODO mouse events
                                            // android_activity::input::ToolType::Mouse => continue,
                                            _ => event::ButtonSource::Unknown(0),
                                        },
                                    };
                                    app.window_event(&self.window_target, GLOBAL_WINDOW, event);
                                }
    
                                let event = event::WindowEvent::PointerLeft {
                                    device_id,
                                    primary,
                                    position: Some(position),
                                    kind: match tool_type {
                                        // TODO
                                        // android_activity::input::ToolType::Finger => {
                                        //     event::PointerKind::Touch(finger_id)
                                        // },
                                        // // TODO mouse events
                                        // android_activity::input::ToolType::Mouse => continue,
                                        _ => event::PointerKind::Unknown,
                                    },
                                };
                                app.window_event(&self.window_target, GLOBAL_WINDOW, event);
                            },
                            _ => unreachable!(),
                        }
                    }
                }
            },
            InputEvent::KeyEvent(key) => {
                match key.code {
                    // Flag keys related to volume as unhandled. While winit does not have a way for
                    // applications to configure what keys to flag as handled,
                    // this appears to be a good default until winit
                    // can be configured.
                    // TODO
                    // KeyCode::VolumeUp | KeyCode::VolumeDown | KeyCode::VolumeMute
                    //     if self.ignore_volume_keys =>
                    // {
                    //     input_status = InputStatus::Unhandled
                    // },
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
                                platform_specific: KeyEventExtra {},
                            },
                            is_synthetic: false,
                        };

                        app.window_event(&self.window_target, GLOBAL_WINDOW, event);
                    },
                }
            },
            _ => {
                warn!("Unknown openharmony_ability input event {event:?}")
            },
        }
    }

    pub fn run<F>(mut self, event_handle: F) -> Result<(), EventLoopError>
    where
        F: FnMut(event::Event<T>, &RootAEL),
    {
        trace!("Mainloop iteration");

        let cause = self.cause;

        let mut callback = event_handle;

        callback(event::Event::NewEvents(cause), self.window_target());

        let openharmony_app = self.openharmony_app.clone();
        let input_app = self.openharmony_app.clone();

        openharmony_app.run_loop(|event| {
            match event {
                MainEvent::SurfaceCreate { .. } => {
                    callback(event::Event::Resumed, self.window_target());
                },
                MainEvent::SurfaceDestroy { .. } => {
                    callback(event::Event::Suspended, self.window_target());
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
                    callback(event, self.window_target());
                },
                MainEvent::WindowRedraw { .. } => {
                    let event = event::Event::WindowEvent {
                        window_id: window::WindowId(WindowId),
                        event: event::WindowEvent::RedrawRequested,
                    };
                    callback(event, self.window_target());
                },
                MainEvent::ContentRectChange { .. } => {
                    warn!("TODO: find a way to notify application of content rect change");
                },
                MainEvent::GainedFocus => {
                    HAS_FOCUS.store(true, Ordering::Relaxed);
                    callback(
                        event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Focused(true),
                        },
                        self.window_target(),
                    );
                },
                MainEvent::LostFocus => {
                    HAS_FOCUS.store(false, Ordering::Relaxed);
                    callback(
                        event::Event::WindowEvent {
                            window_id: window::WindowId(WindowId),
                            event: event::WindowEvent::Focused(true),
                        },
                        self.window_target(),
                    );
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
                        callback(event, self.window_target());
                    }
                },
                MainEvent::LowMemory => {
                    callback(event::Event::MemoryWarning, self.window_target());
                },
                MainEvent::Start => {
                    // app.resumed(self.window_target());
                },
                MainEvent::Resume { .. } => {
                    debug!("App Resumed - is running");
                    // TODO: This is incorrect - will be solved in https://github.com/rust-windowing/winit/pull/3897
                    // self.running = true;
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
                MainEvent::Stop => {
                    callback(event::Event::Suspended, self.window_target());
                },
                MainEvent::Destroy => {
                    // XXX: maybe exit mainloop to drop things before being
                    // killed by the OS?
                    warn!("TODO: forward onDestroy notification to application");
                },
                MainEvent::Input(e) => {
                    warn!("TODO: forward onDestroy notification to application");
                    // let openharmony_app = self.openharmony_app.clone();
                    self.handle_input_event(&input_app, &e, &mut callback)
                },
                unknown => {
                    trace!("Unknown MainEvent {unknown:?} (ignored)");
                },
            }
        });

        Ok(())
    }

    fn control_flow(&self) -> ControlFlow {
        self.window_target.control_flow()
    }

    fn exiting(&self) -> bool {
        self.window_target.exiting()
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

pub struct ActiveEventLoop {
    pub(crate) app: OpenHarmonyApp,
    control_flow: Cell<ControlFlow>,
    exit: Cell<bool>,
}

impl ActiveEventLoop {
    fn create_custom_cursor(&self, source: CustomCursorSource) -> CustomCursor {
        let _ = source.inner;
        CustomCursor { inner: PlatformCustomCursor }
    }

    fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        let mut v = VecDeque::with_capacity(1);
        v.push_back(MonitorHandle::new(self.app.clone()));
        v
    }

    fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        None
    }

    #[cfg(feature = "rwh_05")]
    #[inline]
    pub fn raw_display_handle_rwh_05(&self) -> rwh_05::RawDisplayHandle {
        unreachable!("rwh_05 is not supported on OpenHarmony");
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

    pub(crate) fn owned_display_handle(&self) -> OwnedDisplayHandle {
        OwnedDisplayHandle
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

impl Window {
    pub(crate) fn new(
        el: &ActiveEventLoop,
        _window_attrs: window::WindowAttributes,
    ) -> Result<Self, error::OsError> {
        // FIXME this ignores requested window attributes

        Ok(Self { app: el.app.clone() })
    }

    pub(crate) fn maybe_queue_on_main(&self, f: impl FnOnce(&Self) + Send + 'static) {
        f(self)
    }

    pub(crate) fn maybe_wait_on_main<R: Send>(&self, f: impl FnOnce(&Self) -> R + Send) -> R {
        f(self)
    }

    pub fn id(&self) -> WindowId {
        WindowId
    }

    fn scale_factor(&self) -> f64 {
        1.0
    }

    fn surface_position(&self) -> PhysicalPosition<i32> {
        PhysicalPosition::new(0, 0)
    }

    fn safe_area(&self) -> PhysicalInsets<u32> {
        PhysicalInsets::new(0, 0, 0, 0)
    }

    fn primary_monitor(&self) -> Option<RootMonitorHandle> {
        None
    }

    fn available_monitors(&self) -> Box<dyn Iterator<Item = RootMonitorHandle>> {
        Box::new(std::iter::empty())
    }

    fn current_monitor(&self) -> Option<RootMonitorHandle> {
        None
    }

    fn pre_present_notify(&self) {}

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, error::NotSupportedError> {
        Err(error::NotSupportedError::new())
    }

    fn set_outer_position(&self, _position: Position) {
        // no effect
    }

    fn surface_size(&self) -> PhysicalSize<u32> {
        // self.outer_size()
        PhysicalSize { width: 1080, height: 2720 }
    }

    fn request_surface_size(&self, _size: Size) -> Option<PhysicalSize<u32>> {
        // Some(self.surface_size())
        None
    }

    fn outer_size(&self) -> PhysicalSize<u32> {
        PhysicalSize { width: 1080, height: 2720 }
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

    fn set_window_icon(&self, _window_icon: Option<crate::icon::Icon>) {}

    fn set_ime_cursor_area(&self, _position: Position, _size: Size) {}

    fn set_ime_allowed(&self, _allowed: bool) {}

    fn set_ime_purpose(&self, _purpose: ImePurpose) {}

    fn focus_window(&self) {}

    fn request_user_attention(&self, _request_type: Option<window::UserAttentionType>) {}

    fn set_cursor(&self, _: Cursor) {}

    pub fn set_cursor_position(&self, _: Position) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    pub fn set_cursor_grab(&self, _: CursorGrabMode) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
    }

    fn set_cursor_visible(&self, _: bool) {}
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
    fn show_window_menu(&self, _position: Position) {}

    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), error::ExternalError> {
        Err(error::ExternalError::NotSupported(error::NotSupportedError::new()))
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
        Ok(rwh_06::RawDisplayHandle::Android(rwh_06::AndroidDisplayHandle::new()))
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
        Some("Android Device".to_owned())
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
