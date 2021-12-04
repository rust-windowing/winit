use crate::backend::Instance;

test!(run);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    let window = el.create_window(Default::default());
    window.mapped(true).await;

    for i in 0..instance.redraw_requested_scenarios() {
        el.barrier().await;
        window.request_redraw(i);
        let id = events.redraw_requested_event().await;
        assert_eq!(id, window.winit_id());
    }
}
