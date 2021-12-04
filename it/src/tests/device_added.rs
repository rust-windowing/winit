use crate::backend::{BackendFlags, Instance};

test!(
    run,
    BackendFlags::DEVICE_ADDED | BackendFlags::DEVICE_REMOVED
);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    let seat = instance.default_seat();

    let kb1 = seat.add_keyboard();
    let kb2 = seat.add_keyboard();

    let kb1_id = kb1.id();
    let kb2_id = kb2.id();

    let dev1 = events.device_added_event().await;
    let dev2 = events.device_added_event().await;

    assert!(kb1_id.is(dev1.device_id));
    assert!(kb2_id.is(dev2.device_id));

    drop(kb2);
    let dev2 = events.device_removed_event().await;
    assert!(kb2_id.is(dev2.device_id));

    drop(kb1);
    let dev1 = events.device_removed_event().await;
    assert!(kb1_id.is(dev1.device_id));
}
