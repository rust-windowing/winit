//! Per-key modifier tracking across focus changes.
//!
//! Tracks modifier key state from `KeyboardInput` events (not `ModifiersChanged`).
//!
//! Green = no modifier keys tracked as pressed.
//! Red   = at least one modifier key tracked as pressed.
//!
//! Press C to open a secondary window, then Cmd+W (macOS) or Ctrl+W to close it.
//! Without synthetic key events on focus gain, the primary stays red because the
//! modifier release is lost with the destroyed window.

use std::collections::HashSet;
use std::error::Error;

use tracing::info;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

const GREEN: u32 = 0x00208020;
const RED: u32 = 0x00c02020;
const GREY: u32 = 0x00404040;

#[derive(Default)]
struct App {
    primary: Option<Box<dyn Window>>,
    secondary: Option<Box<dyn Window>>,
    pressed: HashSet<KeyCode>,
}

impl App {
    fn any_mod_held(&self) -> bool {
        self.pressed.iter().any(|k| {
            matches!(
                k,
                KeyCode::MetaLeft
                    | KeyCode::MetaRight
                    | KeyCode::ControlLeft
                    | KeyCode::ControlRight
                    | KeyCode::AltLeft
                    | KeyCode::AltRight
                    | KeyCode::ShiftLeft
                    | KeyCode::ShiftRight
            )
        })
    }

    fn redraw_primary(&self) {
        if let Some(win) = &self.primary {
            let color = if self.any_mod_held() { RED } else { GREEN };
            fill::fill_window_with_color(win.as_ref(), color);
        }
    }

    fn close_secondary(&mut self) {
        if let Some(win) = self.secondary.take() {
            fill::cleanup_window(win.as_ref());
        }
    }

    fn command_held(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.pressed.contains(&KeyCode::MetaLeft) || self.pressed.contains(&KeyCode::MetaRight)
        } else {
            self.pressed.contains(&KeyCode::ControlLeft)
                || self.pressed.contains(&KeyCode::ControlRight)
        }
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let attrs = WindowAttributes::default()
            .with_title("Per-key tracker: green=clear, red=stuck modifier");
        self.primary = Some(event_loop.create_window(attrs).expect("create primary window"));
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let is_primary = self.primary.as_ref().is_some_and(|w| w.id() == window_id);
        let is_secondary = self.secondary.as_ref().is_some_and(|w| w.id() == window_id);

        match event {
            WindowEvent::CloseRequested if is_secondary => self.close_secondary(),
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, is_synthetic, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    let syn = if is_synthetic { " [synthetic]" } else { "" };
                    info!("{code:?} {:?}{syn}", event.state);

                    match event.state {
                        ElementState::Pressed => {
                            self.pressed.insert(code);
                        },
                        ElementState::Released => {
                            self.pressed.remove(&code);
                        },
                    }

                    if code == KeyCode::KeyC
                        && event.state == ElementState::Pressed
                        && self.secondary.is_none()
                    {
                        let attrs = WindowAttributes::default()
                            .with_title("Secondary — close with modifier+W")
                            .with_surface_size(winit::dpi::LogicalSize::new(400, 300));
                        self.secondary =
                            Some(event_loop.create_window(attrs).expect("create secondary window"));
                    }

                    if code == KeyCode::KeyW
                        && event.state == ElementState::Pressed
                        && self.command_held()
                        && is_secondary
                    {
                        self.close_secondary();
                    }
                }
                self.redraw_primary();
            },
            WindowEvent::Focused(focused) => {
                info!("focused={focused} pressed={:?} (window {window_id:?})", self.pressed);
                if is_primary {
                    self.redraw_primary();
                }
            },
            WindowEvent::RedrawRequested if is_primary => self.redraw_primary(),
            WindowEvent::RedrawRequested if is_secondary => {
                if let Some(win) = &self.secondary {
                    fill::fill_window_with_color(win.as_ref(), GREY);
                }
            },
            WindowEvent::SurfaceResized(_) if is_primary => {
                self.primary.as_ref().unwrap().request_redraw()
            },
            WindowEvent::SurfaceResized(_) if is_secondary => {
                self.secondary.as_ref().unwrap().request_redraw()
            },
            _ => {},
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    tracing::init();

    let event_loop = EventLoop::new()?;
    event_loop.run_app(App::default())?;
    Ok(())
}
