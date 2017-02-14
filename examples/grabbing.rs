extern crate winit;

use winit::{WindowEvent, ElementState};

fn main() {
    let events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new().build(&events_loop).unwrap();
    window.set_title("winit - Cursor grabbing test");

    let mut grabbed = false;

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            winit::Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::KeyboardInput(ElementState::Pressed, _, _) => {
                        if grabbed {
                            grabbed = false;
                            window.set_cursor_state(winit::CursorState::Normal)
                                .ok()
                                .expect("could not ungrab mouse cursor");
                        } else {
                            grabbed = true;
                            window.set_cursor_state(winit::CursorState::Grab)
                                .ok()
                                .expect("could not grab mouse cursor");
                        }
                    }

                    WindowEvent::Closed => events_loop.interrupt(),

                    a @ WindowEvent::MouseMoved(_, _) => {
                        println!("{:?}", a);
                    }

                    _ => (),
                }
            }
        }
    });
}
