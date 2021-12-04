use crate::backend::{Button, Instance};
use winit::dpi::PhysicalSize;
use winit::window::WindowBuilder;

test!(run);

async fn run(instance: &dyn Instance) {
    let seat = instance.default_seat();
    let mouse = seat.add_mouse();

    let el = instance.create_event_loop();
    let mut events = el.events();

    let window = el.create_window(WindowBuilder::new().with_inner_size(PhysicalSize {
        width: 100,
        height: 100,
    }));
    window.mapped(true).await;

    seat.set_cursor_position(window.inner_offset().0 + 5, window.inner_offset().1 + 7);

    let left = mouse.press(Button::Left);
    events.window_mouse_input_event().await;
    window.winit().drag_window().unwrap();
    window.dragging(true).await;
    mouse.move_(10, 20);
    mouse.move_(15, 25);
    drop(left);
    window.dragging(false).await;

    window.outer_position(25, 45).await;
}
