#![allow(clippy::single_match)]

// This example is used by developers to test various window functions.

use simple_logger::SimpleLogger;
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{DeviceEventFilter, EventLoop},
    window::{WindowBuilder, WindowButtons},
};

fn main() {
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

    event_loop.set_device_event_filter(DeviceEventFilter::Never);

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(key),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => match key {
                VirtualKeyCode::F => {
                    let buttons = window.enabled_buttons();
                    window.set_enabled_buttons(buttons ^ WindowButtons::CLOSE);
                }
                VirtualKeyCode::G => {
                    let buttons = window.enabled_buttons();
                    window.set_enabled_buttons(buttons ^ WindowButtons::MAXIMIZE);
                }
                VirtualKeyCode::H => {
                    let buttons = window.enabled_buttons();
                    window.set_enabled_buttons(buttons ^ WindowButtons::MINIMIZE);
                }
                _ => (),
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            _ => (),
        }
    });
}
