extern crate winit;

use std::io::{self, Write};

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

    let window = winit::WindowBuilder::new()
        .with_title("Hello world!")
        .with_fullscreen(monitor)
        .build()
        .unwrap();

    for event in window.wait_events() {
        println!("{:?}", event);

        match event {
            winit::Event::Closed => break,
            winit::Event::KeyboardInput(_, _, Some(winit::VirtualKeyCode::Escape)) => break,
            _ => ()
        }
    }
}
