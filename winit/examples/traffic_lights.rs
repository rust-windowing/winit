//! macOS traffic-light inset demo.

use std::error::Error;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
#[cfg(macos_platform)]
use winit::platform::macos::{WindowAttributesMacOS, WindowExtMacOS};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesWeb;
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

const DEFAULT_INSET_X: f64 = 0.0;
const DEFAULT_INSET_Y: f64 = 0.0;
const STEP_FINE: f64 = 1.0;
const STEP_COARSE: f64 = 8.0;

#[derive(Debug)]
struct App {
    window: Option<Box<dyn Window>>,
    inset: LogicalSize<f64>,
    modifiers: ModifiersState,
}

impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            inset: LogicalSize::new(DEFAULT_INSET_X, DEFAULT_INSET_Y),
            modifiers: ModifiersState::default(),
        }
    }
}

impl App {
    fn title(&self) -> String {
        format!(
            "Traffic lights inset: x={:.1}, y={:.1}",
            self.inset.width, self.inset.height
        )
    }

    fn apply_inset(&self) {
        let Some(window) = self.window.as_ref() else {
            return;
        };

        #[cfg(macos_platform)]
        window.set_traffic_light_inset(self.inset);
        window.set_title(&self.title());
    }

    fn set_inset(&mut self, inset: LogicalSize<f64>) {
        self.inset = inset;
        self.apply_inset();
    }

    fn nudge_inset(&mut self, dx: f64, dy: f64) {
        self.inset.width += dx;
        self.inset.height += dy;
        self.apply_inset();
    }
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        let mut window_attributes = WindowAttributes::default().with_title(self.title());

        #[cfg(web_platform)]
        {
            window_attributes = window_attributes
                .with_platform_attributes(Box::new(WindowAttributesWeb::default().with_append(true)));
        }

        #[cfg(macos_platform)]
        {
            let macos_attributes =
                WindowAttributesMacOS::default().with_traffic_light_inset(self.inset);
            window_attributes =
                window_attributes.with_platform_attributes(Box::new(macos_attributes));
        }

        self.window = match event_loop.create_window(window_attributes) {
            Ok(window) => Some(window),
            Err(err) => {
                eprintln!("error creating window: {err}");
                event_loop.exit();
                return;
            },
        };

        self.apply_inset();
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            },
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        key_without_modifiers: key,
                        ..
                    },
                is_synthetic: false,
                ..
            } => {
                let step = if self.modifiers.shift_key() {
                    STEP_COARSE
                } else {
                    STEP_FINE
                };

                match key.as_ref() {
                    Key::Named(NamedKey::ArrowLeft) => self.nudge_inset(-step, 0.0),
                    Key::Named(NamedKey::ArrowRight) => self.nudge_inset(step, 0.0),
                    Key::Named(NamedKey::ArrowUp) => self.nudge_inset(0.0, -step),
                    Key::Named(NamedKey::ArrowDown) => self.nudge_inset(0.0, step),
                    Key::Character("r") => self.set_inset(LogicalSize::new(
                        DEFAULT_INSET_X,
                        DEFAULT_INSET_Y,
                    )),
                    Key::Named(NamedKey::Escape) => event_loop.exit(),
                    _ => (),
                }
            },
            WindowEvent::SurfaceResized(_) => {
                self.window
                    .as_ref()
                    .expect("resize event without a window")
                    .request_redraw();
            },
            WindowEvent::RedrawRequested => {
                let window = self.window.as_ref().expect("redraw request without a window");
                window.pre_present_notify();
                fill::fill_window(window.as_ref());
            },
            _ => (),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    println!("Traffic-light inset demo (macOS only).");
    println!("Arrow keys adjust X/Y. Shift = coarse step.");
    println!("R resets. Esc closes.");

    let event_loop = EventLoop::new()?;
    event_loop.run_app(App::default())?;
    Ok(())
}
