#![allow(clippy::single_match, clippy::disallowed_methods)]

#[cfg(not(wasm_platform))]
use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{EventLoop, EventLoopWindowTarget},
    keyboard::Key,
    window::{CustomCursor, WindowBuilder},
};

fn decode_cursor<T>(bytes: &[u8], window_target: &EventLoopWindowTarget<T>) -> CustomCursor {
    let img = image::load_from_memory(bytes).unwrap().to_rgba8();
    let samples = img.into_flat_samples();
    let (_, w, h) = samples.extents();
    let (w, h) = (w as u16, h as u16);
    let builder = CustomCursor::from_rgba(samples.samples, w, h, w / 2, h / 2).unwrap();

    builder.build(window_target)
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
        decode_cursor(include_bytes!("data/cross.png"), &event_loop),
        decode_cursor(include_bytes!("data/cross2.png"), &event_loop),
    ];

    event_loop.run(move |event, _elwt| match event {
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        logical_key: key,
                        ..
                    },
                ..
            } => match key.as_ref() {
                Key::Character("1") => {
                    log::debug!("Setting cursor to {:?}", cursor_idx);
                    window.set_custom_cursor(&custom_cursors[cursor_idx]);
                    cursor_idx = (cursor_idx + 1) % 2;
                }
                Key::Character("2") => {
                    log::debug!("Setting cursor icon to default");
                    window.set_cursor_icon(Default::default());
                }
                Key::Character("3") => {
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
