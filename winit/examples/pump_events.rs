#![allow(clippy::single_match)]

use std::any::Any;
use std::time::Duration;
use winit::event_loop::EventLoop;
use winit_core::application::ApplicationHandler;
use winit_core::event_loop::pump_events::{EventLoopExtPumpEvents, PumpStatus};

// Limit this example to only compatible platforms.
#[cfg(any(
    windows_platform,
    macos_platform,
    x11_platform,
    wayland_platform,
    android_platform,
    orbital_platform,
))]
fn main() -> std::process::ExitCode {
    use std::process::ExitCode;
    use std::thread::sleep;
    use std::time::Duration;

    use softbuffer::{Context, Surface};
    use tracing::info;
    use winit::application::ApplicationHandler;
    use winit::event::WindowEvent;
    use winit::event_loop::pump_events::{PumpStatus};
    use winit::event_loop::{ActiveEventLoop, EventLoop, OwnedDisplayHandle};
    use winit::window::{Window, WindowAttributes, WindowId};

    #[path = "util/fill.rs"]
    mod fill;
    #[path = "util/tracing.rs"]
    mod tracing;

    #[derive(Default, Debug)]
    struct PumpDemo {
        surface: Option<Surface<OwnedDisplayHandle, Box<dyn Window>>>,
    }

    impl ApplicationHandler for PumpDemo {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            let window_attributes = WindowAttributes::default().with_title("A fantastic window!");
            let window = event_loop.create_window(window_attributes).unwrap();

            let context = Context::new(event_loop.owned_display_handle()).unwrap();
            self.surface = Some(Surface::new(&context, window).unwrap());
        }

        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            info!("{event:?}");

            let surface = match self.surface.as_mut() {
                Some(surface) => surface,
                None => return,
            };

            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::RedrawRequested => {
                    fill::fill(surface);
                    surface.window().request_redraw();
                },
                _ => (),
            }
        }
    }

    tracing::init();

    let mut event_loop = EventLoop::new().unwrap();

    let mut app = PumpDemo::default();

    loop {
        let timeout = Some(Duration::ZERO);
        let status = pump_events(&mut event_loop, &mut app, timeout);

        if let PumpStatus::Exit(exit_code) = status {
            break ExitCode::from(exit_code as u8);
        }

        // Sleep for 1/60 second to simulate application work
        //
        // Since `pump_events` doesn't block it will be important to
        // throttle the loop in the app somehow.
        info!("Update()");
        sleep(Duration::from_millis(16));
    }
}

fn pump_events(
    event_loop: &mut EventLoop,
    app: &mut dyn ApplicationHandler,
    timeout: Option<Duration>,
) -> PumpStatus {
    let event_loop = event_loop.raw_event_loop_mut() as &mut dyn Any;

    #[cfg(windows_platform)]
    if let Some(event_loop) = event_loop.downcast_mut::<winit_win32::EventLoop>() {
        return event_loop.pump_app_events(timeout, app);
    }

    #[cfg(macos_platform)]
    if let Some(event_loop) = event_loop.downcast_mut::<winit_appkit::EventLoop>() {
        return event_loop.pump_app_events(timeout, app);
    }

    #[cfg(any(x11_platform, wayland_platform))]
    if let Some(event_loop) = event_loop.downcast_mut::<winit::platform_impl::linux::EventLoop>() {
        return event_loop.pump_app_events(timeout, app);
    }

    #[cfg(android_platform)]
    if let Some(event_loop) = event_loop.downcast_mut::<winit_android::EventLoop>() {
        return event_loop.pump_app_events(timeout, app);
    }

    #[cfg(orbital_platform)]
    if let Some(event_loop) = event_loop.downcast_mut::<winit_orbital:EventLoop>() {
        return event_loop.pump_app_events(timeout, app);
    }

    unreachable!("Not supported by backend");
}

#[cfg(any(ios_platform, web_platform))]
fn main() {
    panic!("This platform doesn't support pump_events.")
}
