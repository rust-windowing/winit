#![allow(clippy::single_match)]

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use winit::{
    dpi::LogicalSize,
    event::{ElementState, Event, WindowEvent},
    event_loop::EventLoop,
    keyboard::{Key, ModifiersState},
    // WARNING: This is not available on all platforms (for example on the web).
    platform::modifier_supplement::KeyEventExtModifierSupplement,
    window::WindowBuilder,
};

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn main() {
    println!("This example is not supported on this platform");
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn main() -> Result<(), impl std::error::Error> {
    #[path = "util/fill.rs"]
    mod fill;

    simple_logger::SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new().unwrap();

    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(400.0, 200.0))
        .build(&event_loop)
        .unwrap();

    let mut modifiers = ModifiersState::default();

    event_loop.run(move |event, elwt| {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::ModifiersChanged(new) => {
                    modifiers = new.state();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state == ElementState::Pressed && !event.repeat {
                        match event.key_without_modifiers().as_ref() {
                            Key::Character("1") => {
                                if modifiers.shift_key() {
                                    println!("Shift + 1 | logical_key: {:?}", event.logical_key);
                                } else {
                                    println!("1");
                                }
                            }
                            _ => (),
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    fill::fill_window(&window);
                }
                _ => (),
            }
        };
    })
}
