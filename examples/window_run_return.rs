extern crate winit;

use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{EventLoop, ControlFlow};
use winit::platform::desktop::EventLoopExtDesktop;

fn main() {
    let mut events_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    println!("Close the window to continue.");
    events_loop.run_return(|event, _, control_flow| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });
    drop(window);

    let _window_2 = WindowBuilder::new()
        .with_title("A second, fantasticer window!")
        .build(&events_loop)
        .unwrap();

    println!("Wa ha ha! You thought that closing the window would finish this?!");
    events_loop.run_return(|event, _, control_flow| {
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => *control_flow = ControlFlow::Exit,
            _ => *control_flow = ControlFlow::Wait,
        }
    });

    println!("Okay we're done now for real.");
}
