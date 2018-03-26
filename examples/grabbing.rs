extern crate winit;

use winit::{ControlFlow, WindowEvent, ElementState, KeyboardInput};

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new().build(&events_loop).unwrap();
    window.set_title("winit - Cursor grabbing test");

    let mut grabbed = false;

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::KeyboardInput { input: KeyboardInput { state: ElementState::Pressed, .. }, .. } => {
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

                    WindowEvent::CloseRequested => return ControlFlow::Break,

                    a @ WindowEvent::CursorMoved { .. } => {
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
