#[allow(dead_code)]
fn needs_sync<T: Sync>() {}

#[cfg(not(wasm_platform))]
#[test]
fn event_loop_proxy_send() {
    #[allow(dead_code)]
    fn is_send<T: 'static + Send>() {
        // ensures that `winit::EventLoopProxy` implements `Sync`
        needs_sync::<winit::event_loop::EventLoopProxy<T>>();
    }
}

#[cfg(not(wasm_platform))]
#[test]
fn window_sync() {
    // ensures that `winit::Window` implements `Sync`
    needs_sync::<winit::window::Window>();
}
