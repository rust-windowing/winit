#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::EventLoop,
    window::{WindowArea, WindowBuilder},
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(winit::dpi::LogicalSize::new(400.0, 400.0))
        .with_decorations(false)
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::NewEvents(StartCause::Init) => {
                eprintln!("Click on window edges to start resizing. Click anywhere in the top 30px to start dragging the window.")
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            Event::WindowEvent {
                event: WindowEvent::HitTest{ x, y, new_area},
                ..
            } => {
                let size = window.inner_size();
                let h = size.height;
                let w = size.width;

                const MARGIN: u32 = 3;

                *new_area = match (x, y) {
                    _ if x <= MARGIN && y <= MARGIN => WindowArea::TOPLEFT,
                    _ if x >= w - MARGIN && y <= MARGIN => WindowArea::TOPRIGHT,
                    _ if x >= w - MARGIN && y >= h - MARGIN => WindowArea::BOTTOMRIGHT,
                    _ if x <= MARGIN && y >= h - MARGIN => WindowArea::BOTTOMLEFT,
                    _ if x <= MARGIN => WindowArea::LEFT,
                    _ if y <= MARGIN => WindowArea::TOP,
                    _ if x >= w - MARGIN => WindowArea::RIGHT,
                    _ if y >= h - MARGIN => WindowArea::BOTTOM,
                    (_, 10..=30) => WindowArea::CAPTION,
                    _ => WindowArea::CLIENT,
                }
            }
            _ => (),
        }
    });
}
