#![allow(clippy::single_match, clippy::disallowed_methods)]

#[cfg(not(wasm_platform))]
use simple_logger::SimpleLogger;
use winit::{
    cursor::CustomCursor,
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

fn decode_cursor(bytes: &[u8]) -> CustomCursor {
    let img = image::load_from_memory(bytes).unwrap().to_rgba8();
    let samples = img.into_flat_samples();
    let (_, w, h) = samples.extents();
    let (w, h) = (w as u32, h as u32);
    CustomCursor::from_rgba(samples.samples, w, h, w / 2, h / 2).unwrap()
}

#[cfg(not(wasm_platform))]
#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    #[cfg(not(wasm_platform))]
    SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .init()
        .unwrap();
    #[cfg(wasm_platform)]
    console_log::init_with_level(log::Level::Debug).unwrap();

    let event_loop = EventLoop::new().unwrap();
    let builder = WindowBuilder::new().with_title("A fantastic window!");
    #[cfg(wasm_platform)]
    let builder = {
        use winit::platform::web::WindowBuilderExtWebSys;
        builder.with_append(true)
    };
    let window = builder.build(&event_loop).unwrap();

    let mut cursor_idx = 0;
    let mut cursor_visible = true;

    let custom_cursors = [
        decode_cursor(include_bytes!("data/cross.png")),
        decode_cursor(include_bytes!("data/cross2.png")),
    ];

    event_loop.run(move |event, _elwt| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(code),
                        ..
                    },
                ..
            } => match code {
                KeyCode::KeyA => {
                    log::debug!("Setting cursor to {:?}", cursor_idx);
                    window.set_custom_cursor(&custom_cursors[cursor_idx]);
                    cursor_idx = (cursor_idx + 1) % 2;
                }
                KeyCode::KeyS => {
                    log::debug!("Setting cursor icon to default");
                    window.set_cursor_icon(Default::default());
                }
                KeyCode::KeyD => {
                    cursor_visible = !cursor_visible;
                    log::debug!("Setting cursor visibility to {:?}", cursor_visible);
                    window.set_cursor_visible(cursor_visible);
                }
                _ => {}
            },
            WindowEvent::RedrawRequested => {
                #[cfg(not(wasm_platform))]
                fill::fill_window(&window);
            }
            WindowEvent::CloseRequested => {
                #[cfg(not(wasm_platform))]
                _elwt.exit();
            }
            _ => (),
        },
        Event::AboutToWait => {
            window.request_redraw();
        }
        _ => {}
    })
}
