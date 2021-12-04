use crate::backend::{BackendFlags, Instance};
use winit::platform::unix::WindowBuilderExtUnix;
use winit::window::WindowBuilder;

test!(run, BackendFlags::X11);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.class("winit-it").await;
        window.instance("winit-it").await;
    }

    {
        let window =
            el.create_window(WindowBuilder::default().with_class("a".to_string(), "b".to_string()));
        window.class("b").await;
        window.instance("a").await;
    }
}
