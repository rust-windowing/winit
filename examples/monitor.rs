extern crate winit;

fn main() {
    println!("Available monitors:");
    for (num, monitor) in winit::get_available_monitors().enumerate() {
        println!("\tMonitor #{}: {:?}", num, monitor.get_name());
    }

    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    let mut monitor = window.monitor_id();
    println!("Window opened on monitor: {}", monitor.get_name().unwrap());
    println!("Try moving the window to a different monitor to see the monitor's name printed to the screen.");

    events_loop.run_forever(|event| {

        let current_monitor = window.monitor_id();
        if monitor.get_name() != current_monitor.get_name() {
            println!("Window moved to monitor: {}", current_monitor.get_name().unwrap());
            monitor = current_monitor;
        }

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        }
    });
}
