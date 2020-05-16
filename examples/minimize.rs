extern crate winit;

use winit::event::{Event, LogicalKey, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent(_, WindowEvent::CloseRequested) => *control_flow = ControlFlow::Exit,

            // Keyboard input event to handle minimize via a hotkey
            Event::WindowEvent(window_id, WindowEvent::KeyPress(e))
                if e.is_down() && e.logical_key_is(LogicalKey::M) && window_id == window.id() =>
            {
                window.set_minimized(true)
            }
            _ => (),
        }
    });
}
