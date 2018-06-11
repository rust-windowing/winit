extern crate winit;

fn main() {
    let mut events_loop = winit::EventsLoop::new();

    let window = winit::WindowBuilder::new()
        .with_title("A blurry window!")
        .with_blur(true)
        .build(&events_loop)
        .unwrap();

    #[cfg(target_os = "macos")]
    {
        use winit::os::macos::{BlurMaterial, WindowExt};
        window.set_blur_material(BlurMaterial::UltraDark);
    }

    events_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent {
                event: winit::WindowEvent::CloseRequested,
                ..
            } => winit::ControlFlow::Break,
            winit::Event::WindowEvent {
                event: winit::WindowEvent::KeyboardInput {
                    input: winit::KeyboardInput {
                        virtual_keycode: Some(winit::VirtualKeyCode::Escape),
                        ..
                    },
                    ..
                },
                ..
            } => winit::ControlFlow::Break,
            _ => winit::ControlFlow::Continue,
        }
    });
}
