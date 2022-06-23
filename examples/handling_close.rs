#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{Event, KeyboardInput, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_title("Your faithful window")
        .build(&event_loop)
        .unwrap();

    let mut close_requested = false;

    event_loop.run(move |event, _, control_flow| {
        use winit::event::{
            ElementState::Released,
            VirtualKeyCode::{N, Y},
        };
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::CloseRequested => {
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
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(virtual_code),
                                state: Released,
                                ..
                            },
                        ..
                    } => {
                        match virtual_code {
                            Y => {
                                if close_requested {
                                    // This is where you'll want to do any cleanup you need.
                                    println!("Buh-bye!");

                                    // For a single-window application like this, you'd normally just
                                    // break out of the event loop here. If you wanted to keep running the
                                    // event loop (i.e. if it's a multi-window application), you need to
                                    // drop the window. That closes it, and results in `Destroyed` being
                                    // sent.
                                    control_flow.set_exit();
                                }
                            }
                            N => {
                                if close_requested {
                                    println!("Your window will continue to stay by your side.");
                                    close_requested = false;
                                }
                            }
                            _ => (),
                        }
                    }
                    _ => (),
                }
            }
            _ => (),
        }
    });
}
