#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::monitor::MonitorHandle;
use winit::{event_loop::EventLoop, window::WindowBuilder};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    for mon in window.available_monitors() {
        print_info(mon);
    }
    if let Some(mon) = window.primary_monitor() {
        print_info(mon);
    }
}

fn print_info(monitor: MonitorHandle) {
    if let Some(name) = monitor.name() {
        println!("name: {name}");
    } else {
        println!("name: <none>");
    }
    println!("size: {:?}", monitor.size());
    println!("position: {:?}", monitor.position());
    println!("refresh_rate: {:?} mHz", monitor.refresh_rate_millihertz());
    println!("scale_factor: {:?}", monitor.scale_factor());
    for mode in monitor.video_modes() {
        println!(
            "mode: {:?}, depth = {} bits, refresh rate = {} mHz",
            mode.size(),
            mode.bit_depth(),
            mode.refresh_rate_millihertz()
        );
    }
}
