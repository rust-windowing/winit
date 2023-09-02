#![allow(clippy::single_match)]

use std::thread;
#[cfg(not(wasm_platform))]
use std::time;
#[cfg(wasm_platform)]
use web_time as time;

use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::Key,
    window::{Window, WindowBuilder},
    ApplicationHandler,
};

#[path = "util/fill.rs"]
mod fill;

const WAIT_TIME: time::Duration = time::Duration::from_millis(100);
const POLL_SLEEP_TIME: time::Duration = time::Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Wait,
    WaitUntil,
    Poll,
}

#[derive(Debug)]
struct App {
    mode: Mode,
    request_redraw: bool,
    wait_cancelled: bool,
    close_requested: bool,
    window: Window,
}

impl ApplicationHandler for App {
    fn window_event(
        &mut self,
        _elwt: &winit::event_loop::EventLoopWindowTarget,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        println!("{event:?}");
        match event {
            WindowEvent::CloseRequested => {
                self.close_requested = true;
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
                    self.mode = Mode::Wait;
                    println!("\nmode: {:?}\n", self.mode);
                }
                Key::Character("2") => {
                    self.mode = Mode::WaitUntil;
                    println!("\nmode: {:?}\n", self.mode);
                }
                Key::Character("3") => {
                    self.mode = Mode::Poll;
                    println!("\nmode: {:?}\n", self.mode);
                }
                Key::Character("r") => {
                    self.request_redraw = !self.request_redraw;
                    println!("\nrequest_redraw: {}\n", self.request_redraw);
                }
                Key::Escape => {
                    self.close_requested = true;
                }
                _ => (),
            },
            WindowEvent::RedrawRequested => {
                fill::fill_window(&self.window);
            }
            _ => (),
        }
    }

    fn start_wait_cancelled(
        &mut self,
        _elwt: &winit::event_loop::EventLoopWindowTarget<()>,
        _start: time::Instant,
        _requested_resume: Option<time::Instant>,
    ) {
        self.wait_cancelled = self.mode == Mode::WaitUntil;
    }

    fn start_resume_time_reached(
        &mut self,
        _elwt: &winit::event_loop::EventLoopWindowTarget<()>,
        _start: time::Instant,
        _requested_resume: time::Instant,
    ) {
        self.wait_cancelled = false;
    }

    fn start_poll(&mut self, _elwt: &winit::event_loop::EventLoopWindowTarget<()>) {
        self.wait_cancelled = false;
    }

    fn about_to_wait(&mut self, elwt: &winit::event_loop::EventLoopWindowTarget) {
        if self.request_redraw && !self.wait_cancelled && !self.close_requested {
            self.window.request_redraw();
        }

        match self.mode {
            Mode::Wait => elwt.set_wait(),
            Mode::WaitUntil => {
                if !self.wait_cancelled {
                    elwt.set_wait_until(time::Instant::now() + WAIT_TIME);
                }
            }
            Mode::Poll => {
                thread::sleep(POLL_SLEEP_TIME);
                elwt.set_poll();
            }
        };

        if self.close_requested {
            elwt.exit();
        }
    }
}

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();

    println!("Press '1' to switch to Wait mode.");
    println!("Press '2' to switch to WaitUntil mode.");
    println!("Press '3' to switch to Poll mode.");
    println!("Press 'R' to toggle request_redraw() calls.");
    println!("Press 'Esc' to close the window.");

    let event_loop = EventLoop::new().unwrap();
    event_loop.run_with(|elwt| {
        let window = WindowBuilder::new()
            .with_title(
                "Press 1, 2, 3 to change control flow mode. Press R to toggle redraw requests.",
            )
            .build(elwt)
            .unwrap();

        App {
            window,
            mode: Mode::Wait,
            request_redraw: false,
            wait_cancelled: false,
            close_requested: false,
        }
    })
}
