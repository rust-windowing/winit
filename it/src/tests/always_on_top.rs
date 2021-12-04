use crate::backend::{BackendFlags, Instance};
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_ALWAYS_ON_TOP);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.always_on_top(false).await;
        window.winit_set_always_on_top(true);
        window.always_on_top(true).await;
        window.winit_set_always_on_top(false);
        window.always_on_top(false).await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_always_on_top(true));
        window.always_on_top(true).await;
        window.winit_set_always_on_top(false);
        window.always_on_top(false).await;
    }
}
