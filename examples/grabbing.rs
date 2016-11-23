extern crate winit;

use winit::{Event, ElementState};

fn main() {
    let window = winit::WindowBuilder::new().build().unwrap();
    window.set_title("winit - Cursor grabbing test");

    let mut grabbed = false;

    for event in window.wait_events() {
        match event {
            Event::KeyboardInput(ElementState::Pressed, _, _) => {
                if grabbed {
                    grabbed = false;
                    window.set_cursor_state(winit::CursorState::Normal)
                          .ok().expect("could not ungrab mouse cursor");
                } else {
                    grabbed = true;
                    window.set_cursor_state(winit::CursorState::Grab)
                          .ok().expect("could not grab mouse cursor");
                }
            },

            Event::Closed => break,

            a @ Event::MouseMoved(_, _) => {
                println!("{:?}", a);
            },

            _ => (),
        }
    }
}
