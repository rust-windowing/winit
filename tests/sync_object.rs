#[allow(dead_code)]
fn needs_sync<T: Sync + ?Sized>() {}

#[test]
fn event_loop_proxy_send() {
    needs_sync::<winit::event_loop::EventLoopProxy>();
}

#[test]
fn window_sync() {
    needs_sync::<dyn winit::window::Window>();
}

#[test]
fn window_builder_sync() {
    needs_sync::<winit::window::WindowAttributes>();
}

#[test]
fn custom_cursor_sync() {
    needs_sync::<winit::window::CustomCursorSource>();
    needs_sync::<winit::window::CustomCursor>();
}
