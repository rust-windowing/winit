use std::cell::OnceCell;
use std::f64;

use dpi::{LogicalPosition, PhysicalPosition, Position};
use smol_str::SmolStr;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{KeyboardEvent, MouseEvent, PointerEvent, WheelEvent};

use super::pointer::{PointerEventExt, WebPointerType};
use super::Engine;
use crate::event::{
    CursorButton, CursorType, Force, MouseButton, MouseScrollDelta, ToolAngle, ToolButton,
    ToolState, ToolTilt,
};
use crate::keyboard::{Key, KeyLocation, ModifiersState, NamedKey, PhysicalKey};

bitflags::bitflags! {
    // https://www.w3.org/TR/pointerevents3/#the-buttons-property
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ButtonsState: u16 {
        const LEFT    = 0b00001;
        const RIGHT   = 0b00010;
        const MIDDLE  = 0b00100;
        const BACK    = 0b01000;
        const FORWARD = 0b10000;
    }
}

impl From<ButtonsState> for MouseButton {
    fn from(value: ButtonsState) -> Self {
        match value {
            ButtonsState::LEFT => MouseButton::Left,
            ButtonsState::RIGHT => MouseButton::Right,
            ButtonsState::MIDDLE => MouseButton::Middle,
            ButtonsState::BACK => MouseButton::Back,
            ButtonsState::FORWARD => MouseButton::Forward,
            _ => MouseButton::Other(value.bits()),
        }
    }
}

impl From<CursorButton> for ButtonsState {
    fn from(cursor: CursorButton) -> Self {
        match cursor {
            CursorButton::Mouse(mouse) => mouse.into(),
            CursorButton::Pen(tool) | CursorButton::Eraser(tool) => tool.into(),
        }
    }
}

impl From<MouseButton> for ButtonsState {
    fn from(value: MouseButton) -> Self {
        match value {
            MouseButton::Left => ButtonsState::LEFT,
            MouseButton::Right => ButtonsState::RIGHT,
            MouseButton::Middle => ButtonsState::MIDDLE,
            MouseButton::Back => ButtonsState::BACK,
            MouseButton::Forward => ButtonsState::FORWARD,
            MouseButton::Other(value) => ButtonsState::from_bits_retain(value),
        }
    }
}

impl From<ToolButton> for ButtonsState {
    fn from(tool: ToolButton) -> Self {
        match tool {
            ToolButton::Contact => ButtonsState::LEFT,
            ToolButton::Barrel => ButtonsState::RIGHT,
            ToolButton::Other(value) => Self::from_bits_retain(value),
        }
    }
}

pub fn cursor_buttons(event: &MouseEvent) -> ButtonsState {
    #[allow(clippy::disallowed_methods)]
    ButtonsState::from_bits_retain(event.buttons())
}

pub fn raw_button(event: &MouseEvent) -> Option<u16> {
    // https://www.w3.org/TR/pointerevents3/#the-button-property
    #[allow(clippy::disallowed_methods)]
    let button = event.button();

    if button == -1 {
        None
    } else {
        Some(button.try_into().expect("unexpected negative mouse button value"))
    }
}

pub fn cursor_button(event: &PointerEventExt, r#type: WebPointerType) -> Option<CursorButton> {
    // https://www.w3.org/TR/pointerevents3/#the-button-property

    let button = raw_button(event)?;

    let button = match r#type {
        WebPointerType::Mouse => {
            let button = match button {
                0 => MouseButton::Left,
                1 => MouseButton::Middle,
                2 => MouseButton::Right,
                3 => MouseButton::Back,
                4 => MouseButton::Forward,
                button => MouseButton::Other(button),
            };

            CursorButton::Mouse(button)
        },
        WebPointerType::Pen => {
            if button == 5 {
                return Some(CursorButton::Eraser(ToolButton::Contact));
            };

            let button = match button {
                0 => ToolButton::Contact,
                2 => ToolButton::Barrel,
                button => ToolButton::Other(button),
            };

            CursorButton::Pen(button)
        },
        WebPointerType::Touch => unreachable!(),
    };

    Some(button)
}

