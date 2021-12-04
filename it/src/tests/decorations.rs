use crate::backend::{BackendFlags, Instance};
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_DECORATIONS);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.decorations(true).await;
        window.winit_set_decorations(false);
        window.decorations(false).await;
        window.winit_set_decorations(true);
        window.decorations(true).await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_decorations(false));
        window.decorations(false).await;
        window.winit_set_decorations(true);
        window.decorations(true).await;
    }
}
