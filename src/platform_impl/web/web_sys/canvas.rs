use super::event;
use super::gamepad::{GamepadManagerShared, GamepadShared};
use crate::dpi::{LogicalPosition, LogicalSize};
use crate::error::OsError as RootOE;
use crate::event::{ModifiersState, MouseButton, ScanCode, VirtualKeyCode};
use crate::platform_impl::OsError;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::{
    Event, FocusEvent, GamepadEvent, HtmlCanvasElement, KeyboardEvent, PointerEvent, WheelEvent,
};

pub struct Canvas {
    raw: HtmlCanvasElement,
    window: web_sys::Window,
    gamepad_manager: GamepadManagerShared,
    on_focus: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_blur: Option<Closure<dyn FnMut(FocusEvent)>>,
    on_keyboard_release: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_keyboard_press: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_received_character: Option<Closure<dyn FnMut(KeyboardEvent)>>,
    on_cursor_leave: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_cursor_enter: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_cursor_move: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_press: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_release: Option<Closure<dyn FnMut(PointerEvent)>>,
    on_mouse_wheel: Option<Closure<dyn FnMut(WheelEvent)>>,
    on_fullscreen_change: Option<Closure<dyn FnMut(Event)>>,
    on_gamepad_connected: Option<Closure<dyn FnMut(GamepadEvent)>>,
    on_gamepad_disconnected: Option<Closure<dyn FnMut(GamepadEvent)>>,
    wants_fullscreen: Rc<RefCell<bool>>,
}

impl Drop for Canvas {
    fn drop(&mut self) {
        self.raw.remove();
    }
}

impl Canvas {
    pub fn create() -> Result<Self, RootOE> {
        let window =
            web_sys::window().ok_or(os_error!(OsError("Failed to obtain window".to_owned())))?;

        let document = window
            .document()
            .ok_or(os_error!(OsError("Failed to obtain document".to_owned())))?;

        let canvas: HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?
            .unchecked_into();

        // A tabindex is needed in order to capture local keyboard events.
        // A "0" value means that the element should be focusable in
        // sequential keyboard navigation, but its order is defined by the
        // document's source order.
        // https://developer.mozilla.org/en-US/docs/Web/HTML/Global_attributes/tabindex
        canvas
            .set_attribute("tabindex", "0")
            .map_err(|_| os_error!(OsError("Failed to set a tabindex".to_owned())))?;

        let gamepad_manager = GamepadManagerShared::create();

        Ok(Canvas {
            raw: canvas,
            window,
            gamepad_manager,
            on_blur: None,
            on_focus: None,
            on_keyboard_release: None,
            on_keyboard_press: None,
            on_received_character: None,
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_mouse_release: None,
            on_mouse_press: None,
            on_mouse_wheel: None,
            on_fullscreen_change: None,
            on_gamepad_connected: None,
            on_gamepad_disconnected: None,
            wants_fullscreen: Rc::new(RefCell::new(false)),
        })
    }

    pub fn set_attribute(&self, attribute: &str, value: &str) {
        self.raw
            .set_attribute(attribute, value)
            .expect(&format!("Set attribute: {}", attribute));
    }

    pub fn position(&self) -> (f64, f64) {
        let bounds = self.raw.get_bounding_client_rect();

        (bounds.x(), bounds.y())
    }

    pub fn width(&self) -> f64 {
        self.raw.width() as f64
    }

    pub fn height(&self) -> f64 {
        self.raw.height() as f64
    }

    pub fn set_size(&self, size: LogicalSize) {
        self.raw.set_width(size.width as u32);
        self.raw.set_height(size.height as u32);
    }

    pub fn raw(&self) -> &HtmlCanvasElement {
        &self.raw
    }

