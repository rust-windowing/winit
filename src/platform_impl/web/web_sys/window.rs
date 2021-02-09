use super::gamepad;
use crate::error::OsError as RootOE;
use crate::platform_impl::OsError;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::GamepadEvent;

#[derive(Debug)]
pub struct Shared(pub Rc<RefCell<Window>>);

#[derive(Debug)]
pub struct Window {
    raw: web_sys::Window,
    on_gamepad_connected: Option<Closure<dyn FnMut(GamepadEvent)>>,
    on_gamepad_disconnected: Option<Closure<dyn FnMut(GamepadEvent)>>,
}

impl Shared {
    pub fn create() -> Result<Self, RootOE> {
        let global = Window::create()?;
        Ok(Shared(Rc::new(RefCell::new(global))))
    }
}

impl Clone for Shared {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl Window {
    pub fn create() -> Result<Self, RootOE> {
        let raw =
            web_sys::window().ok_or(os_error!(OsError("Failed to obtain window".to_owned())))?;

        Ok(Window {
            raw,
            on_gamepad_connected: None,
            on_gamepad_disconnected: None,
        })
    }

    pub fn on_gamepad_connected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(gamepad::Gamepad),
    {
        self.on_gamepad_connected = Some(self.add_event(
            "gamepadconnected",
            move |event: GamepadEvent| {
                let gamepad = event
                    .gamepad()
                    .expect("[gamepadconnected] expected gamepad");
                handler(gamepad::Gamepad::new(gamepad));
            },
        ))
    }

    pub fn on_gamepad_disconnected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(gamepad::Gamepad),
    {
        self.on_gamepad_disconnected = Some(self.add_event(
            "gamepaddisconnected",
            move |event: GamepadEvent| {
                let gamepad = event
                    .gamepad()
                    .expect("[gamepaddisconnected] expected gamepad");
                handler(gamepad::Gamepad::new(gamepad));
            },
        ))
    }

    fn add_event<E, F>(&self, event_name: &str, mut handler: F) -> Closure<dyn FnMut(E)>
    where
        E: 'static + AsRef<web_sys::Event> + wasm_bindgen::convert::FromWasmAbi,
        F: 'static + FnMut(E),
    {
        let closure = Closure::wrap(Box::new(move |event: E| {
            handler(event);
        }) as Box<dyn FnMut(E)>);

        self.raw
            .add_event_listener_with_callback(event_name, &closure.as_ref().unchecked_ref())
            .expect("Failed to add event listener with callback");

        closure
    }
}
