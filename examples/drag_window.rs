#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{
        ElementState, Event, KeyboardInput, MouseButton, StartCause, VirtualKeyCode, WindowEvent,
    },
    event_loop::EventLoop,
    window::{Window, WindowBuilder, WindowId},
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window_1 = WindowBuilder::new().build(&event_loop).unwrap();
    let window_2 = WindowBuilder::new().build(&event_loop).unwrap();

    let mut switched = false;
    let mut entered_id = window_2.id();

    event_loop.run(move |event, _, control_flow| match event {
        Event::NewEvents(StartCause::Init) => {
            eprintln!("Switch which window is to be dragged by pressing \"x\".")
        }
        Event::WindowEvent { event, window_id } => match event {
            WindowEvent::CloseRequested => control_flow.set_exit(),
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                let window = if (window_id == window_1.id() && switched)
                    || (window_id == window_2.id() && !switched)
                {
                    &window_2
                } else {
                    &window_1
                };

                window.drag_window().unwrap()
            }
            WindowEvent::CursorEntered { .. } => {
                entered_id = window_id;
                name_windows(entered_id, switched, &window_1, &window_2)
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Released,
                        virtual_keycode: Some(VirtualKeyCode::X),
                        ..
                    },
                ..
            } => {
                switched = !switched;
                name_windows(entered_id, switched, &window_1, &window_2);
                println!("Switched!")
            }
            _ => (),
        },
        _ => (),
    });
}

fn name_windows(window_id: WindowId, switched: bool, window_1: &Window, window_2: &Window) {
    let (drag_target, other) =
        if (window_id == window_1.id() && switched) || (window_id == window_2.id() && !switched) {
            (&window_2, &window_1)
        } else {
            (&window_1, &window_2)
        };
    drag_target.set_title("drag target");
    other.set_title("winit window");
}
