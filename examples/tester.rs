//! Basic winit interactivity example.

//! Web compilation and testing example:
//!     cargo build --release --target wasm32-unknown-unknown --example tester && \
//!      wasm-bindgen target/wasm32-unknown-unknown/release/examples/tester.wasm \
//!      --out-dir pkg --target web
//!     python3 -m http.server
//! Then navigate to http://localhost:8000/examples/tester.html

use std::error::Error;

use font8x8::legacy::BASIC_LEGACY;
use winit::application::ApplicationHandler;
use winit::event::{ButtonSource, MouseButton, PointerSource, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
#[cfg(web_platform)]
use winit::platform::web::WindowAttributesExtWeb;
use winit::window::{Window, WindowAttributes, WindowId};

fn draw_char(frame: &mut [u32], width: usize, x: u32, y: u32, ch: char, fg: u32, bg: u32) {
    let x: usize = x.try_into().unwrap();
    let y: usize = y.try_into().unwrap();
    let glyph = BASIC_LEGACY.get(ch as usize).unwrap_or(&BASIC_LEGACY[' ' as usize]);
    for (row, byte) in glyph.iter().enumerate() {
        let ypart = (y + row) * width;
        for col in 0..8 {
            let i = ypart + (x + col);
            if i < frame.len() && byte & (1 << col) != 0 {
                frame[i] = fg;
            } else {
                frame[i] = bg;
            }
        }
    }
}

fn draw_text(
    frame: &mut [u32],
    width: usize,
    mut x: u32,
    mut y: u32,
    text: &str,
    fg: u32,
    bg: u32,
) -> (u32, u32) {
    let x_init = x;
    let mut max_x = x;
    let mut max_y = y;
    for ch in text.chars() {
        if ch == '\n' {
            x = x_init;
            y += 8;
            max_y = max_y.max(y);
        } else {
            draw_char(frame, width, x, y, ch, fg, bg);
            x += 8;
            max_x = max_x.max(x);
        }
    }
    (max_x + 8, max_y + 8)
}

#[path = "util/fill.rs"]
mod fill;
#[path = "util/tracing.rs"]
mod tracing;

#[derive(Default, Debug)]
struct App {
    window: Option<Box<dyn Window>>,
    old_posx: f32,
    old_posy: f32,
    posx: f32,
    posy: f32,
    has_pressure: Option<f32>,
    drawing: bool,
    last_draw: Vec<[u32; 4]>,
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[cfg(not(web_platform))]
        let window_attributes = WindowAttributes::default();
        #[cfg(web_platform)]
        let window_attributes = WindowAttributes::default().with_append(true);
        self.window = match event_loop.create_window(window_attributes) {
            Ok(window) => Some(window),
            Err(err) => {
                eprintln!("error creating window: {err}");
                event_loop.exit();
                return;
            },
        };

        let window = self.window.as_ref().unwrap();
        window.pre_present_notify();
        fill::fill_window_with_color(&**window, 0xff181818);
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                println!("Close was requested; stopping");
                event_loop.exit();
            },
            WindowEvent::SurfaceResized(_) => {
                let window = self.window.as_ref().expect("resize event without a window");
                window.pre_present_notify();
                fill::fill_window_with_color(&**window, 0xff181818);
                window.request_redraw();
            },
            WindowEvent::PointerButton { primary, position, state, button, .. } => {
                match (primary, button) {
                    (true, ButtonSource::Mouse(MouseButton::Left)) => {
                        self.posx = position.x as f32;
                        self.posy = position.y as f32;
                        self.old_posx = position.x as f32;
                        self.old_posy = position.y as f32;
                        self.drawing = state == winit::event::ElementState::Pressed;
                        self.has_pressure = None;
                    },
                    (true, ButtonSource::Touch { force, .. }) => {
                        self.posx = position.x as f32;
                        self.posy = position.y as f32;
                        self.old_posx = position.x as f32;
                        self.old_posy = position.y as f32;
                        self.drawing = state == winit::event::ElementState::Pressed;
                        self.has_pressure = force.map(|x| x.normalized() as f32);
                    },
                    _ => {
                        let window = self.window.as_ref().unwrap();
                        window.pre_present_notify();
                        fill::fill_window_with_color(&**window, 0xff181818);
                        window.request_redraw();
                    },
                }
            },
            WindowEvent::PointerMoved { position, source, .. } => {
                if self.drawing {
                    self.posx = position.x as f32;
                    self.posy = position.y as f32;
                }
                if let PointerSource::Touch { force, .. } = source {
                    if self.has_pressure.is_some() {
                        self.has_pressure = force.map(|x| x.normalized() as f32);
                    }
                }
            },
            WindowEvent::RedrawRequested => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in AboutToWait, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.

                let window = self.window.as_ref().expect("redraw request without a window");

                // Notify that you're about to draw.
                window.pre_present_notify();

                let mut rects = vec![];
                let rects_ref = &mut rects;
                // Draw.
                fill::fill_window_with_fn(&**window, |frame, stride, scale, frame_w, frame_h| {
                    // Clear the top left 50x50 rect, we'll put an animation there.
                    for y in 0..50.min(frame_h as usize) {
                        for x in 0..50.min(frame_w as usize) {
                            frame[y * stride + x] = 0xff181818;
                        }
                    }

                    let extent = draw_text(
                        frame,
                        stride,
                        20,
                        50,
                        &format!(
                            "Input tester.\nLeft click to draw, right click to clear.\nx: {}\ny: \
                             {}\npressure: {:?}",
                            self.posx, self.posy, self.has_pressure
                        ),
                        0xffffffff,
                        0xff181818,
                    );
                    let rect1 = [20, 50, extent.0 - 20, extent.1 - 50];

                    let mut draw_line =
                        |xpos: f32, ypos: f32, xoff: f32, yoff: f32, thickness: u32| {
                            if xoff == 0.0 && yoff == 0.0 {
                                return;
                            }
                            let len = (xoff * xoff + yoff * yoff).sqrt();
                            let norm = 1.0 / xoff.abs().max(yoff.abs());
                            let xo_small = xoff * norm;
                            let yo_small = yoff * norm;
                            let spacing = (thickness as f32 * 0.25).max(1.0);
                            let antinorm =
                                1.0 / (xo_small * xo_small + yo_small * yo_small).sqrt() / spacing;
                            for i in 0..=(len * antinorm) as usize {
                                let i = i as f32;

                                for _y in -(thickness as i32) / 2..(thickness as i32 + 1) / 2 {
                                    for _x in -(thickness as i32) / 2..(thickness as i32 + 1) / 2 {
                                        let x = (xpos + _x as f32 + xo_small * i * spacing)
                                            / scale as f32;
                                        let y = (ypos + _y as f32 + yo_small * i * spacing)
                                            / scale as f32;

                                        let xpart = x.clamp(0.0, frame_w as f32 - 1.0) as usize;
                                        let ypart =
                                            y.clamp(0.0, frame_h as f32 - 1.0) as usize * stride;
                                        frame[ypart + xpart] = 0xffffffff;
                                    }
                                }
                            }
                        };

                    // Animation.
                    let x = 25.0;
                    let y = 25.0;

                    static START: std::sync::OnceLock<web_time::Instant> =
                        std::sync::OnceLock::new();
                    let start = START.get_or_init(web_time::Instant::now);
                    let time = web_time::Instant::now().duration_since(*start).as_secs_f32();

                    let xo = (time).sin() * 25.0;
                    let yo = (time).cos() * 25.0;
                    draw_line(x, y, xo, yo, 1);
                    draw_line(x, y, -xo, -yo, 1);

                    // Any new lines to draw from input?
                    let diameter = self.has_pressure.map(|x| x * 10.0).unwrap_or(0.0).ceil();
                    if self.drawing {
                        draw_line(
                            self.old_posx,
                            self.old_posy,
                            self.posx - self.old_posx,
                            self.posy - self.old_posy,
                            diameter as u32 + 1,
                        );
                    }
                    let radius = (diameter * 0.5).ceil();

                    let x = (self.posx.min(self.old_posx) - radius).max(0.0);
                    let y = (self.posy.min(self.old_posy) - radius).max(0.0);
                    let w =
                        (self.posx.max(self.old_posx) - x + 1.0 + radius).min(frame_w as f32 - x);
                    let h =
                        (self.posy.max(self.old_posy) - y + 1.0 + radius).min(frame_h as f32 - y);

                    *rects_ref =
                        vec![[0, 0, 50, 50], rect1, [x as u32, y as u32, w as u32, h as u32]];
                    let mut damaged = rects_ref.clone();
                    damaged.extend_from_slice(&self.last_draw);
                    damaged
                });

                self.old_posx = self.posx;
                self.old_posy = self.posy;

                self.last_draw = rects;

                // For contiguous redraw loop you can request a redraw from here.
                window.request_redraw();
                // Don't run hundreds of thousands of times per second even if the platform asks.
                // (Can't sleep on web, so gated off there. Browsers won't ask for too much.)
                #[cfg(not(web_platform))]
                {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
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

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
    event_loop.run_app(App::default())?;

    Ok(())
}

#[cfg(web_platform)]
use wasm_bindgen::prelude::wasm_bindgen;
#[cfg(web_platform)]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), wasm_bindgen::JsValue> {
    #[cfg(web_platform)]
    console_error_panic_hook::set_once();

    tracing::init();

    let event_loop = EventLoop::new().unwrap();

    // For alternative loop run options see `pump_events` and `run_on_demand` examples.
    event_loop.run_app(App::default()).unwrap();

    Ok(())
}
