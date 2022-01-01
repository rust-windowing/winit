
#[cfg(target_os = "macos")]
fn main() {
    // use simple_logger::SimpleLogger;
    use winit::{
        event::{Event, WindowEvent},
        event_loop::{ControlFlow, EventLoop},
        window::WindowBuilder,
        platform::macos::{EventLoopExtMacOS, objc::{self, sel, sel_impl}}
    };
    // SimpleLogger::new().init().unwrap();
    let mut event_loop = EventLoop::new();

    event_loop.add_application_method(
        sel!(applicationDidChangeOcclusionState:),
        Box::new(|_notification: *mut objc::runtime::Object| {
            println!("The occlusion state has changed!");
        }) as Box<dyn Fn(_)>
    ).unwrap();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        // println!("{:?}", event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => (),
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("There's currently no example for how to register handlers for native events on this platform");
}
