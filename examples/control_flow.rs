#![allow(clippy::single_match)]

use std::thread;
#[cfg(not(web_platform))]
use std::time;

use ::tracing::{info, warn};
#[cfg(web_platform)]
use web_time as time;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

const WAIT_TIME: time::Duration = time::Duration::from_millis(100);
const POLL_SLEEP_TIME: time::Duration = time::Duration::from_millis(100);

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    #[default]
    Wait,
    WaitUntil,
    Poll,
}

fn main() -> Result<(), impl std::error::Error> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    info!("Press '1' to switch to Wait mode.");
    info!("Press '2' to switch to WaitUntil mode.");
    info!("Press '3' to switch to Poll mode.");
    info!("Press 'R' to toggle request_redraw() calls.");
    info!("Press 'Esc' to close the window.");

    let event_loop = EventLoop::new().unwrap();

    event_loop.run(ControlFlowDemo::new)
}

struct ControlFlowDemo {
    mode: Mode,
    request_redraw: bool,
    wait_cancelled: bool,
    close_requested: bool,
    window: Box<dyn Window>,
}

impl ControlFlowDemo {
    fn new(event_loop: &dyn ActiveEventLoop) -> Self {
        let window_attributes = WindowAttributes::default().with_title(
            "Press 1, 2, 3 to change control flow mode. Press R to toggle redraw requests.",
        );
        let window = event_loop.create_window(window_attributes).unwrap();
        Self {
            mode: Mode::default(),
            request_redraw: false,
            wait_cancelled: false,
            close_requested: false,
            window,
        }
    }
}

impl ApplicationHandler for ControlFlowDemo {
    fn new_events(&mut self, _event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        info!("new_events: {cause:?}");

        self.wait_cancelled = match cause {
            StartCause::WaitCancelled { .. } => self.mode == Mode::WaitUntil,
            _ => false,
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        info!("{event:?}");

        match event {
            WindowEvent::CloseRequested => {
                self.close_requested = true;
            },
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key: key, state: ElementState::Pressed, .. },
                ..
            } => match key.as_ref() {
                // WARNING: Consider using `key_without_modifiers()` if available on your platform.
                // See the `key_binding` example
                Key::Character("1") => {
                    self.mode = Mode::Wait;
                    warn!("mode: {:?}", self.mode);
                },
                Key::Character("2") => {
                    self.mode = Mode::WaitUntil;
                    warn!("mode: {:?}", self.mode);
                },
                Key::Character("3") => {
                    self.mode = Mode::Poll;
                    warn!("mode: {:?}", self.mode);
                },
                Key::Character("r") => {
                    self.request_redraw = !self.request_redraw;
                    warn!("request_redraw: {}", self.request_redraw);
                },
                Key::Named(NamedKey::Escape) => {
                    self.close_requested = true;
                },
                _ => (),
            },
            WindowEvent::RedrawRequested => {
                self.window.pre_present_notify();
                fill::fill_window(&*self.window);
            },
            _ => (),
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.request_redraw && !self.wait_cancelled && !self.close_requested {
            self.window.request_redraw();
        }

        match self.mode {
            Mode::Wait => event_loop.set_control_flow(ControlFlow::Wait),
            Mode::WaitUntil => {
                if !self.wait_cancelled {
                    event_loop
                        .set_control_flow(ControlFlow::WaitUntil(time::Instant::now() + WAIT_TIME));
                }
            },
            Mode::Poll => {
                thread::sleep(POLL_SLEEP_TIME);
                event_loop.set_control_flow(ControlFlow::Poll);
            },
        };

        if self.close_requested {
            event_loop.exit();
        }
    }
}
