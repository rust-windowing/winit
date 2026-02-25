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
use cursor_icon::CursorIcon;
use winit::application::ApplicationHandler;
use winit::cursor::Cursor;
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
const CAPTION_BUTTON_SIZE_LOGICAL: f64 = 14.0;
const CAPTION_BUTTON_GAP_LOGICAL: f64 = 8.0;
const CAPTION_BUTTON_PADDING_LOGICAL: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptionButton {
    Minimize,
    Maximize,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HitTarget {
    None,
    Titlebar,
    Resize(ResizeDirection),
    Button(CaptionButton),
}

#[derive(Debug)]
struct App {
    window: Option<Box<dyn Window>>,
    decorations: bool,
    hit_target: HitTarget,
    cursor_icon: CursorIcon,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            decorations: false,
            hit_target: HitTarget::None,
            cursor_icon: CursorIcon::default(),
        }
    }
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
            (true, ..) => Some(ResizeDirection::West),
            (_, true, ..) => Some(ResizeDirection::East),
            (_, _, true, _) => Some(ResizeDirection::North),
            (_, _, _, true) => Some(ResizeDirection::South),
            _ => None,
        }
    }

    fn is_in_titlebar(&self, position: winit::dpi::PhysicalPosition<f64>) -> bool {
        let y = position.y;
        y >= 0.0 && y < self.titlebar_height_px()
    }

    fn caption_button_rects(&self) -> [(CaptionButton, fill::Rect); 3] {
        let size = self.window().surface_size();
        let width = size.width;

        let scale = self.window().scale_factor();
        let button_size = (CAPTION_BUTTON_SIZE_LOGICAL * scale).round().max(1.0) as u32;
        let gap = (CAPTION_BUTTON_GAP_LOGICAL * scale).round().max(0.0) as u32;
        let padding = (CAPTION_BUTTON_PADDING_LOGICAL * scale).round().max(0.0) as u32;

        let top_bar_height = self.titlebar_height_px().round().max(1.0) as u32;
        let y = (top_bar_height.saturating_sub(button_size)) / 2;

        let cluster_width = button_size.saturating_mul(3).saturating_add(gap.saturating_mul(2));
        let x0 = width.saturating_sub(padding.saturating_add(cluster_width));

        let rect = |i: u32| fill::Rect {
            x: x0.saturating_add(i.saturating_mul(button_size.saturating_add(gap))),
            y,
            width: button_size,
            height: button_size,
        };

        [
            (CaptionButton::Minimize, rect(0)),
            (CaptionButton::Maximize, rect(1)),
            (CaptionButton::Close, rect(2)),
        ]
    }

    fn hit_test_caption_buttons(
        &self,
        position: winit::dpi::PhysicalPosition<f64>,
    ) -> Option<CaptionButton> {
        for (button, rect) in self.caption_button_rects() {
            let x0 = rect.x as f64;
            let y0 = rect.y as f64;
            let x1 = (rect.x + rect.width) as f64;
            let y1 = (rect.y + rect.height) as f64;
            if position.x >= x0 && position.x < x1 && position.y >= y0 && position.y < y1 {
                return Some(button);
            }
        }
        None
    }

    fn hit_test(&self, position: winit::dpi::PhysicalPosition<f64>) -> HitTarget {
        if let Some(direction) = self.hit_test_resize(position) {
            return HitTarget::Resize(direction);
        }

        if let Some(button) = self.hit_test_caption_buttons(position) {
            return HitTarget::Button(button);
        }

        if self.is_in_titlebar(position) {
            return HitTarget::Titlebar;
        }

        HitTarget::None
    }

    fn cursor_for_target(target: HitTarget) -> CursorIcon {
        match target {
            HitTarget::None => CursorIcon::Default,
            HitTarget::Titlebar => CursorIcon::Grab,
            HitTarget::Button(_) => CursorIcon::Pointer,
            HitTarget::Resize(direction) => match direction {
                ResizeDirection::East | ResizeDirection::West => CursorIcon::EwResize,
                ResizeDirection::North | ResizeDirection::South => CursorIcon::NsResize,
                ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::NeswResize,
                ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::NwseResize,
            },
        }
    }

    fn set_cursor_icon(&mut self, icon: CursorIcon) {
        if icon == self.cursor_icon {
            return;
        }
        self.cursor_icon = icon;
        self.window().set_cursor(Cursor::from(icon));
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
        self.hit_target = HitTarget::None;
        self.cursor_icon = CursorIcon::Default;

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
            } => match self.hit_test(position) {
                HitTarget::Resize(direction) => {
                    if let Err(err) = self.window().drag_resize_window(direction) {
                        warn!("drag_resize_window({direction:?}) failed: {err:?}");
                    }
                },
                HitTarget::Titlebar => {
                    if let Err(err) = self.window().drag_window() {
                        warn!("drag_window failed: {err:?}");
                    }
                },
                HitTarget::Button(CaptionButton::Close) => {
                    fill::cleanup_window(self.window());
                    event_loop.exit();
                },
                HitTarget::Button(CaptionButton::Minimize) => {
                    self.window().set_minimized(true);
                },
                HitTarget::Button(CaptionButton::Maximize) => {
                    let maximized = self.window().is_maximized();
                    self.window().set_maximized(!maximized);
                },
                HitTarget::None => (),
            },
            WindowEvent::PointerButton {
                state: ElementState::Pressed,
                button: ButtonSource::Mouse(MouseButton::Right),
                position,
                ..
            } => {
                self.window().show_window_menu(position.into());
            },
            WindowEvent::PointerMoved { position, .. } => {
                let target = self.hit_test(position);
                if target != self.hit_target {
                    self.hit_target = target;
                    self.set_cursor_icon(Self::cursor_for_target(target));
                    self.window().request_redraw();
                }
            },
            WindowEvent::SurfaceResized(_) => {
                self.window().request_redraw();
            },
            WindowEvent::RedrawRequested => {
                let window = self.window();
                window.pre_present_notify();

                let top_bar_height = self.titlebar_height_px().ceil().max(1.0) as u32;

                let mut rects = Vec::new();
                for (button, rect) in self.caption_button_rects() {
                    let base: u32 = match button {
                        CaptionButton::Close => 0xffb8383d_u32,
                        CaptionButton::Maximize => 0xff2f9e44_u32,
                        CaptionButton::Minimize => 0xfff08c00_u32,
                    };
                    let color = if self.hit_target == HitTarget::Button(button) {
                        base.saturating_add(0x00101010)
                    } else {
                        base
                    };
                    rects.push((rect, color));
                }

                fill::fill_window_with_top_bar_and_rects(
                    window,
                    0xff1c1c1c,
                    0xff2b2b2b,
                    top_bar_height,
                    &rects,
                );
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
