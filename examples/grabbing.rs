#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate glutin;

use glutin::{Event, ElementState};

mod support;

#[cfg(target_os = "android")]
android_start!(main);

fn main() {
    let window = glutin::WindowBuilder::new().build().unwrap();
    window.set_title("glutin - Cursor grabbing test");
    let _ = unsafe { window.make_current() };

    let context = support::load(&window);
    let mut grabbed = false;

    for event in window.wait_events() {
        match event {
            Event::KeyboardInput(ElementState::Pressed, _, _) => {
                if grabbed {
                    grabbed = false;
                    window.set_cursor_state(glutin::CursorState::Normal)
                          .ok().expect("could not ungrab mouse cursor");
                } else {
                    grabbed = true;
                    window.set_cursor_state(glutin::CursorState::Grab)
                          .ok().expect("could not grab mouse cursor");
                }
            },

            Event::Closed => break,

            a @ Event::MouseMoved(_, _) => {
                println!("{:?}", a);
            },

            _ => (),
        }

        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        let _ = window.swap_buffers();
    }
}
