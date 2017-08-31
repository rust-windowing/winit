macro_rules! group_attr {
    (#[cfg($attr:meta)] $($yes:item)*) => {
        $(#[cfg($attr)] $yes)*
    }
}

group_attr! {

    #[cfg(target_os = "linux")]

    extern crate winit;
    extern crate libc;
    extern crate x11_dl;

    fn main() {
        let mut events_loop = winit::EventsLoop::new();
        unsafe {
            // using empty c string to fallback to LC_CTYPE in environment variables
            libc::setlocale(libc::LC_CTYPE, b"\0".as_ptr() as *const _);
            let xlib = x11_dl::xlib::Xlib::open().expect("get xlib");
            // using empty c string for implementation dependent behavior,
            // which might be the XMODIFIERS set in env
            (xlib.XSetLocaleModifiers)(b"\0".as_ptr() as *const _);
        }
        let _window = winit::Window::new(&events_loop)
            .unwrap();
        events_loop.run_forever(|event| {
            match event {
                winit::Event::WindowEvent {
                    event: winit::WindowEvent::ReceivedCharacter(chr), ..} => {
                        println!("{:?}", chr);
                        winit::ControlFlow::Continue
                    }
                winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => {
                    winit::ControlFlow::Break
                },
                _ => winit::ControlFlow::Continue,
            }
        });
    }
}

#[cfg(not(target_os = "linux"))]

fn main() {
    // do nothing, xim is not available
}
