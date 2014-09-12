#![feature(phase)]
#![feature(tuple_indexing)]

#[cfg(target_os = "android")]
#[phase(plugin, link)]
extern crate android_glue;

extern crate gl_init;

use std::io::stdio::stdin;

mod support;

#[cfg(target_os = "android")]
android_start!(main)

fn main() {
    // enumerating monitors
    let monitor = {
        for (num, monitor) in gl_init::get_available_monitors().enumerate() {
            println!("Monitor #{}: {}", num, monitor.get_name());
        }

        print!("Please write the number of the monitor to use: ");
        let num = from_str(stdin().read_line().unwrap().as_slice().trim())
            .expect("Plase enter a number");
        let monitor = gl_init::get_available_monitors().nth(num).expect("Please enter a valid ID");

        println!("Using {}", monitor.get_name());

        monitor
    };

    let window = gl_init::WindowBuilder::new()
        .with_title("Hello world!".to_string())
        .with_fullscreen(monitor)
        .build()
        .unwrap();

    unsafe { window.make_current() };

    
    let context = support::load(&window);

    while !window.is_closed() {
        context.draw_frame((0.0, 1.0, 0.0, 1.0));
        window.swap_buffers();

        println!("{}", window.wait_events().collect::<Vec<gl_init::Event>>());
    }
}
