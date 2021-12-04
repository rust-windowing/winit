use crate::backend::{BackendFlags, Instance};
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_RESIZABLE);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.resizable(true).await;
        window.winit_set_resizable(false);
        window.resizable(false).await;
        window.winit_set_resizable(true);
        window.resizable(true).await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_resizable(false));
        window.resizable(false).await;
        window.winit_set_resizable(true);
        window.resizable(true).await;
    }
}
