extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .build(&events_loop)
        .unwrap();

    let proxy = events_loop.create_proxy();

    if cfg!(target_os = "linux") {
        println!("Running this example under wayland may not display a window at all.\n\
                  This is normal and because this example does not actually draw anything in the window,\
                  thus the compositor does not display it.");
    }

    std::thread::spawn(move || {
        // Wake up the `events_loop` once every second.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            proxy.wakeup().unwrap();
        }
    });

    events_loop.run_forever(|event| {
        println!("{:?}", event);
        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } =>
                winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        }
    });
}
