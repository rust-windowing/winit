extern crate winit;

use std::{time::{Instant, Duration}, thread::sleep};

fn main() {
    let mut running = true;
    let mut material = 0i64;
    let mut events_loop = winit::EventsLoop::new();
    let mut timer = Instant::now();
    let material_duration = Duration::from_secs(2);

    let window = winit::WindowBuilder::new()
        .with_title("A blurry window!")
        .with_blur(true)
        .build(&events_loop)
        .unwrap();

    while running {
        events_loop.poll_events(|event| {
            match event {
                winit::Event::WindowEvent {
                    event: winit::WindowEvent::CloseRequested,
                    ..
                } => running = false,
                winit::Event::WindowEvent {
                    event: winit::WindowEvent::KeyboardInput {
                        input: winit::KeyboardInput {
                            virtual_keycode: Some(winit::VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    },
                    ..
                } => running = false,
                _ => {},
            }
        });

        if timer.elapsed() >= material_duration {
            use winit::os::macos::WindowExt;
            window.set_blur_material(material);
            println!("Set blur material: {}", material);
            material = (material + 1) % 10;
            timer = Instant::now();
        }

        sleep(Duration::from_millis(16));
    }
}
