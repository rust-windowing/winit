use crate::backend::Instance;
use crate::event::UserEvent;

test!(run);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    el.send_event(UserEvent(1));
    assert_eq!(events.user_event().await, UserEvent(1));

    el.send_event(UserEvent(2));
    el.send_event(UserEvent(3));
    assert_eq!(events.user_event().await, UserEvent(2));
    assert_eq!(events.user_event().await, UserEvent(3));
}
