#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;

use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::Key;
use winit::window::WindowBuilder;

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => control_flow.set_exit(),

            // Keyboard input event to handle minimize via a hotkey
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { event, .. },
                window_id,
            } => {
                if window_id == window.id() {
                    // Pressing the 'm' key will minimize the window
                    // WARNING: Consider using `key_without_modifers()` if available on your platform.
                    // See the `key_binding` example
                    if let Key::Character("m") = event.logical_key {
                        window.set_minimized(true);
                    }
                }
            }
            _ => (),
        }
    });
}
