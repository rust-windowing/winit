use simple_logger::SimpleLogger;
use winit::{
    event::{
        DeviceEvent, ElementState, Event, KeyEvent, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState},
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Super Cursor Grab'n'Hide Simulator 9000")
        .build(&event_loop)
        .unwrap();

    let mut modifiers = ModifiersState::default();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput {
                    event:
                        KeyEvent {
                            logical_key: key,
                            state: ElementState::Released,
                            ..
                        },
                    ..
                } => {
                    // WARNING: Consider using `key_without_modifers()` if available on your platform.
                    // See the `key_binding` example
                    match key {
                        Key::Escape => *control_flow = ControlFlow::Exit,
                        Key::Character(ch) => match ch.to_lowercase().as_str() {
                            "g" => window.set_cursor_grab(!modifiers.shift()).unwrap(),
                            "h" => window.set_cursor_visible(modifiers.shift()),
                            _ => ()
                        }
                        _ => (),
                    }
                }
                WindowEvent::ModifiersChanged(m) => modifiers = m,
                _ => (),
            },
            Event::DeviceEvent { event, .. } => match event {
                DeviceEvent::MouseMotion { delta } => println!("mouse moved: {:?}", delta),
                DeviceEvent::Button { button, state } => match state {
                    ElementState::Pressed => println!("mouse button {} pressed", button),
                    ElementState::Released => println!("mouse button {} released", button),
                },
                _ => (),
            },
            _ => (),
        }
    });
}
