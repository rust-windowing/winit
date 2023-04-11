#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{DeviceEvent, ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, ModifiersState},
    window::{CursorGrabMode, WindowBuilder},
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Super Cursor Grab'n'Hide Simulator 9000")
        .build(&event_loop)
        .unwrap();

    let mut modifiers = ModifiersState::default();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => control_flow.set_exit(),
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
                        Key::Escape => {
                            control_flow.set_exit();
                            Ok(())
                        }
                        Key::Character(ch) => match ch.to_lowercase().as_str() {
                            "g" => window.set_cursor_grab(CursorGrabMode::Confined),
                            "l" => window.set_cursor_grab(CursorGrabMode::Locked),
                            "a" => window.set_cursor_grab(CursorGrabMode::None),
                            "h" => {
                                window.set_cursor_visible(modifiers.shift_key());
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
                WindowEvent::ModifiersChanged(new) => modifiers = new.state(),
                _ => (),
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta } => println!("mouse moved: {delta:?}"),
                DeviceEvent::Button { button, state } => match state {
                    ElementState::Pressed => println!("mouse button {button} pressed"),
                    ElementState::Released => println!("mouse button {button} released"),
                },
                _ => (),
            },
            Event::RedrawRequested(_) => fill::fill_window(&window),
            _ => (),
        }
    })
}
