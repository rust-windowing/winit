extern crate winit;

use winit::{Event, ElementState, MouseCursor, WindowEvent, KeyboardInput, ControlFlow};

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new().build(&events_loop).unwrap();
    window.set_title("A fantastic window!");

    let cursors = [MouseCursor::Default, MouseCursor::Crosshair, MouseCursor::Hand, MouseCursor::Arrow, MouseCursor::Move, MouseCursor::Text, MouseCursor::Wait, MouseCursor::Help, MouseCursor::Progress, MouseCursor::NotAllowed, MouseCursor::ContextMenu, MouseCursor::NoneCursor, MouseCursor::Cell, MouseCursor::VerticalText, MouseCursor::Alias, MouseCursor::Copy, MouseCursor::NoDrop, MouseCursor::Grab, MouseCursor::Grabbing, MouseCursor::AllScroll, MouseCursor::ZoomIn, MouseCursor::ZoomOut, MouseCursor::EResize, MouseCursor::NResize, MouseCursor::NeResize, MouseCursor::NwResize, MouseCursor::SResize, MouseCursor::SeResize, MouseCursor::SwResize, MouseCursor::WResize, MouseCursor::EwResize, MouseCursor::NsResize, MouseCursor::NeswResize, MouseCursor::NwseResize, MouseCursor::ColResize, MouseCursor::RowResize];
    let mut cursor_idx = 0;

    events_loop.run_forever(|event| {
        match event {
            Event::WindowEvent { event: WindowEvent::KeyboardInput { input: KeyboardInput { state: ElementState::Pressed, .. }, .. }, .. } => {
                println!("Setting cursor to \"{:?}\"", cursors[cursor_idx]);
                window.set_cursor(cursors[cursor_idx]);
                if cursor_idx < cursors.len() - 1 {
                    cursor_idx += 1;
                } else {
                    cursor_idx = 0;
                }
            },
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                return ControlFlow::Break;
            },
            _ => ()
        }
        ControlFlow::Continue
    });
}
