extern crate keyboard_types;
extern crate winit;

use keyboard_types::{KeyEvent, KeyState};
use winit::{ControlFlow, WindowEvent};

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new().build(&events_loop).unwrap();
    window.set_title("winit - Cursor grabbing test");

    let mut grabbed = false;

    if cfg!(target_os = "linux") {
        println!("Running this example under wayland may not display a window at all.\n\
                  This is normal and because this example does not actually draw anything in the window,\
                  thus the compositor does not display it.");
    }

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::KeyboardInput { input: KeyEvent { state: KeyState::Down, .. }, .. } => {
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

                    WindowEvent::Closed => return ControlFlow::Break,

                    a @ WindowEvent::MouseMoved { .. } => {
                        println!("{:?}", a);
                    },

                    _ => (),
                }
            }
            _ => {}
        }

        ControlFlow::Continue
    });
}
