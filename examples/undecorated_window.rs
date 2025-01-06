#![allow(unused)]

use std::sync::Arc;

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::Key;
#[cfg(windows)]
use winit::platform::windows::{WindowAttributesExtWindows, WindowExtWindows};
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;

struct App {
    window: Option<Box<dyn Window>>,
    shadow: bool,
}

impl Default for App {
    fn default() -> Self {
        Self { window: None, shadow: true }
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let mut attrs = WindowAttributes::default().with_decorations(false);
        #[cfg(windows)]
        {
            attrs = attrs.with_undecorated_shadow(true);
        }

        self.window = Some(event_loop.create_window(attrs).unwrap());
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            #[cfg(windows)]
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key: Key::Character(c), state: ElementState::Pressed, .. },
                ..
            } if c == "x" => {
                self.shadow = !self.shadow;
                self.window.as_ref().unwrap().set_undecorated_shadow(self.shadow);
            },
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            WindowEvent::RedrawRequested => {
                let window = self.window.as_ref().unwrap();
                fill::fill_window_with_border(window.as_ref());
                window.request_redraw();
            },
            _ => (),
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap()
}