    pub fn on_blur<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_blur = Some(self.add_event(
            "blur",
            move |_: FocusEvent| {
                handler();
            },
            false,
        ));
    }

    pub fn on_focus<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_focus = Some(self.add_event(
            "focus",
            move |_: FocusEvent| {
                handler();
            },
            false,
        ));
    }

    pub fn on_keyboard_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_release = Some(self.add_user_event(
            "keyup",
            move |event: KeyboardEvent| {
                handler(
                    event::scan_code(&event),
                    event::virtual_key_code(&event),
                    event::keyboard_modifiers(&event),
                );
            },
            false,
        ));
    }

    pub fn on_keyboard_press<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(ScanCode, Option<VirtualKeyCode>, ModifiersState),
    {
        self.on_keyboard_press = Some(self.add_user_event(
            "keydown",
            move |event: KeyboardEvent| {
                handler(
                    event::scan_code(&event),
                    event::virtual_key_code(&event),
                    event::keyboard_modifiers(&event),
                );
            },
            false,
        ));
    }

    pub fn on_received_character<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(char),
    {
        // TODO: Use `beforeinput`.
        //
        // The `keypress` event is deprecated, but there does not seem to be a
        // viable/compatible alternative as of now. `beforeinput` is still widely
        // unsupported.
        self.on_received_character = Some(self.add_event(
            "keypress",
            move |event: KeyboardEvent| {
                handler(event::codepoint(&event));
            },
            false,
        ));
    }

    pub fn on_mouse_release<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton),
    {
        self.on_mouse_release = Some(self.add_event(
            "pointerup",
            move |event: PointerEvent| {
                handler(event.pointer_id(), event::mouse_button(&event));
            },
            false,
        ));
    }

    pub fn on_mouse_press<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, MouseButton),
    {
        self.on_mouse_press = Some(self.add_event(
            "pointerdown",
            move |event: PointerEvent| {
                handler(event.pointer_id(), event::mouse_button(&event));
            },
            false,
        ));
    }

    pub fn on_mouse_wheel<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(i32, (f64, f64)),
    {
        self.on_mouse_wheel = Some(self.add_event(
            "wheel",
            move |event: WheelEvent| {
                let delta = event::mouse_scroll_delta(&event);
                handler(0, delta);
            },
            false,
        ));
    }

    pub fn on_cursor_leave<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_cursor_leave = Some(self.add_event(
            "pointerout",
            move |_event: PointerEvent| {
                handler();
            },
            false,
        ));
    }

    pub fn on_cursor_enter<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_cursor_enter = Some(self.add_event(
            "pointerover",
            move |_event: PointerEvent| {
                handler();
            },
            false,
        ));
    }

    pub fn on_cursor_move<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(LogicalPosition, ModifiersState),
    {
        self.on_cursor_move = Some(self.add_event(
            "pointermove",
            move |event: PointerEvent| {
                handler(
                    event::mouse_position(&event),
                    event::mouse_modifiers(&event),
                );
            },
            false,
        ));
    }

    pub fn on_fullscreen_change<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(),
    {
        self.on_fullscreen_change =
            Some(self.add_event("fullscreenchange", move |_: Event| handler(), false));
    }

    pub fn on_gamepad_connected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(GamepadShared),
    {
        let m = self.gamepad_manager.clone();
        self.on_gamepad_connected = Some(self.add_event(
            "gamepadconnected",
            move |event: GamepadEvent| {
                let gamepad = event
                    .gamepad()
                    .expect("[gamepadconnected] expected gamepad");
                let g = m.register(gamepad);
                handler(g);
            },
            false,
        ))
    }

    pub fn on_gamepad_disconnected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(GamepadShared),
    {
        let m = self.gamepad_manager.clone();
        self.on_gamepad_disconnected = Some(self.add_event(
            "gamepaddisconnected",
            move |event: GamepadEvent| {
                let gamepad = event
                    .gamepad()
                    .expect("[gamepaddisconnected] expected gamepad");
                let g = m.register(gamepad);
                handler(g);
            },
            false,
        ))
    }

    fn add_event<E, F>(
        &self,
        event_name: &str,
        mut handler: F,
        is_global: bool,
    ) -> Closure<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let closure = Closure::wrap(Box::new(move |event: E| {
            {
                let event_ref = event.as_ref();
                event_ref.stop_propagation();
                event_ref.cancel_bubble();
            }

            handler(event);
        }) as Box<dyn FnMut(E)>);

        if is_global {
            self.window
                .add_event_listener_with_callback(event_name, &closure.as_ref().unchecked_ref())
                .expect("Failed to add event listener with callback");
        } else {
            self.raw
                .add_event_listener_with_callback(event_name, &closure.as_ref().unchecked_ref())
                .expect("Failed to add event listener with callback");
        }

        closure
    }

    // The difference between add_event and add_user_event is that the latter has a special meaning
    // for browser security. A user event is a deliberate action by the user (like a mouse or key
    // press) and is the only time things like a fullscreen request may be successfully completed.)
    fn add_user_event<E, F>(
        &self,
        event_name: &str,
        mut handler: F,
        is_global: bool,
    ) -> Closure<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let wants_fullscreen = self.wants_fullscreen.clone();
        let canvas = self.raw.clone();

        self.add_event(
            event_name,
            move |event: E| {
                handler(event);

                if *wants_fullscreen.borrow() {
                    canvas
                        .request_fullscreen()
                        .expect("Failed to enter fullscreen");
                    *wants_fullscreen.borrow_mut() = false;
                }
            },
            is_global,
        )
    }

    pub fn request_fullscreen(&self) {
        *self.wants_fullscreen.borrow_mut() = true;
    }

    pub fn is_fullscreen(&self) -> bool {
        super::is_fullscreen(&self.raw)
    }
}
