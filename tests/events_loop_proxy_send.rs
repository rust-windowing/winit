extern crate winit;

#[test]
fn events_loop_proxy_send() {
    // ensures that `winit::EventsLoopProxy` implements `Send`
    fn needs_send<T:Send>() {}
    needs_send::<winit::EventsLoopProxy>();
}