pub fn cursor_type(
    event: &PointerEventExt,
    r#type: WebPointerType,
    button: Option<&CursorButton>,
) -> CursorType {
    match r#type {
        WebPointerType::Mouse => CursorType::Mouse,
        WebPointerType::Pen => {
            let force = Force::Normalized(event.pressure().into());
            let tangential_force = Some(event.tangential_pressure());
            let twist = Some(event.twist().try_into().expect("found invalid `twist`"));
            let tilt = Some(ToolTilt {
                x: event.tilt_x().try_into().expect("found invalid `tiltX`"),
                y: event.tilt_y().try_into().expect("found invalid `tiltY`"),
            });
            let angle = event
                .altitude_angle()
                .map(|altitude| ToolAngle { altitude, azimuth: event.azimuth_angle() });

            let state = ToolState { force, tangential_force, twist, tilt, angle };

            match button {
                Some(CursorButton::Eraser(_)) => CursorType::Eraser(state),
                Some(CursorButton::Pen(_)) | None => CursorType::Pen(state),
                _ => unreachable!(),
            }
        },
        WebPointerType::Touch => unreachable!(),
    }
}

impl MouseButton {
    pub fn to_id(self) -> u32 {
        match self {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Back => 3,
            MouseButton::Forward => 4,
            MouseButton::Other(value) => value.into(),
        }
    }
}

pub fn cursor_position(event: &MouseEvent) -> LogicalPosition<f64> {
    #[wasm_bindgen]
    extern "C" {
        type MouseEventExt;

        #[wasm_bindgen(method, getter, js_name = offsetX)]
        fn offset_x(this: &MouseEventExt) -> f64;

        #[wasm_bindgen(method, getter, js_name = offsetY)]
        fn offset_y(this: &MouseEventExt) -> f64;
    }

    let event: &MouseEventExt = event.unchecked_ref();

    LogicalPosition { x: event.offset_x(), y: event.offset_y() }
}

// TODO: Remove this when Firefox supports correct movement values in coalesced events and browsers
// have agreed on what coordinate space `movementX/Y` is using.
// See <https://bugzilla.mozilla.org/show_bug.cgi?id=1753724>.
// See <https://github.com/w3c/pointerlock/issues/42>.
pub enum MouseDelta {
    Chromium,
    Gecko { old_position: LogicalPosition<f64>, old_delta: LogicalPosition<f64> },
    Other,
}

impl MouseDelta {
    pub fn init(window: &web_sys::Window, event: &PointerEvent) -> Self {
        match super::engine(window) {
            Some(Engine::Chromium) => Self::Chromium,
            // Firefox has wrong movement values in coalesced events.
            Some(Engine::Gecko) if has_coalesced_events_support(event) => Self::Gecko {
                old_position: cursor_position(event),
                old_delta: LogicalPosition::new(
                    event.movement_x() as f64,
                    event.movement_y() as f64,
                ),
            },
            _ => Self::Other,
        }
    }

    pub fn delta(&mut self, event: &MouseEvent) -> Position {
        match self {
            MouseDelta::Chromium => {
                PhysicalPosition::new(event.movement_x(), event.movement_y()).into()
            },
            MouseDelta::Gecko { old_position, old_delta } => {
                let new_position = cursor_position(event);
                let x = new_position.x - old_position.x + old_delta.x;
                let y = new_position.y - old_position.y + old_delta.y;
                *old_position = new_position;
                *old_delta = LogicalPosition::new(0., 0.);
                LogicalPosition::new(x, y).into()
            },
            MouseDelta::Other => {
                LogicalPosition::new(event.movement_x(), event.movement_y()).into()
            },
        }
    }
}

pub fn mouse_scroll_delta(
    window: &web_sys::Window,
    event: &WheelEvent,
) -> Option<MouseScrollDelta> {
    let x = -event.delta_x();
    let y = -event.delta_y();

    match event.delta_mode() {
        WheelEvent::DOM_DELTA_LINE => Some(MouseScrollDelta::LineDelta(x as f32, y as f32)),
        WheelEvent::DOM_DELTA_PIXEL => {
            let delta = LogicalPosition::new(x, y).to_physical(super::scale_factor(window));
            Some(MouseScrollDelta::PixelDelta(delta))
        },
        _ => None,
    }
}

