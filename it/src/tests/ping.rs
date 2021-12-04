use crate::backend::{BackendFlags, Instance};

test!(run, BackendFlags::X11);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.ping().await;
    }
}
