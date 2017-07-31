extern crate keyboard_types;
extern crate winit;

use winit::{ControlFlow, WindowEvent};

use keyboard_types::*;

fn print_line(type_: &'static str, key: Option<Key>, is_composing: bool, data: Option<String>) {
    let composing_string = if (&key).is_some() {
        format!("{:?}", is_composing)
    } else {
        String::new()
    };
    let key_string = match key {
        None => String::new(),
        Some(Key::Character(s)) => s,
        Some(k) => format!("{:?}", k),
    };
    println!("{:<20} {:<15} {:<5}  {}", type_, key_string, composing_string, data.unwrap_or(String::new()))
}

fn process_event(event: WindowEvent) -> ControlFlow {
    match event {
        WindowEvent::Closed => {
            return ControlFlow::Break;
        }
        WindowEvent::KeyboardInput { input, .. } => {
            let state = match input.state {
                KeyState::Down if !input.repeat => "keydown",
                KeyState::Down => "keydown (rep)",
                KeyState::Up => "keyup",
            };
            print_line(state, Some(input.key), input.is_composing, None);
        }
        WindowEvent::CompositionInput { input, .. } => {
            let state = match input.state {
                CompositionState::Start => "compositionstart",
                CompositionState::Update => "compositionupdate",
                CompositionState::End => "compositionend",
            };
            print_line(state, None, false, Some(input.data));
        }
        WindowEvent::ReceivedCharacter(c) => {
            println!("{:<20} {}", "char", c);
        }
        _ => {}
    }
    ControlFlow::Continue
}

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    if cfg!(target_os = "linux") {
        println!("Running this example under wayland may not display a window at all.\n\
                  This is normal and because this example does not actually draw anything in the window,\
                  thus the compositor does not display it.");
    }

    events_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent { event, .. } => {
                process_event(event)
            }
            _ => winit::ControlFlow::Continue,
        }
    });
}