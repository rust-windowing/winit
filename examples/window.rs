#![allow(clippy::single_match)]
#![feature(impl_trait_in_assoc_type)]

use simple_logger::SimpleLogger;
use winit::{
    event::WindowEvent,
    event_loop::{EventLoop, EventLoopWindowTarget},
    window::{Window, WindowBuilder, WindowId},
    ApplicationHandler,
};

#[path = "util/fill.rs"]
mod fill;

#[derive(Debug)]
struct GraphicsContext;

#[derive(Debug)]
struct App {
    window: Window,
    // TODO: Put the context & surface from `fill` in here
    _graphics_context: GraphicsContext,
}

impl ApplicationHandler for App {
    type Waker = impl FnOnce(&EventLoopWindowTarget) -> Self;

    // Note: We do _not_ pass the elwt here, since we don't want users to
    // create windows when the app is about to suspend.
    fn suspend(self) -> Self::Waker {
        println!("---suspended---");
        let Self { window, .. } = self;
        |_elwt| {
            println!("---resumed---");
            Self {
                window,
                _graphics_context: GraphicsContext,
            }
        }
    }

    fn window_event(
        &mut self,
        elwt: &EventLoopWindowTarget,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        println!("{event:?}");
        if window_id != self.window.id() {
            return;
        }
        match event {
            WindowEvent::CloseRequested => elwt.exit(),
            WindowEvent::RedrawRequested => {
                // Notify the windowing system that we'll be presenting to the window.
                self.window.pre_present_notify();
                fill::fill_window(&self.window);
            }
            _ => (),
        }
    }

    fn about_to_wait(&mut self, _elwt: &EventLoopWindowTarget) {
        // self.window.request_redraw();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    event_loop.run_with(|elwt| {
        elwt.set_wait();

        let window = WindowBuilder::new()
            .with_title("A fantastic window!")
            .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
            .build(elwt)
            .unwrap();

        App {
            window,
            _graphics_context: GraphicsContext,
        }
    })?;

    Ok(())
}
