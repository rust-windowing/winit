#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{ButtonId, DeviceId, ElementState, KeyEvent, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    handler::{ApplicationHandler, DeviceEventHandler},
    keyboard::{Key, ModifiersState, NamedKey},
    window::{CursorGrabMode, Window, WindowBuilder, WindowId},
};

#[path = "util/fill.rs"]
mod fill;

struct App {
    window: Window,
    modifiers: ModifiersState,
}

impl ApplicationHandler for App {
    fn window_event(
        &mut self,
        active: ActiveEventLoop<'_>,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => active.exit(),
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: key,
                        state: ElementState::Released,
                        ..
                    },
                ..
            } => {
                let result = match key {
                    Key::Named(NamedKey::Escape) => {
                        active.exit();
                        Ok(())
                    }
                    Key::Character(ch) => match ch.to_lowercase().as_str() {
                        "g" => self.window.set_cursor_grab(CursorGrabMode::Confined),
                        "l" => self.window.set_cursor_grab(CursorGrabMode::Locked),
                        "a" => self.window.set_cursor_grab(CursorGrabMode::None),
                        "h" => {
                            self.window.set_cursor_visible(self.modifiers.shift_key());
                            Ok(())
                        }
                        _ => Ok(()),
                    },
                    _ => Ok(()),
                };

                if let Err(err) = result {
                    println!("error: {err}");
                }
            }
            WindowEvent::ModifiersChanged(new) => self.modifiers = new.state(),
            WindowEvent::RedrawRequested => fill::fill_window(&self.window),
            _ => (),
        }
    }

    fn device_event(&mut self) -> Option<&mut dyn DeviceEventHandler> {
        Some(self)
    }
}

impl DeviceEventHandler for App {
    fn mouse_motion(
        &mut self,
        _active: ActiveEventLoop<'_>,
        _device_id: DeviceId,
        delta: (f64, f64),
    ) {
        println!("mouse moved: {delta:?}");
    }

    fn button(
        &mut self,
        _active: ActiveEventLoop<'_>,
        _device_id: DeviceId,
        button: ButtonId,
        state: ElementState,
    ) {
        match state {
            ElementState::Pressed => println!("mouse button {button} pressed"),
            ElementState::Released => println!("mouse button {button} released"),
        }
    }
}

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("Super Cursor Grab'n'Hide Simulator 9000")
        .build(&event_loop)
        .unwrap();

    event_loop.run_with(App {
        window,
        modifiers: ModifiersState::default(),
    })
}
