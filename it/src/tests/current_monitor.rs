use crate::backend::{BackendFlags, Instance};

test!(run, BackendFlags::SECOND_MONITOR);

async fn run(instance: &dyn Instance) {
    instance.enable_second_monitor(true);

    let el = instance.create_event_loop();
    let mut events = el.events();

    let window = el.create_window(Default::default());
    window.mapped(true).await;
    window.set_outer_position(100, 100);
    window.winit_outer_position(100, 100).await;

    assert_eq!(
        window.winit().current_monitor().unwrap().scale_factor(),
        1.0
    );
    assert_eq!(
        window.winit().current_monitor().unwrap().scale_factor(),
        window.winit().scale_factor()
    );

    el.barrier().await;

    window.set_outer_position(1024, 100);
    window.winit_outer_position(1024, 100).await;

    assert!(window.winit().current_monitor().unwrap().scale_factor() > 1.0);
    assert_eq!(
        window.winit().current_monitor().unwrap().scale_factor(),
        window.winit().scale_factor()
    );

    let (we, sf) = events.window_scale_factor_changed().await;

    assert_eq!(we.window_id, window.winit_id());
    assert_eq!(sf.scale_factor, window.winit().scale_factor());
    window
        .inner_size(
            sf.new_inner_size.width as u32,
            sf.new_inner_size.height as u32,
        )
        .await;
}
