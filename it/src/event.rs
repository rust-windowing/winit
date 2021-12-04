use std::path::PathBuf;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{
    AxisId, ButtonId, DeviceEvent as WDeviceEvent, DeviceId, ElementState, KeyEvent as WKeyEvent,
    MouseButton, MouseScrollDelta, RawKeyEvent, Touch, TouchPhase,
};
use winit::event::{Event as WEvent, WindowEvent as WWindowEvent};
use winit::keyboard;
use winit::keyboard::ModifiersState;
#[cfg(have_mod_supplement)]
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
use winit::window::{Theme, WindowId};

#[derive(Clone, Debug, PartialEq)]
pub struct UserEvent(pub usize);

#[derive(Clone, Debug, PartialEq)]
pub struct ModSupplement {
    pub text_with_all_modifiers: Option<String>,
    pub key_without_modifiers: keyboard::Key<'static>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KeyEvent {
    pub physical_key: keyboard::KeyCode,
    pub logical_key: keyboard::Key<'static>,
    pub text: Option<&'static str>,
    pub location: keyboard::KeyLocation,
    pub state: ElementState,
    pub repeat: bool,
    #[cfg(have_mod_supplement)]
    pub mod_supplement: ModSupplement,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowKeyboardInput {
    pub device_id: DeviceId,
    pub event: KeyEvent,
    pub is_synthetic: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowCursorMoved {
    pub device_id: DeviceId,
    pub position: PhysicalPosition<f64>,
    pub modifiers: ModifiersState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowCursorEntered {
    pub device_id: DeviceId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowCursorLeft {
    pub device_id: DeviceId,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowMouseWheel {
    pub device_id: DeviceId,
    pub delta: MouseScrollDelta,
    pub phase: TouchPhase,
    pub modifiers: ModifiersState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowMouseInput {
    pub device_id: DeviceId,
    pub state: ElementState,
    pub button: MouseButton,
    pub modifiers: ModifiersState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowTouchpadPressure {
    pub device_id: DeviceId,
    pub pressure: f32,
    pub stage: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowAxisMotion {
    pub device_id: DeviceId,
    pub axis: AxisId,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowScaleFactorChanged {
    pub scale_factor: f64,
    pub new_inner_size: PhysicalSize<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WindowEvent {
    Resized(PhysicalSize<u32>),
    Moved(PhysicalPosition<i32>),
    CloseRequested,
    Destroyed,
    DroppedFile(PathBuf),
    HoveredFile(PathBuf),
    HoveredFileCancelled,
    Focused(bool),
    KeyboardInput(WindowKeyboardInput),
    ModifiersChanged(ModifiersState),
    CursorMoved(WindowCursorMoved),
    CursorEntered(WindowCursorEntered),
    CursorLeft(WindowCursorLeft),
    MouseWheel(WindowMouseWheel),
    MouseInput(WindowMouseInput),
    TouchpadPressure(WindowTouchpadPressure),
    AxisMotion(WindowAxisMotion),
    Touch(Touch),
    ScaleFactorChanged(WindowScaleFactorChanged),
    ThemeChanged(Theme),
    ReceivedImeText(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct WindowEventExt {
    pub window_id: WindowId,
    pub event: WindowEvent,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceEventExt {
    pub device_id: DeviceId,
    pub event: DeviceEvent,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    WindowEvent(WindowEventExt),
    DeviceEvent(DeviceEventExt),
    UserEvent(UserEvent),
    RedrawRequested(WindowId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceMouseMotion {
    pub delta: (f64, f64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceMouseWheel {
    pub delta: MouseScrollDelta,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceMotion {
    pub axis: AxisId,
    pub value: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceButton {
    pub button: ButtonId,
    pub state: ElementState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceText {
    pub codepoint: char,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DeviceEvent {
    Added,
    Removed,
    MouseMotion(DeviceMouseMotion),
    MouseWheel(DeviceMouseWheel),
    Motion(DeviceMotion),
    Button(DeviceButton),
    Key(RawKeyEvent),
    Text(DeviceText),
}

#[allow(deprecated)]
pub fn map_device_event(e: WDeviceEvent) -> DeviceEvent {
    match e {
        WDeviceEvent::Added => DeviceEvent::Added,
        WDeviceEvent::Removed => DeviceEvent::Removed,
        WDeviceEvent::MouseMotion { delta } => {
            DeviceEvent::MouseMotion(DeviceMouseMotion { delta })
        }
        WDeviceEvent::MouseWheel { delta } => DeviceEvent::MouseWheel(DeviceMouseWheel { delta }),
        WDeviceEvent::Motion { axis, value } => DeviceEvent::Motion(DeviceMotion { axis, value }),
        WDeviceEvent::Button { button, state } => {
            DeviceEvent::Button(DeviceButton { button, state })
        }
        WDeviceEvent::Key(v) => DeviceEvent::Key(v),
        WDeviceEvent::Text { codepoint } => DeviceEvent::Text(DeviceText { codepoint }),
    }
}

pub fn map_key_event(e: WKeyEvent) -> KeyEvent {
    KeyEvent {
        physical_key: e.physical_key,
        logical_key: e.logical_key,
        text: e.text,
        location: e.location,
        state: e.state,
        repeat: e.repeat,
        #[cfg(have_mod_supplement)]
        mod_supplement: ModSupplement {
            text_with_all_modifiers: e.text_with_all_modifiers().map(|s| s.to_string()),
            key_without_modifiers: e.key_without_modifiers(),
        },
    }
}

#[allow(deprecated)]
pub fn map_window_event(e: WWindowEvent<'_>) -> WindowEvent {
    match e {
        WWindowEvent::Resized(v) => WindowEvent::Resized(v),
        WWindowEvent::Moved(v) => WindowEvent::Moved(v),
        WWindowEvent::CloseRequested => WindowEvent::CloseRequested,
        WWindowEvent::Destroyed => WindowEvent::Destroyed,
        WWindowEvent::DroppedFile(v) => WindowEvent::DroppedFile(v),
        WWindowEvent::HoveredFile(v) => WindowEvent::HoveredFile(v),
        WWindowEvent::HoveredFileCancelled => WindowEvent::HoveredFileCancelled,
        WWindowEvent::Focused(v) => WindowEvent::Focused(v),
        WWindowEvent::KeyboardInput {
            device_id,
            event,
            is_synthetic,
        } => WindowEvent::KeyboardInput(WindowKeyboardInput {
            device_id,
            event: map_key_event(event),
            is_synthetic,
        }),
        WWindowEvent::ModifiersChanged(v) => WindowEvent::ModifiersChanged(v),
        WWindowEvent::CursorMoved {
            device_id,
            position,
            modifiers,
        } => WindowEvent::CursorMoved(WindowCursorMoved {
            device_id,
            position,
            modifiers,
        }),
        WWindowEvent::CursorEntered { device_id } => {
            WindowEvent::CursorEntered(WindowCursorEntered { device_id })
        }
        WWindowEvent::CursorLeft { device_id } => {
            WindowEvent::CursorLeft(WindowCursorLeft { device_id })
        }
        WWindowEvent::MouseWheel {
            device_id,
            delta,
            phase,
            modifiers,
        } => WindowEvent::MouseWheel(WindowMouseWheel {
            device_id,
            delta,
            phase,
            modifiers,
        }),
        WWindowEvent::MouseInput {
            device_id,
            state,
            button,
            modifiers,
        } => WindowEvent::MouseInput(WindowMouseInput {
            device_id,
            state,
            button,
            modifiers,
        }),
        WWindowEvent::TouchpadPressure {
            device_id,
            pressure,
            stage,
        } => WindowEvent::TouchpadPressure(WindowTouchpadPressure {
            device_id,
            pressure,
            stage,
        }),
        WWindowEvent::AxisMotion {
            device_id,
            axis,
            value,
        } => WindowEvent::AxisMotion(WindowAxisMotion {
            device_id,
            axis,
            value,
        }),
        WWindowEvent::Touch(v) => WindowEvent::Touch(v),
        WWindowEvent::ScaleFactorChanged {
            scale_factor,
            new_inner_size,
        } => WindowEvent::ScaleFactorChanged(WindowScaleFactorChanged {
            scale_factor,
            new_inner_size: *new_inner_size,
        }),
        WWindowEvent::ThemeChanged(v) => WindowEvent::ThemeChanged(v),
        WWindowEvent::ReceivedImeText(v) => WindowEvent::ReceivedImeText(v),
    }
}

pub fn map_event(e: WEvent<'_, UserEvent>) -> Option<Event> {
    match e {
        WEvent::NewEvents(_) => None,
        WEvent::WindowEvent { window_id, event } => Some(Event::WindowEvent(WindowEventExt {
            window_id,
            event: map_window_event(event),
        })),
        WEvent::DeviceEvent { device_id, event } => Some(Event::DeviceEvent(DeviceEventExt {
            device_id,
            event: map_device_event(event),
        })),
        WEvent::UserEvent(v) => Some(Event::UserEvent(v)),
        WEvent::Suspended => None,
        WEvent::Resumed => None,
        WEvent::MainEventsCleared => None,
        WEvent::RedrawRequested(v) => Some(Event::RedrawRequested(v)),
        WEvent::RedrawEventsCleared => None,
        WEvent::LoopDestroyed => None,
    }
}

#[derive(Clone, Debug, Default)]
pub struct KeyEventMatcher {
    pub physical_key: Option<keyboard::KeyCode>,
    pub logical_key: Option<keyboard::Key<'static>>,
    pub text: Option<Option<&'static str>>,
    pub location: Option<keyboard::KeyLocation>,
    pub state: Option<ElementState>,
    pub repeat: Option<bool>,
}

impl KeyEventMatcher {
    #[allow(dead_code)]
    pub fn matches(&self, kev: &KeyEvent) -> bool {
        if let Some(pk) = &self.physical_key {
            if pk != &kev.physical_key {
                return false;
            }
        }
        if let Some(lk) = &self.logical_key {
            if lk != &kev.logical_key {
                return false;
            }
        }
        if let Some(lk) = &self.text {
            if lk != &kev.text {
                return false;
            }
        }
        if let Some(lk) = &self.location {
            if lk != &kev.location {
                return false;
            }
        }
        if let Some(lk) = &self.state {
            if lk != &kev.state {
                return false;
            }
        }
        if let Some(lk) = &self.repeat {
            if lk != &kev.repeat {
                return false;
            }
        }
        true
    }
}
