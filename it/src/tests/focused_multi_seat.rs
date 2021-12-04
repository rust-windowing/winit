use crate::backend::{BackendFlags, Instance};

test!(run, BackendFlags::CREATE_SEAT);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    let seat = instance.default_seat();
    let seat2 = instance.create_seat();

    let window = el.create_window(Default::default());
    window.mapped(true).await;

    seat.focus(&*window);

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window.winit_id());
    assert!(focus);

    seat2.focus(&*window);

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window.winit_id());
    assert!(focus);

    seat.un_focus();

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window.winit_id());
    assert!(!focus);

    seat2.un_focus();

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window.winit_id());
    assert!(!focus);

    seat.focus(&*window);
    seat2.focus(&*window);

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window.winit_id());
    assert!(focus);

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window.winit_id());
    assert!(focus);

    let window2 = el.create_window(Default::default());
    window2.mapped(true).await;

    seat.focus(&*window2);

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window.winit_id());
    assert!(!focus);

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window2.winit_id());
    assert!(focus);

    seat2.focus(&*window2);

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window.winit_id());
    assert!(!focus);

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window2.winit_id());
    assert!(focus);

    seat.un_focus();

    let (we, focus) = events.window_focus_event().await;
    assert_eq!(we.window_id, window2.winit_id());
    assert!(!focus);
}
