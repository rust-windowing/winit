#![allow(clippy::single_match)]
use std::thread;
use std::time::Duration;

use simple_logger::SimpleLogger;
use winit::event_loop::EventLoop;

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    println!("Primary monitor: {:#?}", event_loop.primary_monitor());

    let mut monitors = event_loop.available_monitors().collect::<Vec<_>>();
    println!("Monitor list: {:#?}", monitors);

    loop {
        let new_monitors = event_loop.available_monitors().collect::<Vec<_>>();
        if new_monitors != monitors {
            println!(
                "Monitor list changed from {:#?} to {:#?}",
                monitors, new_monitors
            );
            monitors = new_monitors;
        }

        // Sleep for the example; in practice, you should not need to listen
        // for monitor changes.
        thread::sleep(Duration::from_secs(1));
    }
}
