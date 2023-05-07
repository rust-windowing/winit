#![allow(clippy::single_match)]

use std::{thread, time};

use simple_logger::SimpleLogger;
use winit::{
    event::{Event, KeyboardInput, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Wait,
    WaitUntil,
    Poll,
}

const WAIT_TIME: time::Duration = time::Duration::from_millis(100);
const POLL_SLEEP_TIME: time::Duration = time::Duration::from_millis(100);

fn main() {
    SimpleLogger::new().init().unwrap();

    println!("Press '1' to switch to Wait mode.");
    println!("Press '2' to switch to WaitUntil mode.");
    println!("Press '3' to switch to Poll mode.");
    println!("Press 'R' to toggle request_redraw() calls.");
    println!("Press 'Esc' to close the window.");

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Press 1, 2, 3 to change control flow mode. Press R to toggle redraw requests.")
        .build(&event_loop)
        .unwrap();

    let mut mode = Mode::Wait;
    let mut request_redraw = false;
    let mut wait_cancelled = false;
    let mut close_requested = false;

    event_loop.run(move |event, _, control_flow| {
        use winit::event::{ElementState, StartCause, VirtualKeyCode};
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
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state: ElementState::Pressed,
                            ..
                        },
                    ..
                } => match virtual_code {
                    VirtualKeyCode::Key1 => {
                        mode = Mode::Wait;
                        println!("\nmode: {mode:?}\n");
                    }
                    VirtualKeyCode::Key2 => {
                        mode = Mode::WaitUntil;
                        println!("\nmode: {mode:?}\n");
                    }
                    VirtualKeyCode::Key3 => {
                        mode = Mode::Poll;
                        println!("\nmode: {mode:?}\n");
                    }
                    VirtualKeyCode::R => {
                        request_redraw = !request_redraw;
                        println!("\nrequest_redraw: {request_redraw}\n");
                    }
                    VirtualKeyCode::Escape => {
                        close_requested = true;
                    }
                    _ => (),
                },
                _ => (),
            },
            Event::MainEventsCleared => {
                if request_redraw && !wait_cancelled && !close_requested {
                    window.request_redraw();
                }
                if close_requested {
                    control_flow.set_exit();
                }
            }
            Event::RedrawRequested(_window_id) => {}
            Event::RedrawEventsCleared => {
                match mode {
                    Mode::Wait => control_flow.set_wait(),
                    Mode::WaitUntil => {
                        if !wait_cancelled {
                            control_flow.set_wait_until(instant::Instant::now() + WAIT_TIME);
                        }
                    }
                    Mode::Poll => {
                        thread::sleep(POLL_SLEEP_TIME);
                        control_flow.set_poll();
                    }
                };
            }
            _ => (),
        }
    });
}
