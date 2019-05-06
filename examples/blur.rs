extern crate winit;
extern crate winapi;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("A blurry window!")
        .with_blur(true)
        .build(&events_loop)
        .unwrap();

    #[cfg(target_os = "macos")] {
        // On macOS the blur material is 'light' by default.
        // Let's change it to a dark theme!
        use winit::os::macos::{BlurMaterial, WindowExt};
        unsafe { window.set_blur_material(BlurMaterial::Dark) };
    }

    events_loop.run_forever(|event| match event {
        winit::Event::WindowEvent {
            event: winit::WindowEvent::CloseRequested,
            ..
        } => winit::ControlFlow::Break,
        winit::Event::WindowEvent {
            event:
                winit::WindowEvent::KeyboardInput {
                    input:
                        winit::KeyboardInput {
                            virtual_keycode: Some(winit::VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                },
            ..
        } => winit::ControlFlow::Break,
        winit::Event::WindowEvent {
            event: winit::WindowEvent::Refresh,
            window_id,
        } if window_id == window.id() => {
            paint_window(&window); // Important!
            winit::ControlFlow::Continue
        },
        _ => winit::ControlFlow::Continue,
    });
}

fn paint_window(window: &winit::Window) {
    // On Windows we need to paint the color black onto the window.
    // The black color is made transparent by the compositor.
    #[cfg(target_os = "windows")] {
        use winapi::um::winuser;
        use winapi::shared::windef;
        use winit::os::windows::WindowExt;

        let window = window.get_hwnd() as windef::HWND;

        unsafe {
            let mut ps: winuser::PAINTSTRUCT = std::mem::zeroed();
            let hdc = winuser::BeginPaint(window, &mut ps as *mut _);
            let _ = winuser::FillRect(hdc, &ps.rcPaint as *const _, (winuser::COLOR_WINDOWTEXT + 1) as _);
            let _ = winuser::EndPaint(window, &mut ps as *mut _);
        }
    }
}
