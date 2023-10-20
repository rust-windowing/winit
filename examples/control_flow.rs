#![allow(clippy::single_match)]

use std::thread;
#[cfg(not(wasm_platform))]
use std::time;
#[cfg(wasm_platform)]
use web_time as time;

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    window::WindowBuilder,
};

#[path = "util/fill.rs"]
mod fill;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Wait,
    WaitUntil,
    Poll,
}

const WAIT_TIME: time::Duration = time::Duration::from_millis(100);
const POLL_SLEEP_TIME: time::Duration = time::Duration::from_millis(100);

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();

    println!("Press '1' to switch to Wait mode.");
    println!("Press '2' to switch to WaitUntil mode.");
    println!("Press '3' to switch to Poll mode.");
    println!("Press 'R' to toggle request_redraw() calls.");
    println!("Press 'Esc' to close the window.");

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Press 1, 2, 3 to change control flow mode. Press R to toggle redraw requests.")
        .build(&event_loop)
        .unwrap();

    let mut mode = Mode::Wait;
    let mut request_redraw = false;
    let mut wait_cancelled = false;
    let mut close_requested = false;

    event_loop.run(move |event, elwt| {
        use winit::event::StartCause;
        println!("{event:?}");
        match event {
            Event::NewEvents(start_cause) => {
                wait_cancelled = match start_cause {
                    StartCause::WaitCancelled { .. } => mode == Mode::WaitUntil,
                    _ => false,
                }
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    close_requested = true;
                }
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            logical_key: key,
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => match key.as_ref() {
                    // WARNING: Consider using `key_without_modifers()` if available on your platform.
                    // See the `key_binding` example
                    Key::Character("1") => {
                        mode = Mode::Wait;
                        println!("\nmode: {mode:?}\n");
                    }
                    Key::Character("2") => {
                        mode = Mode::WaitUntil;
                        println!("\nmode: {mode:?}\n");
                    }
                    Key::Character("3") => {
                        mode = Mode::Poll;
                        println!("\nmode: {mode:?}\n");
                    }
                    Key::Character("r") => {
                        request_redraw = !request_redraw;
                        println!("\nrequest_redraw: {request_redraw}\n");
                    }
                    Key::Named(NamedKey::Escape) => {
                        close_requested = true;
                    }
                    _ => (),
                },
                WindowEvent::RedrawRequested => {
                    fill::fill_window(&window);
                }
                _ => (),
            },
            Event::AboutToWait => {
                if request_redraw && !wait_cancelled && !close_requested {
                    window.request_redraw();
                }

                match mode {
                    Mode::Wait => elwt.set_control_flow(ControlFlow::Wait),
                    Mode::WaitUntil => {
                        if !wait_cancelled {
                            elwt.set_control_flow(ControlFlow::WaitUntil(
                                time::Instant::now() + WAIT_TIME,
                            ));
                        }
                    }
                    Mode::Poll => {
                        thread::sleep(POLL_SLEEP_TIME);
                        elwt.set_control_flow(ControlFlow::Poll);
                    }
                };

                if close_requested {
                    elwt.exit();
                }
            }
            _ => (),
        }
    })
}
