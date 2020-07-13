use std::{path::Path, fs::File};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{CursorIcon, RgbaIcon, Icon, WindowBuilder},
};

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_title("A fantastic window!");

    let mut cursor_idx = 0;

    let custom_cursor_icon = {
        let base_path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icons/icon_folder/"));

        Icon::from_rgba_fn(move |size, _| {
            let path = base_path.join(format!("{}.png", size.width));
            let (icon_rgba, icon_size) = {
                let decoder = png::Decoder::new(File::open(path)?);
                let (info, mut reader) = decoder.read_info()?;

                let mut rgba = vec![0; info.buffer_size()];
                reader.next_frame(&mut rgba).unwrap();

                (rgba, PhysicalSize::new(info.width, info.height))
            };
            Ok(RgbaIcon::from_rgba_with_hot_spot(icon_rgba, icon_size, PhysicalPosition::new(0, 0)))
        })
    };

    let cursors = vec![
        CursorIcon::Custom(custom_cursor_icon),
        CursorIcon::Default,
        CursorIcon::Crosshair,
        CursorIcon::Hand,
        CursorIcon::Arrow,
        CursorIcon::Move,
        CursorIcon::Text,
        CursorIcon::Wait,
        CursorIcon::Help,
        CursorIcon::Progress,
        CursorIcon::NotAllowed,
        CursorIcon::ContextMenu,
        CursorIcon::Cell,
        CursorIcon::VerticalText,
        CursorIcon::Alias,
        CursorIcon::Copy,
        CursorIcon::NoDrop,
        CursorIcon::Grab,
        CursorIcon::Grabbing,
        CursorIcon::AllScroll,
        CursorIcon::ZoomIn,
        CursorIcon::ZoomOut,
        CursorIcon::EResize,
        CursorIcon::NResize,
        CursorIcon::NeResize,
        CursorIcon::NwResize,
        CursorIcon::SResize,
        CursorIcon::SeResize,
        CursorIcon::SwResize,
        CursorIcon::WResize,
        CursorIcon::EwResize,
        CursorIcon::NsResize,
        CursorIcon::NeswResize,
        CursorIcon::NwseResize,
        CursorIcon::ColResize,
        CursorIcon::RowResize,
    ];

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => {
                println!("Setting cursor to \"{:?}\"", cursors[cursor_idx]);
                window.set_cursor_icon(cursors[cursor_idx].clone());
                if cursor_idx < cursors.len() - 1 {
                    cursor_idx += 1;
                } else {
                    cursor_idx = 0;
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
                return;
            }
            _ => (),
        }
    });
}
