extern crate winit;

fn main() {
    println!("Available monitors:");
    for (num, monitor) in winit::get_available_monitors().enumerate() {
        println!("\tMonitor #{}:", num);
        println!("\t\tName: {:?}", monitor.get_name());
        println!("\t\tNative Identifier: {:?}", monitor.get_native_identifier());
        println!("\t\tDimensions: {:?}", monitor.get_dimensions());
    }

    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    let mut monitor = window.monitor_id();
    if let Some(ref monitor) = monitor {
        println!("Window opened on monitor: {}", monitor.get_name().unwrap());
    }
    println!("Try moving the window to a different monitor to see the monitor's name printed to the screen.");

    events_loop.run_forever(|event| {

        let current_monitor = window.monitor_id();
        match (&monitor, &current_monitor) {
            (&None, &Some(ref monitor)) => {
                println!("Window appeared on {}", monitor.get_name().unwrap());
            },
            (&Some(ref monitor), &None) => {
                println!("Window disappeared from {}", monitor.get_name().unwrap());
            },
            (&Some(ref old), &Some(ref new)) => if old.get_name() != new.get_name() {
                println!("Window moved from {} to {}", old.get_name().unwrap(), new.get_name().unwrap());
            },
            _ => (),
        }
        monitor = current_monitor;

        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        }
    });
}
