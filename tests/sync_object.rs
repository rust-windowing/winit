extern crate winit;

fn needs_sync<T:Sync>() {}

#[test]
fn window_sync() {
    // ensures that `winit::Window` implements `Sync`
    needs_sync::<winit::Window>();
}
