#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate glutin;

use glutin::{Event, ElementState, MouseCursor};

mod support;

#[cfg(target_os = "android")]
android_start!(main);

fn main() {
    let window = glutin::WindowBuilder::new().build().unwrap();
    window.set_title("A fantastic window!");
    unsafe { window.make_current().unwrap() };

    let context = support::load(&window);
    let cursors = [MouseCursor::Default, MouseCursor::Crosshair, MouseCursor::Hand, MouseCursor::Arrow, MouseCursor::Move, MouseCursor::Text, MouseCursor::Wait, MouseCursor::Help, MouseCursor::Progress, MouseCursor::NotAllowed, MouseCursor::ContextMenu, MouseCursor::NoneCursor, MouseCursor::Cell, MouseCursor::VerticalText, MouseCursor::Alias, MouseCursor::Copy, MouseCursor::NoDrop, MouseCursor::Grab, MouseCursor::Grabbing, MouseCursor::AllScroll, MouseCursor::ZoomIn, MouseCursor::ZoomOut, MouseCursor::EResize, MouseCursor::NResize, MouseCursor::NeResize, MouseCursor::NwResize, MouseCursor::SResize, MouseCursor::SeResize, MouseCursor::SwResize, MouseCursor::WResize, MouseCursor::EwResize, MouseCursor::NsResize, MouseCursor::NeswResize, MouseCursor::NwseResize, MouseCursor::ColResize, MouseCursor::RowResize];
    let mut cursor_idx = 0;

    for event in window.wait_events() {
        match event {
            Event::KeyboardInput(ElementState::Pressed, _, _) => {
                println!("Setting cursor to \"{:?}\"", cursors[cursor_idx]);
                window.set_cursor(cursors[cursor_idx]);
                if cursor_idx < cursors.len() - 1 {
                    cursor_idx += 1;
                } else {
                    cursor_idx = 0;
                }
            },
            Event::Closed => break,
            _ => (),
        }

        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        window.swap_buffers().unwrap();
    }
}
