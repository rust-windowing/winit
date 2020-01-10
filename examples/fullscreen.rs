use std::io::{stdin, stdout, Write};
use winit::event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::monitor::{MonitorHandle, VideoMode};
use winit::window::{Fullscreen, WindowBuilder};

fn main() {
    simple_logger::init().unwrap();
    let event_loop = EventLoop::new();

    print!("Please choose the fullscreen mode: (1) exclusive, (2) borderless: ");
    stdout().flush().unwrap();

    let mut num = String::new();
    stdin().read_line(&mut num).unwrap();
    let num = num.trim().parse().ok().expect("Please enter a number");

    let fullscreen = Some(match num {
        1 => Fullscreen::Exclusive(prompt_for_video_mode(&prompt_for_monitor(&event_loop))),
        2 => Fullscreen::Borderless(prompt_for_monitor(&event_loop)),
        _ => panic!("Please enter a valid number"),
    });

    let mut is_maximized = false;
    let mut decorations = true;

    let window = WindowBuilder::new()
        .with_title("Hello world!")
        .with_fullscreen(fullscreen.clone())
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state,
                            ..
                        },
                    ..
                } => match (virtual_code, state) {
                    (VirtualKeyCode::Escape, _) => *control_flow = ControlFlow::Exit,
                    (VirtualKeyCode::F, ElementState::Pressed) => {
                        if window.fullscreen().is_some() {
                            window.set_fullscreen(None);
                        } else {
                            window.set_fullscreen(fullscreen.clone());
                        }
                    }
                    (VirtualKeyCode::S, ElementState::Pressed) => {
                        println!("window.fullscreen {:?}", window.fullscreen());
                    }
                    (VirtualKeyCode::M, ElementState::Pressed) => {
                        is_maximized = !is_maximized;
                        window.set_maximized(is_maximized);
                    }
                    (VirtualKeyCode::D, ElementState::Pressed) => {
                        decorations = !decorations;
                        window.set_decorations(decorations);
                    }
                    _ => (),
                },
                _ => (),
            },
            _ => {}
        }
    });
}

// Enumerate monitors and prompt user to choose one
fn prompt_for_monitor(event_loop: &EventLoop<()>) -> MonitorHandle {
    for (num, monitor) in event_loop.available_monitors().enumerate() {
        println!("Monitor #{}: {:?}", num, monitor.name());
    }

    print!("Please write the number of the monitor to use: ");
    stdout().flush().unwrap();

    let mut num = String::new();
    stdin().read_line(&mut num).unwrap();
    let num = num.trim().parse().ok().expect("Please enter a number");
    let monitor = event_loop
        .available_monitors()
        .nth(num)
        .expect("Please enter a valid ID");

    println!("Using {:?}", monitor.name());

    monitor
}

fn prompt_for_video_mode(monitor: &MonitorHandle) -> VideoMode {
    for (i, video_mode) in monitor.video_modes().enumerate() {
        println!("Video mode #{}: {}", i, video_mode);
    }

    print!("Please write the number of the video mode to use: ");
    stdout().flush().unwrap();

    let mut num = String::new();
    stdin().read_line(&mut num).unwrap();
    let num = num.trim().parse().ok().expect("Please enter a number");
    let video_mode = monitor
        .video_modes()
        .nth(num)
        .expect("Please enter a valid ID");

    println!("Using {}", video_mode);

    video_mode
}
