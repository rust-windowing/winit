use crate::backend::Instance;

test!(run);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    {
        let window = el.create_window(Default::default());
        window.mapped(true).await;
        window.delete();
        events.window_close_requested().await;
    }
}
