use std::error::Error;
use winit::{
    event::{Event, StartCause},
    event_loop::{ActiveEventLoop, EventLoop},
};

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;

    Ok(event_loop.run(|event, event_loop| match event {
        Event::NewEvents(StartCause::Init) => {
            dump_monitors(event_loop);
            event_loop.exit()
        }
        _ => {}
    })?)
}

fn dump_monitors(event_loop: &ActiveEventLoop) {
    println!("Monitors information");
    let primary_monitor = event_loop.primary_monitor();
    for monitor in event_loop.available_monitors() {
        let intro = if primary_monitor.as_ref() == Some(&monitor) {
            "Primary monitor"
        } else {
            "Monitor"
        };

        if let Some(name) = monitor.name() {
            println!("{intro}: {name}");
        } else {
            println!("{intro}: [no name]");
        }

        let size = monitor.size();
        print!("  Current mode: {}x{}", size.width, size.height);
        if let Some(m_hz) = monitor.refresh_rate_millihertz() {
            println!(" @ {}.{} Hz", m_hz / 1000, m_hz % 1000);
        } else {
            println!();
        }

        let position = monitor.position();
        println!("  Position: {}, {}", position.x, position.y);

        println!("  Scale factor: {}", monitor.scale_factor());

        println!("  Available modes (width x height x bit-depth):");
        for mode in monitor.video_modes() {
            let size = mode.size();
            let m_hz = mode.refresh_rate_millihertz();
            println!(
                "    {:04}x{:04}x{:02} @ {:>3}.{} Hz",
                size.width,
                size.height,
                mode.bit_depth(),
                m_hz / 1000,
                m_hz % 1000
            );
        }
    }
}
