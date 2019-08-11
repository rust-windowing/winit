use std::time::{Duration, Instant};
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

#[derive(Debug, Clone, Copy)]
enum CustomEvent {
    Timer,
}

fn main() {
    let event_loop = EventLoop::<CustomEvent>::with_user_event();
    // `EventLoopProxy` allows you to dispatch custom events to the main Winit event
    // loop from any thread.
    let event_loop_proxy = event_loop.create_proxy();

    let timer_length = Duration::new(1, 0);

    let _window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        match event {
            // When the event loop initially starts up, queue the timer.
            Event::NewEvents(StartCause::Init) => {
                *control_flow = ControlFlow::WaitUntil(Instant::now() + timer_length);
            }

            // When the timer expires, dispatch a timer event and queue a new timer.
            Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
                event_loop_proxy.send_event(CustomEvent::Timer).ok();
                *control_flow = ControlFlow::WaitUntil(Instant::now() + timer_length);
            }

            Event::UserEvent(event) => println!("user event: {:?}", event),

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                *control_flow = ControlFlow::Exit;
            }

            _ => (),
        }
    });
}
