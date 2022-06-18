#[allow(dead_code)]
fn needs_sync<T: Sync>() {}

#[test]
fn event_loop_proxy_sync() {
    #[allow(dead_code)]
    fn is_send<T: 'static + Send>() {
        // ensures that `winit::EventLoopProxy` implements `Sync`
        needs_sync::<winit::event_loop::EventLoopProxy<T>>();
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn window_sync() {
    // ensures that `winit::Window` implements `Sync`
    needs_sync::<winit::window::Window>();
}
