extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window1 = winit::Window::new(&events_loop).unwrap();
    let window2 = winit::Window::new(&events_loop).unwrap();
    let window3 = winit::Window::new(&events_loop).unwrap();
    let window4 = winit::Window::new(&events_loop).unwrap();

    let window4_id = window4.id();
    let mut window4_opt = Some(window4);

    let mut num_windows = 4;

    println!("Press any key on any window to drop the 4th window explicitly. (Testing impl Drop for Window)");

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
                } else if window_id == window4_id {
                    println!("Window 4 has been closed");
                } else {
                    unreachable!()
                }

                num_windows -= 1;
                if num_windows == 0 {
                    return winit::ControlFlow::Break;
                }
            },

            winit::Event::WindowEvent { event: winit::WindowEvent::KeyboardInput{..}, .. } => {
                println!("Dropping window 4 explicitly");
                window4_opt = None;
            },

            _ => (),
        }
        winit::ControlFlow::Continue
    })
}
