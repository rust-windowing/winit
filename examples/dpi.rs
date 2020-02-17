use winit::dpi::LogicalSize;
use winit::dpi::PhysicalSize;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

/// Change the DPI settings in Windows while running this.
fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            Event::MainEventsCleared => {
                window.request_redraw();
            }

            // DPI changed happened!
            Event::WindowEvent {
                event:
                    WindowEvent::ScaleFactorChanged {
                        scale_factor,
                        new_inner_size,
                    },
                ..
            } => {
                dbg!((scale_factor, new_inner_size.inner_size()));

                new_inner_size
                    .set_inner_size(
                        LogicalSize {
                            width: 100,
                            height: 200,
                        }
                        .to_physical(scale_factor),
                    )
                    .unwrap();
            }

            _ => (),
        }
    });
}
