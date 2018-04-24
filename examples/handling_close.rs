extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("Your faithful window")
        .build(&events_loop)
        .unwrap();

    let mut close_requested = false;

    events_loop.run_forever(|event| {
        use winit::WindowEvent::*;
        use winit::ElementState::Released;
        use winit::VirtualKeyCode::{N, Y};

        match event {
            winit::Event::WindowEvent { event, .. } => match event {
                CloseRequested => {
                    // `CloseRequested` is sent when the close button on the window is pressed (or
                    // through whatever other mechanisms the window manager provides for closing a
                    // window). If you don't handle this event, the close button won't actually do
                    // anything.

                    // A common thing to do here is prompt the user if they have unsaved work.
                    // Creating a proper dialog box for that is far beyond the scope of this
                    // example, so here we'll just respond to the Y and N keys.
                    println!("Are you ready to bid your window farewell? [Y/N]");
                    close_requested = true;

                    // In applications where you can safely close the window without further
                    // action from the user, this is generally where you'd handle cleanup before
                    // closing the window. How to close the window is detailed in the handler for
                    // the Y key.
                }
                KeyboardInput {
                    input:
                        winit::KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state: Released,
                            ..
                        },
                    ..
                } => match virtual_code {
                    Y => {
                        if close_requested {
                            // This is where you'll want to do any cleanup you need.
                            println!("Buh-bye!");

                            // For a single-window application like this, you'd normally just
                            // break out of the event loop here. If you wanted to keep running the
                            // event loop (i.e. if it's a multi-window application), you need to
                            // drop the window. That closes it, and results in `Destroyed` being
                            // sent.
                            return winit::ControlFlow::Break;
                        }
                    }
                    N => {
                        if close_requested {
                            println!("Your window will continue to stay by your side.");
                            close_requested = false;
                        }
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => (),
        }

        winit::ControlFlow::Continue
    });
}
