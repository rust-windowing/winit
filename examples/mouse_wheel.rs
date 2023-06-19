#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[path = "util/fill.rs"]
mod fill;

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Mouse Wheel events")
        .build(&event_loop)
        .unwrap();

    println!(
        r"
When using so called 'natural scrolling' (scrolling that acts like on a touch screen), this is what to expect:

Moving your finger downwards on a scroll wheel should make the window move down, and you should see a positive Y scroll value.

When moving fingers on a trackpad down and to the right, you should see positive X and Y deltas, and the window should move down and to the right.

With reverse scrolling, you should see the inverse behavior.

In both cases the example window should move like the content of a scroll area in any other application.

In other words, the deltas indicate the direction in which to move the content (in this case the window)."
    );

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => control_flow.set_exit(),
                WindowEvent::MouseWheel { delta, .. } => match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        println!("mouse wheel Line Delta: ({x},{y})");
                        let pixels_per_line = 120.0;
                        let mut pos = window.outer_position().unwrap();
                        pos.x += (x * pixels_per_line) as i32;
                        pos.y += (y * pixels_per_line) as i32;
                        window.set_outer_position(pos)
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        println!("mouse wheel Pixel Delta: ({},{})", p.x, p.y);
                        let mut pos = window.outer_position().unwrap();
                        pos.x += p.x as i32;
                        pos.y += p.y as i32;
                        window.set_outer_position(pos)
                    }
                },
                _ => (),
            },
            Event::RedrawRequested(_) => {
                fill::fill_window(&window);
            }
            _ => (),
        }
    });
}
