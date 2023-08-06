//! Demonstrates the use of startup notifications on Linux.

#[cfg(any(x11_platform, wayland_platform))]
#[path = "./util/fill.rs"]
mod fill;

#[cfg(any(x11_platform, wayland_platform))]
mod example {
    use std::collections::HashMap;
    use std::rc::Rc;

    use winit::event::{ElementState, Event, KeyEvent, WindowEvent};
    use winit::event_loop::EventLoop;
    use winit::keyboard::Key;
    use winit::platform::startup_notify::{
        EventLoopExtStartupNotify, WindowBuilderExtStartupNotify, WindowExtStartupNotify,
    };
    use winit::window::{Window, WindowBuilder, WindowId};

    pub(super) fn main() -> Result<(), impl std::error::Error> {
        // Create the event loop and get the activation token.
        let event_loop = EventLoop::new();
        let mut current_token = match event_loop.read_token_from_env() {
            Some(token) => Some(token),
            None => {
                println!("No startup notification token found in environment.");
                None
            }
        };

        let mut windows: HashMap<WindowId, Rc<Window>> = HashMap::new();
        let mut counter = 0;
        let mut create_first_window = false;

        event_loop.run(move |event, elwt, flow| {
            match event {
                Event::Resumed => create_first_window = true,

                Event::WindowEvent {
                    window_id,
                    event:
                        WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    logical_key,
                                    state: ElementState::Pressed,
                                    ..
                                },
                            ..
                        },
                } => {
                    if logical_key == Key::Character("n".into()) {
                        if let Some(window) = windows.get(&window_id) {
                            // Request a new activation token on this window.
                            // Once we get it we will use it to create a window.
                            window
                                .request_activation_token()
                                .expect("Failed to request activation token.");
                        }
                    }
                }

                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested,
                } => {
                    // Remove the window from the map.
                    windows.remove(&window_id);
                    if windows.is_empty() {
                        flow.set_exit();
                        return;
                    }
                }

                Event::WindowEvent {
                    event: WindowEvent::ActivationTokenDone { token, .. },
                    ..
                } => {
                    current_token = Some(token);
                }

                Event::RedrawRequested(id) => {
                    if let Some(window) = windows.get(&id) {
                        super::fill::fill_window(window);
                    }
                }

                _ => {}
            }

            // See if we've passed the deadline.
            if current_token.is_some() || create_first_window {
                // Create the initial window.
                let window = {
                    let mut builder =
                        WindowBuilder::new().with_title(format!("Window {}", counter));

                    if let Some(token) = current_token.take() {
                        println!("Creating a window with token {token:?}");
                        builder = builder.with_activation_token(token);
                    }

                    Rc::new(builder.build(elwt).unwrap())
                };

                // Add the window to the map.
                windows.insert(window.id(), window.clone());

                counter += 1;
                create_first_window = false;
            }

            flow.set_wait();
        })
    }
}

#[cfg(any(x11_platform, wayland_platform))]
fn main() -> Result<(), impl std::error::Error> {
    example::main()
}

#[cfg(not(any(x11_platform, wayland_platform)))]
fn main() {
    println!("This example is only supported on X11 and Wayland platforms.");
}