pub fn key_code(event: &KeyboardEvent) -> PhysicalKey {
    let code = event.code();
    PhysicalKey::from_key_code_attribute_value(&code)
}

pub fn key(event: &KeyboardEvent) -> Key {
    Key::from_key_attribute_value(&event.key())
}

pub fn key_text(event: &KeyboardEvent) -> Option<SmolStr> {
    let key = event.key();
    let key = Key::from_key_attribute_value(&key);
    match &key {
        Key::Character(text) => Some(text.clone()),
        Key::Named(NamedKey::Tab) => Some(SmolStr::new("\t")),
        Key::Named(NamedKey::Enter) => Some(SmolStr::new("\r")),
        Key::Named(NamedKey::Space) => Some(SmolStr::new(" ")),
        _ => None,
    }
    .map(SmolStr::new)
}

pub fn key_location(event: &KeyboardEvent) -> KeyLocation {
    match event.location() {
        KeyboardEvent::DOM_KEY_LOCATION_LEFT => KeyLocation::Left,
        KeyboardEvent::DOM_KEY_LOCATION_RIGHT => KeyLocation::Right,
        KeyboardEvent::DOM_KEY_LOCATION_NUMPAD => KeyLocation::Numpad,
        KeyboardEvent::DOM_KEY_LOCATION_STANDARD => KeyLocation::Standard,
        location => {
            tracing::warn!("Unexpected key location: {location}");
            KeyLocation::Standard
        },
    }
}

pub fn keyboard_modifiers(event: &KeyboardEvent) -> ModifiersState {
    let mut state = ModifiersState::empty();

    if event.shift_key() {
        state |= ModifiersState::SHIFT;
    }
    if event.ctrl_key() {
        state |= ModifiersState::CONTROL;
    }
    if event.alt_key() {
        state |= ModifiersState::ALT;
    }
    if event.meta_key() {
        state |= ModifiersState::SUPER;
    }

    state
}

pub fn mouse_modifiers(event: &MouseEvent) -> ModifiersState {
    let mut state = ModifiersState::empty();

    if event.shift_key() {
        state |= ModifiersState::SHIFT;
    }
    if event.ctrl_key() {
        state |= ModifiersState::CONTROL;
    }
    if event.alt_key() {
        state |= ModifiersState::ALT;
    }
    if event.meta_key() {
        state |= ModifiersState::SUPER;
    }

    state
}

pub fn pointer_move_event(event: PointerEventExt) -> impl Iterator<Item = PointerEventExt> {
    // make a single iterator depending on the availability of coalesced events
    if has_coalesced_events_support(&event) {
        None.into_iter().chain(
            Some(event.get_coalesced_events().into_iter().map(PointerEventExt::unchecked_from_js))
                .into_iter()
                .flatten(),
        )
    } else {
        Some(event).into_iter().chain(None.into_iter().flatten())
    }
}

// TODO: Remove when Safari supports `getCoalescedEvents`.
// See <https://bugs.webkit.org/show_bug.cgi?id=210454>.
pub fn has_coalesced_events_support(event: &PointerEvent) -> bool {
    thread_local! {
        static COALESCED_EVENTS_SUPPORT: OnceCell<bool> = const { OnceCell::new() };
    }

    COALESCED_EVENTS_SUPPORT.with(|support| {
        *support.get_or_init(|| {
            #[wasm_bindgen]
            extern "C" {
                type PointerCoalescedEventsSupport;

                #[wasm_bindgen(method, getter, js_name = getCoalescedEvents)]
                fn has_get_coalesced_events(this: &PointerCoalescedEventsSupport) -> JsValue;
            }

            let support: &PointerCoalescedEventsSupport = event.unchecked_ref();
            !support.has_get_coalesced_events().is_undefined()
        })
    })
}
