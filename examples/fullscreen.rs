#[cfg(target_os = "android")]
#[macro_use]
extern crate android_glue;

extern crate glutin;

use std::io::{self, Write};

mod support;

#[cfg(target_os = "android")]
android_start!(main);

fn main() {
    // enumerating monitors
    let monitor = {
        for (num, monitor) in glutin::get_available_monitors().enumerate() {
            println!("Monitor #{}: {:?}", num, monitor.get_name());
        }

        print!("Please write the number of the monitor to use: ");
        io::stdout().flush().unwrap();

        let mut num = String::new();
        io::stdin().read_line(&mut num).unwrap();
        let num = num.trim().parse().ok().expect("Please enter a number");
        let monitor = glutin::get_available_monitors().nth(num).expect("Please enter a valid ID");

        println!("Using {:?}", monitor.get_name());

        monitor
    };

    let window = glutin::WindowBuilder::new()
        .with_title("Hello world!")
        .with_fullscreen(monitor)
        .build()
        .unwrap();

    let _ = unsafe { window.make_current() };

    
    let context = support::load(&window);

    for event in window.wait_events() {
        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        let _ = window.swap_buffers();

        println!("{:?}", event);

        match event {
            glutin::Event::Closed => break,
            glutin::Event::KeyboardInput(_, _, Some(glutin::VirtualKeyCode::Escape)) => break,
            _ => ()
        }
    }
}
