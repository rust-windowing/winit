use crate::backend::{BackendFlags, Instance};
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_VISIBLE);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.mapped(true).await;
        window.winit_set_visible(false);
        window.mapped(false).await;
        window.winit_set_visible(true);
        window.mapped(true).await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_visible(false));
        window.mapped(false).await;
        window.winit_set_visible(true);
        window.mapped(true).await;
    }
}
