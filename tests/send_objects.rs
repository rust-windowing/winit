#[allow(dead_code)]
fn needs_send<T: Send>() {}

#[test]
fn event_loop_proxy_send() {
    #[allow(dead_code)]
    fn is_send<T: 'static + Send>() {
        // ensures that `winit::EventLoopProxy` implements `Send`
        needs_send::<winit::event_loop::EventLoopProxy<T>>();
    }
}

#[test]
fn window_send() {
    // ensures that `winit::Window` implements `Send`
    needs_send::<winit::window::Window>();
}

#[test]
fn window_builder_send() {
    needs_send::<winit::window::WindowBuilder>();
}

#[test]
fn ids_send() {
    // ensures that the various `..Id` types implement `Send`
    needs_send::<winit::window::WindowId>();
    needs_send::<winit::event::DeviceId>();
    needs_send::<winit::monitor::MonitorHandle>();
}

#[test]
fn custom_cursor_send() {
    needs_send::<winit::window::CustomCursor>();
}
