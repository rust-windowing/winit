#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate glutin;

use glutin::{Event, ElementState};

mod support;

#[cfg(target_os = "android")]
android_start!(main);

#[cfg(not(feature = "window"))]
fn main() { println!("This example requires glutin to be compiled with the `window` feature"); }

#[cfg(feature = "window")]
fn main() {
    let window = glutin::WindowBuilder::new().build().unwrap();
    window.set_title("glutin - Cursor grabbing test");
    unsafe { window.make_current() };

    let context = support::load(&window);
    let mut grabbed = false;
    
    for event in window.poll_events() {
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

            a @ Event::MouseMoved(_) => {
                println!("{:?}", a);
            },

            _ => (),
        }

        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        window.swap_buffers();
    }
}

