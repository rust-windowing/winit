use std::fs::File;
use winit::{
    dpi::{PhysicalSize, PhysicalPosition},
    event::{ElementState, Event, KeyboardInput, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{CursorIcon, Icon, WindowBuilder},
};

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_title("A fantastic window!");

    let mut cursor_idx = 0;

    let custom_cursor_icon = {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/icon.png");

        let (icon_rgba, icon_size) = {
            let decoder = png::Decoder::new(File::open(path).expect("Failed to open icon path"));
            let (info, mut reader) = decoder.read_info().expect("Failed to decode icon PNG");

            let mut rgba = vec![0; info.buffer_size()];
            reader.next_frame(&mut rgba).unwrap();

            (rgba, PhysicalSize::new(info.width, info.height))
        };
        Icon::from_rgba_with_hot_spot(
            &icon_rgba,
            icon_size,
            PhysicalPosition::new(2, 10),
        ).expect("Failed to open icon")
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
