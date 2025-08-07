use std::cell::OnceCell;

use dpi::{LogicalPosition, PhysicalPosition, Position};
use smol_str::SmolStr;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{KeyboardEvent, MouseEvent, Navigator, PointerEvent, WheelEvent};
use winit_core::event::{FingerId, MouseScrollDelta, PointerKind};
use winit_core::keyboard::{
    Key, KeyCode, KeyLocation, ModifiersState, NamedKey, NativeKey, NativeKeyCode, PhysicalKey,
};

use super::Engine;

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

pub fn mouse_buttons(event: &MouseEvent) -> ButtonsState {
    ButtonsState::from_bits_retain(event.buttons())
}

// https://www.w3.org/TR/pointerevents3/#the-button-property
#[derive(Clone, Copy)]
pub struct MouseButton(u16);

impl MouseButton {
    // raw code
    pub fn raw(self) -> u16 {
        self.0
    }

    // Map known buttons to [`ButtonsState`], unknown to an empty state
    pub fn state(self) -> ButtonsState {
        match self.0 {
            0 => ButtonsState::LEFT,
            1 => ButtonsState::MIDDLE,
            2 => ButtonsState::RIGHT,
            3 => ButtonsState::BACK,
            4 => ButtonsState::FORWARD,
            _ => ButtonsState::empty(),
        }
    }
}

pub fn mouse_button(event: &MouseEvent) -> Option<MouseButton> {
    // NOTE: any negative value gets mapped to `None`, though only -1 is documented
    event.button().try_into().ok().map(MouseButton)
}

impl From<MouseButton> for winit_core::event::MouseButton {
    fn from(button: MouseButton) -> Self {
        use winit_core::event::MouseButton as MB;

        match button.0 {
            0 => MB::Left,
            1 => MB::Middle,
            2 => MB::Right,
            3 => MB::Back,
            4 => MB::Forward,
            i => MB::Other(i),
        }
    }
}

pub fn mouse_position(event: &MouseEvent) -> LogicalPosition<f64> {
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
    pub fn init(navigator: &Navigator, event: &PointerEvent) -> Self {
        match super::engine(navigator) {
            Some(Engine::Chromium) => Self::Chromium,
            // Firefox has wrong movement values in coalesced events.
            Some(Engine::Gecko) if has_coalesced_events_support(event) => Self::Gecko {
                old_position: mouse_position(event),
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
                let new_position = mouse_position(event);
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

pub fn pointer_type(event: &PointerEvent, pointer_id: i32) -> PointerKind {
    match event.pointer_type().as_str() {
        "mouse" => PointerKind::Mouse,
        "touch" => PointerKind::Touch(FingerId::from_raw(pointer_id as usize)),
        _ => PointerKind::Unknown,
    }
}

pub fn key_code(event: &KeyboardEvent) -> PhysicalKey {
    // Use keyboard-types' parsing (it is based on the W3C standard).
    match event.code().parse() {
        Ok(KeyCode::Unidentified) => PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
        Ok(code) => PhysicalKey::Code(code),
        Err(err) => {
            tracing::warn!("unknown keyboard input: {err}");
            PhysicalKey::Unidentified(NativeKeyCode::Unidentified)
        },
    }
}

pub fn key(event: &KeyboardEvent) -> Key {
    let key = event.key();
    // Use keyboard-types' parsing (it is based on the W3C standard).
    match key.parse() {
        Ok(NamedKey::Unidentified) => {
            Key::Unidentified(NativeKey::Web(SmolStr::new("Unidentified")))
        },
        Ok(NamedKey::Dead) => Key::Dead(None),
        Ok(named) => Key::Named(named),
        Err(_) => Key::Character(SmolStr::new(key)),
    }
}

pub fn key_text(event: &KeyboardEvent) -> Option<SmolStr> {
    let key = key(event);
    match &key {
        Key::Character(text) => Some(text.clone()),
        Key::Named(NamedKey::Tab) => Some(SmolStr::new("\t")),
        Key::Named(NamedKey::Enter) => Some(SmolStr::new("\r")),
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
        state |= ModifiersState::META;
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
        state |= ModifiersState::META;
    }

    state
}

pub fn pointer_move_event(event: PointerEvent) -> impl Iterator<Item = PointerEvent> {
    // make a single iterator depending on the availability of coalesced events
    if has_coalesced_events_support(&event) {
        None.into_iter().chain(
            Some(event.get_coalesced_events().into_iter().map(PointerEvent::unchecked_from_js))
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
