use crate::backend::{BackendFlags, Button, Instance};
use winit::event::ElementState;

test!(
    run,
    BackendFlags::DEVICE_ADDED | BackendFlags::DEVICE_REMOVED
);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    let seat = instance.default_seat();

    let mouse1 = seat.add_mouse();

    let de = events.device_added_event().await;
    assert!(mouse1.id().is(de.device_id));

    mouse1.move_(1, 2);
    mouse1.scroll(3, 4);
    mouse1.press(Button::Left);
    mouse1.press(Button::Right);
    mouse1.press(Button::Middle);
    mouse1.press(Button::Back);
    mouse1.press(Button::Forward);

    let (de, me) = events.device_mouse_motion_event().await;
    assert!(mouse1.id().is(de.device_id));
    let mut delta = me.delta;
    if delta != (1.0, 2.0) {
        let (de, me) = events.device_mouse_motion_event().await;
        assert!(mouse1.id().is(de.device_id));
        delta.0 += me.delta.0;
        delta.1 += me.delta.1;
    }
    assert_eq!(delta, (1.0, 2.0));

    let (de, _) = events.device_mouse_wheel_event().await;
    assert!(mouse1.id().is(de.device_id));

    let (de, db) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert_eq!(db.button, 1);
    assert_eq!(db.state, ElementState::Pressed);

    let (de, db) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert_eq!(db.button, 1);
    assert_eq!(db.state, ElementState::Released);

    let (de, db) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert_eq!(db.button, 2);
    assert_eq!(db.state, ElementState::Pressed);

    let (de, db) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert_eq!(db.button, 2);
    assert_eq!(db.state, ElementState::Released);

    let (de, db) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert_eq!(db.button, 3);
    assert_eq!(db.state, ElementState::Pressed);

    let (de, db) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert_eq!(db.button, 3);
    assert_eq!(db.state, ElementState::Released);

    let (de, edb1) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert!(!matches!(edb1.button, 1 | 2 | 3));
    assert_eq!(edb1.state, ElementState::Pressed);

    let (de, edb2) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert_eq!(edb2.button, edb1.button);
    assert_eq!(edb2.state, ElementState::Released);

    let (de, edb3) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert!(!matches!(edb3.button, 1 | 2 | 3));
    assert_ne!(edb3.button, edb2.button);
    assert_eq!(edb3.state, ElementState::Pressed);

    let (de, edb4) = events.device_button_event().await;
    assert!(mouse1.id().is(de.device_id));
    assert_eq!(edb4.button, edb3.button);
    assert_eq!(edb4.state, ElementState::Released);
}
