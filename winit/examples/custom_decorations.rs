//! Demonstrates how to create a titlebar-less window and implement basic custom decorations.
//!
//! The goal is to show the building blocks for "draw your own titlebar" across platforms:
//! - Create a window with `decorations(false)`.
//! - Implement click-to-drag (move) via `Window::drag_window()`.
//! - Implement edge/corner resize via `Window::drag_resize_window()`.
//! - Show the system window menu via `Window::show_window_menu()` where supported.
//!
//! This intentionally avoids any UI toolkits: the window is rendered as a solid background with a
//! darker top bar using `softbuffer`.

use std::error::Error;

use ::tracing::{info, warn};
use winit::application::ApplicationHandler;
use winit::event::{ButtonSource, ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{ResizeDirection, Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

const TITLEBAR_HEIGHT_LOGICAL: f64 = 36.0;
const RESIZE_BORDER_LOGICAL: f64 = 8.0;

#[derive(Debug, Default)]
struct App {
    window: Option<Box<dyn Window>>,
    decorations: bool,
}

impl App {
    fn window(&self) -> &dyn Window {
        self.window.as_ref().expect("window should be created").as_ref()
    }

    fn titlebar_height_px(&self) -> f64 {
        TITLEBAR_HEIGHT_LOGICAL * self.window().scale_factor()
    }

    fn resize_border_px(&self) -> f64 {
        RESIZE_BORDER_LOGICAL * self.window().scale_factor()
    }

    fn hit_test_resize(
        &self,
        position: winit::dpi::PhysicalPosition<f64>,
    ) -> Option<ResizeDirection> {
        let size = self.window().surface_size();
        let width = size.width as f64;
        let height = size.height as f64;

        if width <= 0.0 || height <= 0.0 {
            return None;
        }

        let border = self.resize_border_px().max(1.0);
        let x = position.x;
        let y = position.y;

        let left = x >= 0.0 && x < border;
        let right = x <= width && x > width - border;
        let top = y >= 0.0 && y < border;
        let bottom = y <= height && y > height - border;

        match (left, right, top, bottom) {
            (true, _, true, _) => Some(ResizeDirection::NorthWest),
            (_, true, true, _) => Some(ResizeDirection::NorthEast),
            (true, _, _, true) => Some(ResizeDirection::SouthWest),
            (_, true, _, true) => Some(ResizeDirection::SouthEast),
            (true, _, _, _) => Some(ResizeDirection::West),
            (_, true, _, _) => Some(ResizeDirection::East),
            (_, _, true, _) => Some(ResizeDirection::North),
            (_, _, _, true) => Some(ResizeDirection::South),
            _ => None,
        }
    }

    fn is_in_titlebar(&self, position: winit::dpi::PhysicalPosition<f64>) -> bool {
        let y = position.y;
        y >= 0.0 && y < self.titlebar_height_px()
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        info!("Key bindings:");
        info!("  d: toggle decorations (useful for comparison)");
        info!("  esc: exit");
        info!("Mouse:");
        info!("  left drag on top bar: move window");
        info!("  left drag near edges: resize window (if supported)");
        info!("  right click: show window menu (if supported)");

        self.decorations = false;

        let window_attributes = WindowAttributes::default()
            .with_title("Custom decorations (titlebar-less)")
            .with_decorations(self.decorations);

        self.window = match event_loop.create_window(window_attributes) {
            Ok(window) => Some(window),
            Err(err) => {
                eprintln!("error creating window: {err}");
                event_loop.exit();
                return;
            },
        };

        self.window().request_redraw();
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                fill::cleanup_window(self.window());
                event_loop.exit();
            },
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                match event.logical_key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        fill::cleanup_window(self.window());
                        event_loop.exit();
                    },
                    Key::Character("d") => {
                        self.decorations = !self.decorations;
                        info!("decorations: {}", self.decorations);
                        self.window().set_decorations(self.decorations);
                        self.window().request_redraw();
                    },
                    _ => (),
                }
            },
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                button: ButtonSource::Mouse(MouseButton::Left),
                position,
                ..
            } => {
                if let Some(direction) = self.hit_test_resize(position) {
                    if let Err(err) = self.window().drag_resize_window(direction) {
                        warn!("drag_resize_window({direction:?}) failed: {err:?}");
                    }
                } else if self.is_in_titlebar(position) {
                    if let Err(err) = self.window().drag_window() {
                        warn!("drag_window failed: {err:?}");
                    }
                }
            },
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                button: ButtonSource::Mouse(MouseButton::Right),
                position,
                ..
            } => {
                self.window().show_window_menu(position.into());
            },
            WindowEvent::SurfaceResized(_) => {
                self.window().request_redraw();
            },
            WindowEvent::RedrawRequested => {
                let window = self.window();
                window.pre_present_notify();

                let top_bar_height = self.titlebar_height_px().ceil().max(1.0) as u32;
                fill::fill_window_with_top_bar(window, 0xff1c1c1c, 0xff2b2b2b, top_bar_height);
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    let event_loop = EventLoop::new()?;
    event_loop.run_app(App::default())?;

    Ok(())
}

