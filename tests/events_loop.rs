extern crate winit;

// A part of the API requirement for `EventsLoop` is that it is `Send` + `Sync`.
//
// This short test will only compile if the `EventsLoop` is `Send` + `Sync`. 
#[test]
fn send_sync() {
    fn check_send_sync<T: Send + Sync>() {}
    check_send_sync::<winit::EventsLoop>();
}
