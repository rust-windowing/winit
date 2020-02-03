use super::gamepad::{SharedGamepadManager, SharedGamepad};
use crate::error::OsError as RootOE;
use crate::platform_impl::OsError;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{closure::Closure, JsCast};
use web_sys::GamepadEvent;

pub struct SharedWindow(pub Rc<RefCell<Window>>);

pub struct Window {
    raw: web_sys::Window,
    gamepad_manager: SharedGamepadManager,
    on_gamepad_connected: Option<Closure<dyn FnMut(GamepadEvent)>>,
    on_gamepad_disconnected: Option<Closure<dyn FnMut(GamepadEvent)>>,
}

impl SharedWindow {
    pub fn new() -> Self {
        let global = Window::create().unwrap();
        SharedWindow(Rc::new(RefCell::new(global)))
    }
}

impl Clone for SharedWindow {
    fn clone(&self) -> Self {
        SharedWindow(self.0.clone())
    }
}

impl Window {
    pub fn create() -> Result<Window, RootOE> {
        let raw =
            web_sys::window().ok_or(os_error!(OsError("Failed to obtain window".to_owned())))?;

        let gamepad_manager = SharedGamepadManager::create();

        Ok(Window {
            raw,
            gamepad_manager,
            on_gamepad_connected: None,
            on_gamepad_disconnected: None,
        })
    }

    pub fn on_gamepad_connected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(SharedGamepad),
    {
        let manager = self.gamepad_manager.clone().manager();
        self.on_gamepad_connected = Some(self.add_event(
            "gamepadconnected",
            move |event: GamepadEvent| {
                let gamepad = event
                    .gamepad()
                    .expect("[gamepadconnected] expected gamepad");
                let g_index = manager.register(gamepad);
                let g = manager.get(&g_index).expect("[gamepadconnected] Gamepad expected");
                handler(g);
            },
        ))
    }

    pub fn on_gamepad_disconnected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(SharedGamepad),
    {
        let manager = self.gamepad_manager.clone().manager();
        self.on_gamepad_disconnected = Some(self.add_event(
            "gamepaddisconnected",
            move |event: GamepadEvent| {
                let gamepad = event
                    .gamepad()
                    .expect("[gamepaddisconnected] expected gamepad");
                let g_index = manager.register(gamepad);
                let g = manager.get(&g_index).expect("[gamepaddisconnected] Gamepad expected");
                handler(g);
            },
        ))
    }

    fn add_event<E, F>(&self, event_name: &str, mut handler: F) -> Closure<dyn FnMut(E)>
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

        self.raw
            .add_event_listener_with_callback(event_name, &closure.as_ref().unchecked_ref())
            .expect("Failed to add event listener with callback");

        closure
    }
}
