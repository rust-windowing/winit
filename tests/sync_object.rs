#[allow(dead_code)]
fn needs_sync<T: Sync>() {}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn window_sync() {
    // ensures that `winit::Window` implements `Sync`
    needs_sync::<winit::window::Window>();
}
