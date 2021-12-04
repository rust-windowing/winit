use crate::backend::{BackendFlags, Instance};

test!(run, BackendFlags::SET_OUTER_POSITION);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    {
        let window = el.create_window(Default::default());
        window.mapped(true).await;
        window.set_outer_position(100, 200);
        let (w, ev) = events.window_move_event().await;
        assert_eq!(w.window_id, window.winit_id());
        assert_eq!((ev.x, ev.y), (100, 200));
        window.winit_outer_position(100, 200).await;
        window.set_outer_position(-300, -400);
        let (w, ev) = events.window_move_event().await;
        assert_eq!(w.window_id, window.winit_id());
        assert_eq!((ev.x, ev.y), (-300, -400));
        window.winit_outer_position(-300, -400).await;
    }
}
