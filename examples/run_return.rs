extern crate winit;
use winit::platform::desktop::EventLoopExtDesktop;

fn main() {
    let mut events_loop = winit::EventLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    println!("Close the window to continue.");
    events_loop.run_return(|event, _, control_flow| {
        match event {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => *control_flow = winit::ControlFlow::Exit,
            _ => *control_flow = winit::ControlFlow::Wait,
        }
    });
    drop(window);

    let _window_2 = winit::WindowBuilder::new()
        .with_title("A second, fantasticer window!")
        .build(&events_loop)
        .unwrap();

    println!("Wa ha ha! You thought that closing the window would finish this?!");
    events_loop.run_return(|event, _, control_flow| {
        match event {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => *control_flow = winit::ControlFlow::Exit,
            _ => *control_flow = winit::ControlFlow::Wait,
        }
    });

    println!("Okay we're done now for real.");
}
