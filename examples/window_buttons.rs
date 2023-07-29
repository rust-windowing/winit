#![allow(clippy::single_match)]

// This example is used by developers to test various window functions.

use simple_logger::SimpleLogger;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{DeviceEvents, EventLoop},
    keyboard::Key,
    window::{WindowBuilder, WindowButtons},
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(LogicalSize::new(300.0, 300.0))
        .build(&event_loop)
        .unwrap();

    eprintln!("Window Button keys:");
    eprintln!("  (F) Toggle close button");
    eprintln!("  (G) Toggle maximize button");
    eprintln!("  (H) Toggle minimize button");

    event_loop.listen_device_events(DeviceEvents::Always);

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                logical_key: key,
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => match key.as_ref() {
                Key::Character("F" | "f") => {
                    let buttons = window.enabled_buttons();
                    window.set_enabled_buttons(buttons ^ WindowButtons::CLOSE);
                }
                Key::Character("G" | "g") => {
                    let buttons = window.enabled_buttons();
                    window.set_enabled_buttons(buttons ^ WindowButtons::MAXIMIZE);
                }
                Key::Character("H" | "h") => {
                    let buttons = window.enabled_buttons();
                    window.set_enabled_buttons(buttons ^ WindowButtons::MINIMIZE);
                }
                _ => (),
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            Event::RedrawRequested(_) => {
                fill::fill_window(&window);
            }
            _ => (),
        }
    })
}
