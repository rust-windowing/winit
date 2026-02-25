//! Demonstrates basic titlebar customization.
//!
//! This example intentionally keeps rendering simple (a solid fill) and focuses on:
//! - Selecting titlebar-related window attributes at creation time.
//! - Toggling a small set of runtime titlebar properties (where supported).

use std::error::Error;

use ::tracing::{info, warn};
use winit::application::ApplicationHandler;
use winit::event::{ButtonSource, ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
#[cfg(macos_platform)]
use winit::platform::macos::{WindowAttributesMacOS, WindowExtMacOS};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesWeb;
#[cfg(windows_platform)]
use winit::platform::windows::{BackdropType, Color, WindowAttributesWindows, WindowExtWindows};
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

#[derive(Debug, Default)]
struct App {
    window: Option<Box<dyn Window>>,
    decorations: bool,

    #[cfg(macos_platform)]
    macos_unified_titlebar: bool,
    #[cfg(macos_platform)]
    macos_shadow: bool,

    #[cfg(windows_platform)]
    windows_custom_title_colors: bool,
    #[cfg(windows_platform)]
    windows_backdrop: BackdropType,
}

impl App {
    fn window(&self) -> &dyn Window {
        self.window.as_ref().expect("window should be created").as_ref()
    }

    fn request_redraw(&self) {
        self.window().request_redraw();
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.decorations = true;

        #[cfg(macos_platform)]
        {
            self.macos_unified_titlebar = true;
            self.macos_shadow = true;
        }

        #[cfg(windows_platform)]
        {
            self.windows_custom_title_colors = true;
            self.windows_backdrop = BackdropType::MainWindow;
        }

        info!("Key bindings:");
        info!("  d: toggle decorations (all platforms)");
        info!("  right click: show window menu (if supported)");
        #[cfg(macos_platform)]
        {
            info!("  u: toggle unified titlebar (macOS)");
            info!("  s: toggle window shadow (macOS)");
            info!("  h: print titlebar height (macOS)");
        }
        #[cfg(windows_platform)]
        {
            info!("  c: toggle titlebar colors (Windows 11 Build 22000+)");
            info!("  b: cycle backdrop (Windows 11 Build 22523+)");
        }
        info!("  esc: exit");

        let mut window_attributes = WindowAttributes::default()
            .with_title("Titlebar customization (press 'd', 'esc')")
            .with_decorations(self.decorations);

        // Platform-specific titlebar configuration. These are applied at window creation time.
        #[cfg(macos_platform)]
        {
            let platform = WindowAttributesMacOS::default()
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true)
                .with_movable_by_window_background(true)
                .with_unified_titlebar(self.macos_unified_titlebar);
            window_attributes = window_attributes.with_platform_attributes(Box::new(platform));
        }

        #[cfg(windows_platform)]
        {
            // Titlebar color customization requires Windows 11 Build 22000+. The calls below may
            // no-op on older versions.
            let platform = WindowAttributesWindows::default()
                .with_title_background_color(Some(Color::from_rgb(0x20, 0x24, 0x2a)))
                .with_title_text_color(Color::from_rgb(0xf0, 0xf3, 0xf6))
                .with_border_color(Some(Color::from_rgb(0x57, 0x74, 0x8a)))
                .with_system_backdrop(self.windows_backdrop);
            window_attributes = window_attributes.with_platform_attributes(Box::new(platform));
        }

        #[cfg(web_platform)]
        {
            // Make sure the canvas is attached to the DOM.
            let platform = WindowAttributesWeb::default().with_append(true);
            window_attributes = window_attributes.with_platform_attributes(Box::new(platform));
        }

        let window = match event_loop.create_window(window_attributes) {
            Ok(window) => window,
            Err(err) => {
                eprintln!("error creating window: {err}");
                event_loop.exit();
                return;
            },
        };

        // Apply runtime adjustments that are supported after creation.
        #[cfg(macos_platform)]
        {
            window.set_has_shadow(self.macos_shadow);
            window.set_unified_titlebar(self.macos_unified_titlebar);
            info!("macOS titlebar height: {:.1}", window.titlebar_height());
        }
        #[cfg(windows_platform)]
        {
            window.set_system_backdrop(self.windows_backdrop);
        }

        self.window = Some(window);
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                fill::cleanup_window(self.window());
                event_loop.exit();
            },
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key: key, state: ElementState::Pressed, .. },
                ..
            } => {
                match key.as_ref() {
                    Key::Named(NamedKey::Escape) => {
                        fill::cleanup_window(self.window());
                        event_loop.exit();
                    },
                    Key::Character("d") => {
                        self.decorations = !self.decorations;
                        info!("decorations: {}", self.decorations);
                        self.window().set_decorations(self.decorations);
                        self.request_redraw();
                    },

                    Key::Character("u") => {
                        #[cfg(macos_platform)]
                        {
                            self.macos_unified_titlebar = !self.macos_unified_titlebar;
                            info!("macos unified titlebar: {}", self.macos_unified_titlebar);
                            self.window().set_unified_titlebar(self.macos_unified_titlebar);
                        }
                        #[cfg(not(macos_platform))]
                        warn!("'u' only has an effect on macOS");
                    },
                    Key::Character("s") => {
                        #[cfg(macos_platform)]
                        {
                            self.macos_shadow = !self.macos_shadow;
                            info!("macos shadow: {}", self.macos_shadow);
                            self.window().set_has_shadow(self.macos_shadow);
                        }
                        #[cfg(not(macos_platform))]
                        warn!("'s' only has an effect on macOS");
                    },
                    Key::Character("h") => {
                        #[cfg(macos_platform)]
                        info!("macOS titlebar height: {:.1}", self.window().titlebar_height());
                        #[cfg(not(macos_platform))]
                        warn!("'h' only has an effect on macOS");
                    },

                    Key::Character("c") => {
                        #[cfg(windows_platform)]
                        {
                            self.windows_custom_title_colors = !self.windows_custom_title_colors;
                            info!(
                                "windows custom title colors: {}",
                                self.windows_custom_title_colors
                            );

                            if self.windows_custom_title_colors {
                                self.window().set_title_background_color(Some(Color::from_rgb(
                                    0x20, 0x24, 0x2a,
                                )));
                                self.window()
                                    .set_title_text_color(Color::from_rgb(0xf0, 0xf3, 0xf6));
                            } else {
                                self.window().set_title_background_color(None);
                                self.window().set_title_text_color(Color::SYSTEM_DEFAULT);
                            }
                        }
                        #[cfg(not(windows_platform))]
                        warn!("'c' only has an effect on Windows");
                    },
                    Key::Character("b") => {
                        #[cfg(windows_platform)]
                        {
                            self.windows_backdrop = match self.windows_backdrop {
                                BackdropType::Auto => BackdropType::MainWindow,
                                BackdropType::MainWindow => BackdropType::TransientWindow,
                                BackdropType::TransientWindow => BackdropType::TabbedWindow,
                                BackdropType::TabbedWindow => BackdropType::Auto,
                            };

                            info!("windows backdrop: {:?}", self.windows_backdrop);
                            self.window().set_system_backdrop(self.windows_backdrop);
                        }
                        #[cfg(not(windows_platform))]
                        warn!("'b' only has an effect on Windows");
                    },

                    _ => (),
                };
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
                self.request_redraw();
            },
            WindowEvent::RedrawRequested => {
                let window = self.window();
                window.pre_present_notify();

                // Pick a slightly brighter fill so the titlebar overlay is obvious on macOS.
                fill::fill_window_with_color(window, 0xff2a3340);
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
