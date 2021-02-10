use simple_logger::SimpleLogger;
use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget, Hook},
    window::WindowBuilder,
};

struct ExampleHook;

impl<T: std::fmt::Debug> Hook<T> for ExampleHook {
    fn run<F>(
        &mut self,
        handler: F,
        event: Event<'_, T>,
        target: &EventLoopWindowTarget<T>,
        control_flow: &mut ControlFlow,
    ) where
        F: FnOnce(Event<'_, T>, &EventLoopWindowTarget<T>, &mut ControlFlow),
    {
        println!("before event handler: {:?}", event);
        handler(event, target, control_flow);
        println!("after event handler: {:?}", control_flow);
    }
}

fn main() {
    SimpleLogger::new().init().unwrap();

    let event_loop = EventLoop::new().set_hook(ExampleHook);

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        println!("in handler: {:?}", event);

        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    ..
                } => {
                    window.request_redraw();
                }
                _ => (),
            },
            Event::RedrawRequested(_) => {
                println!("\nredrawing!\n");
            }
            _ => (),
        }
    });
}
