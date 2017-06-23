extern crate winit;
extern crate libc;
extern crate x11_dl;

fn main() {
    let mut events_loop = winit::EventsLoop::new();
    unsafe {
        libc::setlocale(libc::LC_CTYPE, b"\0".as_ptr() as *const _);
        let xlib = x11_dl::xlib::Xlib::open().expect("get xlib");
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
