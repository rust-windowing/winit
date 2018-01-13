extern crate winit;

use std::io::{self, Write};
use std::time::{Duration, Instant};
use std::thread;
use std::rc::Rc;
use winit::{ControlFlow, ElementState, Event, VirtualKeyCode, WindowEvent};

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    // enumerating monitors
    let (monitor, num) = {
        for (num, monitor) in events_loop.get_available_monitors().enumerate() {
            println!("Monitor #{}: {:?}", num, monitor.get_name());
        }

        print!("Please write the number of the monitor to use: ");
        io::stdout().flush().unwrap();

        let mut num = String::new();
        io::stdin().read_line(&mut num).unwrap();
        let num = num.trim().parse().ok().expect("Please enter a number");
        let monitor = events_loop.get_available_monitors().nth(num).expect("Please enter a valid ID");

        println!("Using {:?}", monitor.get_name());

        (monitor, num)
    };

    let mut _window = Rc::new(winit::WindowBuilder::new()
        .with_title("Hello world fullscreen!")
        .with_fullscreen(Some(monitor))
        .build(&events_loop)
        .unwrap());

    let mut fullscreen = true;

    start_loop(|| {
        let mut control_flow: ControlFlow = ControlFlow::Continue;
        let mut enter_pressed = false;
        events_loop.poll_events(|event| { 
            println!("{:?}", event);
            control_flow = match event {
                Event::WindowEvent { event, .. } => {
                    match event {
                        WindowEvent::Closed => ControlFlow::Break,
                        WindowEvent::KeyboardInput {
                            input, .. } 
                         => {
                            if let ElementState::Pressed = input.state {
                                if let Some(VirtualKeyCode::Return) = input.virtual_keycode {
                                    enter_pressed = true;
                                };
                            };
                            ControlFlow::Continue
                        },
                        _ => ControlFlow::Continue
                    }
                },
                _ => ControlFlow::Continue
            };
        });
        if enter_pressed {
            fullscreen = !fullscreen;
            let mut builder = winit::WindowBuilder::new()
                .with_title("Hello world!");
            if fullscreen {
                builder = builder.with_fullscreen(events_loop.get_available_monitors().nth(num));
            }
            _window = Rc::new(
                builder
                .build(&events_loop)
                .unwrap());
        };
        control_flow
    });
}

pub fn start_loop<F>(mut callback: F) where F: FnMut() -> ControlFlow {
    let mut accumulator = Duration::new(0, 0);
    let mut previous_clock = Instant::now();

    loop {
        match callback() {
            ControlFlow::Break => break,
            ControlFlow::Continue => ()
        };

        let now = Instant::now();
        accumulator += now - previous_clock;
        previous_clock = now;

        let fixed_time_stamp = Duration::new(0, 16666667);
        while accumulator >= fixed_time_stamp {
            accumulator -= fixed_time_stamp;

            // if you have a game, update the state here
        }

        thread::sleep(fixed_time_stamp - accumulator);
    }
}
