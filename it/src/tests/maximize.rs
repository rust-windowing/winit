use crate::backend::{BackendFlags, Instance};
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_MAXIMIZED);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.minimized(false).await;
        window.winit_set_maximized(true);
        window.maximized(true).await;
        window.winit_set_maximized(false);
        window.maximized(false).await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_maximized(true));
        window.maximized(true).await;
        window.winit_set_maximized(false);
        window.maximized(false).await;
    }
}
