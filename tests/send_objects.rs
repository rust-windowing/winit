#[allow(dead_code)]
fn needs_send<T: Send + ?Sized>() {}

#[test]
fn event_loop_proxy_send() {
    needs_send::<winit::event_loop::EventLoopProxy>();
}

#[test]
fn window_send() {
    needs_send::<dyn winit::window::Window>();
}

#[test]
fn window_builder_send() {
    needs_send::<winit::window::WindowAttributes>();
}

#[test]
fn ids_send() {
    needs_send::<winit::window::WindowId>();
    needs_send::<winit::event::DeviceId>();
    needs_send::<winit::monitor::MonitorHandleProvider>();
}

#[test]
fn custom_cursor_send() {
    needs_send::<winit::window::CustomCursorSource>();
    needs_send::<winit::window::CustomCursor>();
}
