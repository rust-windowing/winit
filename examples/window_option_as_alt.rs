#![allow(clippy::single_match)]

#[cfg(target_os = "macos")]
use winit::platform::macos::{OptionAsAlt, WindowExtMacOS};

#[cfg(target_os = "macos")]
use winit::{
    event::ElementState,
    event::{Event, MouseButton, WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};

#[cfg(target_os = "macos")]
#[path = "util/fill.rs"]
mod fill;

/// Prints the keyboard events characters received when option_is_alt is true versus false.
/// A left mouse click will toggle option_is_alt.
#[cfg(target_os = "macos")]
fn main() -> Result<(), impl std::error::Error> {
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(winit::dpi::LogicalSize::new(128.0, 128.0))
        .build(&event_loop)
        .unwrap();

    window.set_ime_allowed(true);

    let mut option_as_alt = window.option_as_alt();

    event_loop.run(move |event, elwt| match event {
        Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            window_id,
        } if window_id == window.id() => elwt.exit(),
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                option_as_alt = match option_as_alt {
                    OptionAsAlt::None => OptionAsAlt::OnlyLeft,
                    OptionAsAlt::OnlyLeft => OptionAsAlt::OnlyRight,
                    OptionAsAlt::OnlyRight => OptionAsAlt::Both,
                    OptionAsAlt::Both => OptionAsAlt::None,
                };

                println!("Received Mouse click, toggling option_as_alt to: {option_as_alt:?}");
                window.set_option_as_alt(option_as_alt);
            }
            WindowEvent::KeyboardInput { .. } => println!("KeyboardInput: {event:?}"),
            WindowEvent::RedrawRequested => {
                fill::fill_window(&window);
            }
            _ => (),
        },
        Event::AboutToWait => {
            window.request_redraw();
        }

        _ => (),
    })
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("This example is only supported on MacOS");
}
