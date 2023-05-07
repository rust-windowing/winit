#![allow(clippy::single_match)]

use simple_logger::SimpleLogger;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::monitor::MonitorHandle;
use winit::{event_loop::EventLoop, window::WindowBuilder};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    if let Some(mon) = window.primary_monitor() {
        print_info("Primary output", mon);
    }

    for mon in window.available_monitors() {
        if Some(&mon) == window.primary_monitor().as_ref() {
            continue;
        }

        println!();
        print_info("Output", mon);
    }
}

fn print_info(intro: &str, monitor: MonitorHandle) {
    if let Some(name) = monitor.name() {
        println!("{intro}: {name}");
    } else {
        println!("{intro}: [no name]");
    }

    let PhysicalSize { width, height } = monitor.size();
    print!("  Current mode: {width}x{height}");
    if let Some(m_hz) = monitor.refresh_rate_millihertz() {
        println!(" @ {}.{} Hz", m_hz / 1000, m_hz % 1000);
    } else {
        println!();
    }

    let PhysicalPosition { x, y } = monitor.position();
    println!("  Position: {x},{y}");

    println!("  Scale factor: {}", monitor.scale_factor());

    println!("  Available modes (width x height x bit-depth):");
    for mode in monitor.video_modes() {
        let PhysicalSize { width, height } = mode.size();
        let bits = mode.bit_depth();
        let m_hz = mode.refresh_rate_millihertz();
        println!(
            "    {width}x{height}x{bits} @ {}.{} Hz",
            m_hz / 1000,
            m_hz % 1000
        );
    }
}
