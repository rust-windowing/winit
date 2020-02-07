use super::gamepad;
use super::gamepad_manager;
use crate::error::OsError as RootOE;
use crate::event::device;
use crate::platform_impl::OsError;
use std::{cell::RefCell, rc::Rc};
use stdweb::web;
use stdweb::web::{IEventTarget, event::IGamepadEvent};

pub struct Shared(pub Rc<RefCell<Window>>);

pub struct Window {
    raw: web::Window,
    gamepad_manager: gamepad_manager::Shared,
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

        let gamepad_manager = gamepad_manager::Shared::create();

        Ok(Window {
            raw,
            gamepad_manager,
            on_gamepad_connected: None,
            on_gamepad_disconnected: None,
        })
    }

    pub fn collect_gamepad_events(
        &self,
        events: &mut Vec<(gamepad::Gamepad, device::GamepadEvent)>,
    ) {
        let manager = self.gamepad_manager.clone().manager();
        manager.collect_events(events);
    }

    pub fn on_gamepad_connected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(gamepad::Gamepad),
    {
        let manager = self.gamepad_manager.clone().manager();
        self.on_gamepad_connected = Some(self.add_event(
            move |event: stdweb::web::event::GamepadConnectedEvent| {
                let gamepad = event.gamepad();
                let g = manager.register(gamepad);
                handler(g);
            },
        ));
    }

    pub fn on_gamepad_disconnected<F>(&mut self, mut handler: F)
    where
        F: 'static + FnMut(gamepad::Gamepad),
    {
        let manager = self.gamepad_manager.clone().manager();
        self.on_gamepad_connected = Some(self.add_event(
            move |event: stdweb::web::event::GamepadDisconnectedEvent| {
                let gamepad = event.gamepad();
                let g = manager.register(gamepad);
                handler(g);
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
