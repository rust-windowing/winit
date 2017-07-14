extern crate winit;


#[cfg(windows)]
extern crate user32;
#[cfg(windows)]
extern crate gdi32;

// Note: this example will only make sense if you change your DPI from your settings, or have a hi-DPI monitor.

#[cfg(windows)]
fn main() {
	winit::set_process_high_dpi_aware();

    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("A high-DPI window")
        .with_dimensions(300, 100)
        .build(&events_loop)
        .unwrap();

    // Get window handle.
    use winit::os::windows::WindowExt;
    let hwnd = unsafe { std::mem::transmute(window.get_hwnd()) };

    // Create some text using the default font to check high DPI support.
    let message: Vec<_> = "★ Is high DPI supported?"
        .encode_utf16()
        .chain(Some(0))
        .collect();

    // Paint with GDI.
    unsafe {
        let dc = user32::GetDC(hwnd);

        gdi32::TextOutW(
            dc,
            10, 10,
            message.as_ptr(),
            message.len() as i32
        );

        gdi32::DeleteDC(dc);
    }

    events_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::DPIChanged(x_dpi, y_dpi), .. } => {
                println!("New DPI: {}x{}", x_dpi, y_dpi);
                winit::ControlFlow::Continue
            },
            winit::Event::WindowEvent { event: winit::WindowEvent::Closed, .. } => {
                winit::ControlFlow::Break
            },
            _ => winit::ControlFlow::Continue,
        }
    });
}

#[cfg(not(windows))]
fn main() { }
