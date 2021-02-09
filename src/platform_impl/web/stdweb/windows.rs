use super::gamepad;
use crate::error::OsError as RootOE;
use std::{cell::RefCell, rc::Rc};
use stdweb::web;
use stdweb::web::{event::IGamepadEvent, IEventTarget};

#[derive(Debug)]
pub struct Shared(pub Rc<RefCell<Window>>);

#[derive(Debug)]
pub struct Window {
    raw: web::Window,
    on_gamepad_connected: Option<web::EventListenerHandle>,
    on_gamepad_disconnected: Option<web::EventListenerHandle>,
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
        let raw = stdweb::web::window();

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
            move |event: stdweb::web::event::GamepadConnectedEvent| {
                let gamepad = event.gamepad();
                handler(gamepad::Gamepad::new(gamepad));
            },
        ));
    }

    pub fn on_gamepad_disconnected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(gamepad::Gamepad),
    {
        self.on_gamepad_connected = Some(self.add_event(
            move |event: stdweb::web::event::GamepadDisconnectedEvent| {
                let gamepad = event.gamepad();
                handler(gamepad::Gamepad::new(gamepad));
            },
        ));
    }

    fn add_event<E, F>(&self, mut handler: F) -> web::EventListenerHandle
    where
        E: web::event::ConcreteEvent,
        F: 'static + FnMut(E),
    {
        self.raw.add_event_listener(move |event: E| {
            event.stop_propagation();
            event.cancel_bubble();

            handler(event);
        })
    }
}
