#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::Key,
    window::WindowBuilder,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("Your faithful window")
        .build(&event_loop)
        .unwrap();

    let mut close_requested = false;

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
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
                    event:
                        KeyEvent {
                            logical_key: key,
                            state: ElementState::Released,
                            ..
                        },
                    ..
                } => {
                    // WARNING: Consider using `key_without_modifers()` if available on your platform.
                    // See the `key_binding` example
                    match key.as_ref() {
                        Key::Character("y") => {
                            if close_requested {
                                // This is where you'll want to do any cleanup you need.
                                println!("Buh-bye!");

                                // For a single-window application like this, you'd normally just
                                // break out of the event loop here. If you wanted to keep running the
                                // event loop (i.e. if it's a multi-window application), you need to
                                // drop the window. That closes it, and results in `Destroyed` being
                                // sent.
                                elwt.exit();
                            }
                        }
                        Key::Character("n") => {
                            if close_requested {
                                println!("Your window will continue to stay by your side.");
                                close_requested = false;
                            }
                        }
                        _ => (),
                    }
                }
                WindowEvent::RedrawRequested => {
                    fill::fill_window(&window);
                }
                _ => (),
            }
        }
    })
}
