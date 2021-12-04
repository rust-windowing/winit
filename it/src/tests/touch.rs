use crate::backend::Instance;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::TouchPhase;
use winit::window::WindowBuilder;

test!(run);

async fn run(instance: &dyn Instance) {
    let seat = instance.default_seat();
    let touch = seat.add_touchscreen();

    let el = instance.create_event_loop();
    let mut events = el.events();

    let window = el.create_window(WindowBuilder::new().with_inner_size(PhysicalSize {
        width: 100,
        height: 100,
    }));
    window.mapped(true).await;
    window.inner_size(100, 100).await;
    window.set_outer_position(100 - window.inner_offset().0, 100 - window.inner_offset().1);
    window
        .outer_position(100 - window.inner_offset().0, 100 - window.inner_offset().1)
        .await;

    let f1 = touch.down(110, 110);

    let (_, te1) = events.window_touch_event().await;

    assert!(seat.is(te1.device_id));
    assert_eq!(te1.phase, TouchPhase::Started);
    assert_eq!(te1.location, PhysicalPosition { x: 10.0, y: 10.0 });

    f1.move_(105, 120);

    let (_, te2) = events.window_touch_event().await;

    assert!(seat.is(te2.device_id));
    assert_eq!(te2.id, te1.id);
    assert_eq!(te2.phase, TouchPhase::Moved);
    assert_eq!(te2.location, PhysicalPosition { x: 5.0, y: 20.0 });

    let f2 = touch.down(101, 103);

    let (_, te3) = events.window_touch_event().await;

    assert!(seat.is(te3.device_id));
    assert_eq!(te3.phase, TouchPhase::Started);
    assert_eq!(te3.location, PhysicalPosition { x: 1.0, y: 3.0 });

    f1.move_(107, 135);

    let (_, te4) = events.window_touch_event().await;

    assert!(seat.is(te4.device_id));
    assert_eq!(te4.id, te1.id);
    assert_eq!(te4.phase, TouchPhase::Moved);
    assert_eq!(te4.location, PhysicalPosition { x: 7.0, y: 35.0 });

    f2.move_(106, 107);

    let (_, te5) = events.window_touch_event().await;

    assert!(seat.is(te5.device_id));
    assert_eq!(te5.id, te3.id);
    assert_eq!(te5.phase, TouchPhase::Moved);
    assert_eq!(te5.location, PhysicalPosition { x: 6.0, y: 7.0 });

    drop(f1);

    let (_, te6) = events.window_touch_event().await;

    assert!(seat.is(te6.device_id));
    assert_eq!(te6.id, te1.id);
    assert_eq!(te6.phase, TouchPhase::Ended);
    assert_eq!(te6.location, PhysicalPosition { x: 7.0, y: 35.0 });

    drop(f2);

    let (_, te7) = events.window_touch_event().await;

    assert!(seat.is(te7.device_id));
    assert_eq!(te7.id, te3.id);
    assert_eq!(te7.phase, TouchPhase::Ended);
    assert_eq!(te7.location, PhysicalPosition { x: 6.0, y: 7.0 });
}
