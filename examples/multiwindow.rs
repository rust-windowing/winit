#![allow(clippy::single_match)]

use std::collections::HashMap;

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::Key,
    window::{Window, WindowId},
    ApplicationHandler,
};

#[path = "util/fill.rs"]
mod fill;

#[derive(Debug)]
struct App {
    windows: HashMap<WindowId, Window>,
}

impl ApplicationHandler for App {
    type Suspended = Self;

    fn resume(
        suspended: Self::Suspended,
        _elwt: &winit::event_loop::EventLoopWindowTarget,
    ) -> Self {
        suspended
    }

    fn suspend(self) -> Self::Suspended {
        self
    }

    fn window_event(
        &mut self,
        elwt: &winit::event_loop::EventLoopWindowTarget,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Window {window_id:?} has received the signal to close");

                // This drops the window, causing it to close.
                self.windows.remove(&window_id);

                if self.windows.is_empty() {
                    elwt.exit();
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: Key::Character(c),
                        ..
                    },
                is_synthetic: false,
                ..
            } if matches!(c.as_ref(), "n" | "N") => {
                let window = Window::new(elwt).unwrap();
                println!("Opened a new window: {:?}", window.id());
                self.windows.insert(window.id(), window);
            }
            WindowEvent::RedrawRequested => {
                if let Some(window) = self.windows.get(&window_id) {
                    fill::fill_window(window);
                }
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _elwt: &winit::event_loop::EventLoopWindowTarget) {
        // self.window.request_redraw();
    }
}

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    println!("Press N to open a new window.");

    event_loop.run_with::<App>(|elwt| {
        elwt.set_wait();

        let mut windows = HashMap::new();
        for _ in 0..3 {
            let window = Window::new(elwt).unwrap();
            println!("Opened a new window: {:?}", window.id());
            windows.insert(window.id(), window);
        }

        App { windows }
    })
}
