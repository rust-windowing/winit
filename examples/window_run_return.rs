use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::desktop::EventLoopExtDesktop,
    window::WindowBuilder,
};

fn main() {
    let mut event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    println!("Close the window to continue.");
    event_loop.run_return(|event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,
        _ => *control_flow = ControlFlow::Wait,
    });
    drop(window);

    let _window_2 = WindowBuilder::new()
        .with_title("A second, fantasticer window!")
        .build(&event_loop)
        .unwrap();

    println!("Wa ha ha! You thought that closing the window would finish this?!");
    event_loop.run_return(|event, _, control_flow| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } => *control_flow = ControlFlow::Exit,
        _ => *control_flow = ControlFlow::Wait,
    });

    println!("Okay we're done now for real.");
}
