extern crate glutin;

#[cfg(feature = "window")]
#[test]
fn window_proxy_send() {
    // ensures that `glutin::WindowProxy` implements `Send`
    fn needs_send<T:Send>() {}
    needs_send::<glutin::WindowProxy>();
}
