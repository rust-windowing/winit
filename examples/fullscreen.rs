extern crate winit;

use std::io::{self, Write};
use winit::{ControlFlow, Event, WindowEvent};

fn main() {
    // enumerating monitors
    let monitor = {
        for (num, monitor) in winit::get_available_monitors().enumerate() {
            println!("Monitor #{}: {:?}", num, monitor.get_name());
        }

        print!("Please write the number of the monitor to use: ");
        io::stdout().flush().unwrap();

        let mut num = String::new();
        io::stdin().read_line(&mut num).unwrap();
        let num = num.trim().parse().ok().expect("Please enter a number");
        let monitor = winit::get_available_monitors().nth(num).expect("Please enter a valid ID");

        println!("Using {:?}", monitor.get_name());

        monitor
    };

    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("Hello world!")
        .with_fullscreen(monitor)
        .build(&events_loop)
        .unwrap();

    if cfg!(target_os = "linux") {
        println!("Running this example under wayland may not display a window at all.\n\
                  This is normal and because this example does not actually draw anything in the window,\
                  thus the compositor does not display it.");
    }

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent { event, .. } => {
                match event {
                    WindowEvent::Closed => return ControlFlow::Break,
                    WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput { virtual_keycode: Some(winit::VirtualKeyCode::Escape), .. }, ..
                    } => return ControlFlow::Break,
                    _ => ()
                }
            },
            _ => {}
        }

        ControlFlow::Continue
    });
}
