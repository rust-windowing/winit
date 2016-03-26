extern crate winit;

#[cfg(feature = "window")]
#[test]
fn window_proxy_send() {
    // ensures that `winit::WindowProxy` implements `Send`
    fn needs_send<T:Send>() {}
    needs_send::<winit::WindowProxy>();
}
