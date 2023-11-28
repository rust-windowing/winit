#![allow(clippy::single_match)]

//! Example for focusing a window.

use simple_logger::SimpleLogger;
#[cfg(not(wasm_platform))]
use std::time;
#[cfg(wasm_platform)]
use web_time as time;
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[path = "util/fill.rs"]
mod fill;

fn main() -> Result<(), impl std::error::Error> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
        .build(&event_loop)
        .unwrap();

    let mut deadline = time::Instant::now() + time::Duration::from_secs(3);
    event_loop.run(move |event, elwt| {
        match event {
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                // Timeout reached; focus the window.
                println!("Re-focusing the window.");
                deadline += time::Duration::from_secs(3);
                window.focus_window();
            }
            Event::WindowEvent { event, window_id } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::RedrawRequested => {
                    // Notify the windowing system that we'll be presenting to the window.
                    window.pre_present_notify();
                    fill::fill_window(&window);
                }
                _ => (),
            },
            Event::AboutToWait => {
                window.request_redraw();
            }

            _ => (),
        }

        elwt.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(deadline));
    })
}
