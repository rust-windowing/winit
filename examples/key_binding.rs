use simple_logger::SimpleLogger;
use winit::{
    dpi::LogicalSize,
    event::{keyboard_types::KeyState, Event, KeyEvent, ModifiersState, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

/////////////////////////////////////////////////////////////////////////////
// WARNING: This is not available on all platforms (for example on the web).
use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
/////////////////////////////////////////////////////////////////////////////

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let _window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(400.0, 200.0))
        .build(&event_loop)
        .unwrap();

    let mut modifiers = ModifiersState::default();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::ModifiersChanged(new_state) => {
                    modifiers = new_state;
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    handle_key_event(modifiers, event);
                }
                _ => (),
            },
            _ => (),
        };
    });
}

fn handle_key_event(modifiers: ModifiersState, event: KeyEvent) {
    if event.state == KeyState::Down && !event.repeat {
        match event.key_without_modifers() {
            keyboard_types::Key::Character(c) => match c.as_str() {
                "1" => {
                    if modifiers.shift() {
                        println!("Shift + 1 | logical_key: {:?}", event.logical_key);
                    } else {
                        println!("1");
                    }
                }
                _ => (),
            },
            _ => (),
        }
    }
}
