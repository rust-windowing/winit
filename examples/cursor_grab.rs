use winit::{
    event::{Event, LogicalKey, ModifiersState, RawPointerEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("Super Cursor Grab'n'Hide Simulator 9000")
        .build(&event_loop)
        .unwrap();

    let mut modifiers = ModifiersState::default();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent(_, event) => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::Key(e) if e.is_down() => match e.logical_key() {
                    Some(LogicalKey::Escape) => *control_flow = ControlFlow::Exit,
                    Some(LogicalKey::G) => window.set_cursor_grab(!modifiers.shift()).unwrap(),
                    Some(LogicalKey::H) => window.set_cursor_visible(modifiers.shift()),
                    _ => (),
                },
                WindowEvent::ModifiersChanged(m) => modifiers = m,
                _ => (),
            },
            Event::RawPointerEvent(_, event) => match event {
                RawPointerEvent::MovedRelative(delta) => println!("pointer moved: {:?}", delta),
                RawPointerEvent::MovedAbsolute(position) => {
                    println!("pointer moved to: {:?}", position)
                }
                RawPointerEvent::Press(e) if e.is_down() => {
                    println!("pointer button {:?} pressed", e.button())
                }
                RawPointerEvent::Press(e) if e.is_up() => {
                    println!("pointer button {:?} released", e.button())
                }
                _ => (),
            },
            _ => (),
        }
    });
}
