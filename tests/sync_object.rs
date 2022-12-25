#[allow(dead_code)]
fn needs_sync<T: Sync>() {}

#[cfg(not(wasm_platform))]
#[test]
fn window_sync() {
    // ensures that `winit::Window` implements `Sync`
    needs_sync::<winit::window::Window>();
}
