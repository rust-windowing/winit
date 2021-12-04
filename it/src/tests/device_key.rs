use crate::backend::{BackendFlags, Instance};
use crate::keyboard::{Key, Layout};
use winit::event::ElementState;
use winit::keyboard::KeyCode;

test!(
    run,
    BackendFlags::DEVICE_ADDED | BackendFlags::DEVICE_REMOVED
);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    let seat = instance.default_seat();

    let kb1 = seat.add_keyboard();

    events.device_added_event().await;

    kb1.press(Key::KeyR);

    let (_, ke) = events.device_key_event().await;
    assert_eq!(ke.physical_key, KeyCode::KeyR);
    assert_eq!(ke.state, ElementState::Pressed);

    let (_, ke) = events.device_key_event().await;
    assert_eq!(ke.physical_key, KeyCode::KeyR);
    assert_eq!(ke.state, ElementState::Released);

    {
        let _r = kb1.press(Key::KeyR);
        kb1.press(Key::KeyL);
    }

    let (_, ke) = events.device_key_event().await;
    assert_eq!(ke.physical_key, KeyCode::KeyR);
    assert_eq!(ke.state, ElementState::Pressed);

    let (_, ke) = events.device_key_event().await;
    assert_eq!(ke.physical_key, KeyCode::KeyL);
    assert_eq!(ke.state, ElementState::Pressed);

    let (_, ke) = events.device_key_event().await;
    assert_eq!(ke.physical_key, KeyCode::KeyL);
    assert_eq!(ke.state, ElementState::Released);

    let (_, ke) = events.device_key_event().await;
    assert_eq!(ke.physical_key, KeyCode::KeyR);
    assert_eq!(ke.state, ElementState::Released);

    seat.set_layout(Layout::Azerty);

    kb1.press(Key::KeyQ);

    let (_, ke) = events.device_key_event().await;
    assert_eq!(ke.physical_key, KeyCode::KeyQ);
    assert_eq!(ke.state, ElementState::Pressed);

    let (_, ke) = events.device_key_event().await;
    assert_eq!(ke.physical_key, KeyCode::KeyQ);
    assert_eq!(ke.state, ElementState::Released);
}
