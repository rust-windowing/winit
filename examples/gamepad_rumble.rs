use std::time::Instant;
use winit::event_loop::EventLoop;

#[derive(Debug, Clone)]
enum Rumble {
    None,
    Left,
    Right,
}

fn main() {
    let event_loop = EventLoop::new();

    // You should generally use `GamepadEvent::Added/Removed` to detect gamepads, as doing that will
    // allow you to more easily support gamepad hotswapping. However, we're using `enumerate` here
    // because it makes this example more concise.
    let gamepads = winit::event::device::GamepadHandle::enumerate(&event_loop).collect::<Vec<_>>();

    let rumble_patterns = &[
        (0.5, Rumble::None),
        (2.0, Rumble::Left),
        (0.5, Rumble::None),
        (2.0, Rumble::Right),
    ];
    let mut rumble_iter = rumble_patterns.iter().cloned().cycle();

    let mut active_pattern = rumble_iter.next().unwrap();
    let mut timeout = active_pattern.0;
    let mut timeout_start = Instant::now();

    event_loop.run(move |_, _, _| {
        if timeout <= active_pattern.0 {
            let t = (timeout / active_pattern.0) * std::f64::consts::PI;
            let intensity = t.sin();

            for g in &gamepads {
                let result = match active_pattern.1 {
                    Rumble::Left => g.rumble(intensity, 0.0),
                    Rumble::Right => g.rumble(0.0, intensity),
                    Rumble::None => Ok(()),
                };

                if let Err(e) = result {
                    println!("Rumble failed: {:?}", e);
                }
            }

            timeout = (Instant::now() - timeout_start).as_millis() as f64 / 1000.0;
        } else {
            active_pattern = rumble_iter.next().unwrap();
            println!(
                "Rumbling {:?} for {:?} seconds",
                active_pattern.1, active_pattern.0
            );

            timeout = 0.0;
            timeout_start = Instant::now();
        }
    });
}
