use crate::backend::{BackendFlags, Instance};

test!(run, BackendFlags::SET_INNER_SIZE);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    {
        let window = el.create_window(Default::default());
        window.mapped(true).await;
        window.set_inner_size(100, 200);
        let (w, ev) = events.window_resize_event().await;
        assert_eq!(w.window_id, window.winit_id());
        assert_eq!((ev.width, ev.height), (100, 200));
        window.winit_inner_size(100, 200).await;
        window.set_inner_size(300, 400);
        let (w, ev) = events.window_resize_event().await;
        assert_eq!(w.window_id, window.winit_id());
        assert_eq!((ev.width, ev.height), (300, 400));
        window.winit_inner_size(300, 400).await;
    }
}
