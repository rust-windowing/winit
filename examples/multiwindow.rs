extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window1 = winit::Window::new(&events_loop).unwrap();
    let window2 = winit::Window::new(&events_loop).unwrap();
    let window3 = winit::Window::new(&events_loop).unwrap();

    let mut num_windows = 3;

    if cfg!(target_os = "linux") {
        println!("Running this example under wayland may not display a window at all.\n\
                  This is normal and because this example does not actually draw anything in the window,\
                  thus the compositor does not display it.");
    }

    events_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, window_id } => {
                if window_id == window1.id() {
                    println!("Window 1 has been closed")
                } else if window_id == window2.id() {
                    println!("Window 2 has been closed")
                } else if window_id == window3.id() {
                    println!("Window 3 has been closed");
                } else {
                    unreachable!()
                }

                num_windows -= 1;
                if num_windows == 0 {
                    return winit::ControlFlow::Break;
                }
            },
            _ => (),
        }
        winit::ControlFlow::Continue
    })
}
