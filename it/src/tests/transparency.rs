use crate::backend::{BackendFlags, Instance};
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_TRANSPARENCY);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(WindowBuilder::default().with_transparent(true));
        assert!(window.properties().supports_transparency());
    }
}
