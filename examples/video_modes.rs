#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::event_loop::EventLoop;

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();
    let monitor = match event_loop.primary_monitor() {
        Some(monitor) => monitor,
        None => {
            println!("No primary monitor detected.");
            return;
        }
    };

    println!("Listing available video modes:");

    for mode in monitor.video_modes() {
        println!("{mode}");
    }
}
