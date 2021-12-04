use crate::backend::{BackendFlags, Instance};
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_TITLE);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.winit_set_title("abc");
        window.title("abc").await;
        window.winit_set_title("def");
        window.title("def").await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_title("ghi"));
        window.title("ghi").await;
        window.winit_set_title("jkl");
        window.title("jkl").await;
    }
}
