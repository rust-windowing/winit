use std::error::Error;

use softbuffer::{Context, Surface};
use tracing::{error, info};
use winit::application::ApplicationHandler;
use winit::data_transfer::{DataTransferSendBuilder, TypeHint};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, AsyncRequestSerial, EventLoop, OwnedDisplayHandle};
use winit::keyboard::Key;
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

fn main() -> Result<(), Box<dyn Error>> {
    tracing::init();

    let event_loop = EventLoop::new()?;

    let app = Application::new();
    Ok(event_loop.run_app(app)?)
}

#[derive(Debug)]
struct Application {
    surface: Option<Surface<OwnedDisplayHandle, Box<dyn Window>>>,
    last_clipboard_fetch: Option<AsyncRequestSerial>,
}

impl Application {
    fn new() -> Self {
        Self { surface: None, last_clipboard_fetch: None }
    }
}

impl ApplicationHandler for Application {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let window_attributes =
            WindowAttributes::default().with_title("Press C to copy, V to paste");

        let window = event_loop.create_window(window_attributes).unwrap();
        let context = Context::new(event_loop.owned_display_handle()).unwrap();
        let surface = Surface::new(&context, window).unwrap();
        self.surface = Some(surface);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                match event.logical_key {
                    Key::Character(ref c) if c == "c" => {
                        let send_data = DataTransferSendBuilder::new("Winit example".to_string())
                            .with_type(TypeHint::Plaintext, |text: &String, _| Some(text.clone()))
                            .with_type(TypeHint::Html, |text, _| {
                                Some(format!("<strong>{text}</strong>"))
                            })
                            .build();

                        match event_loop.set_clipboard(send_data) {
                            Ok(()) => info!("Copied"),
                            Err(err) => error!("Failed to set the clipboard: {err}"),
                        }
                    },
                    Key::Character(ref c) if c == "v" => {
                        let result = event_loop.clipboard().and_then(|id| {
                            let Some(id) = id else {
                                info!("Clipboard is empty");
                                return Ok(None);
                            };

                            let data_transfer = event_loop.data_transfer(id)?;

                            info!("Types: {:#?}", data_transfer.available_types());

                            event_loop.fetch_data_transfer(id, &TypeHint::Plaintext).map(Some)
                        });

                        match result {
                            Ok(serial) => self.last_clipboard_fetch = serial,
                            Err(err) => error!("Failed to read the clipboard: {err}"),
                        }
                    },
                    _ => {},
                }
            },
            WindowEvent::DataTransferReceived { ref value, serial, .. } => {
                assert_eq!(self.last_clipboard_fetch, Some(serial));

                match value.type_().hint() {
                    Some(TypeHint::Plaintext) => {
                        let Ok(text) = value.try_as_string() else {
                            return;
                        };
                        info!("Pasted {text:?}");
                    },
                    _ => {
                        unreachable!("Received a type we didn't ask for!");
                    },
                }
            },
            WindowEvent::RedrawRequested => {
                let surface = self.surface.as_mut().unwrap();
                surface.window().pre_present_notify();
                fill::fill(surface);
            },
            WindowEvent::CloseRequested => {
                event_loop.exit();
            },
            _ => {},
        }
    }
}
