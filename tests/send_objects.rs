extern crate winit;

fn needs_send<T:Send>() {}

#[test]
fn events_loop_proxy_send() {
    // ensures that `winit::EventsLoopProxy` implements `Send`
    needs_send::<winit::EventsLoopProxy>();
}

#[test]
fn window_send() {
    // ensures that `winit::Window` implements `Send`
    needs_send::<winit::Window>();
}

#[test]
fn ids_send() {
    // ensures that the various `..Id` types implement `Send`
    needs_send::<winit::WindowId>();
    needs_send::<winit::DeviceId>();
    needs_send::<winit::MonitorId>();
}
