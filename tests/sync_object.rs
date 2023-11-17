#[allow(dead_code)]
fn needs_sync<T: Sync>() {}

#[test]
fn window_sync() {
    // ensures that `winit::Window` implements `Sync`
    needs_sync::<winit::window::Window>();
}

#[test]
fn window_builder_sync() {
    needs_sync::<winit::window::WindowBuilder>();
}

#[test]
fn custom_cursor_sync() {
    needs_sync::<winit::cursor::CustomCursor>();
}
