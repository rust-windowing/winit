use crate::backend::{BackendFlags, Instance};
use winit::dpi::PhysicalSize;
use winit::window::WindowBuilder;

test!(run, BackendFlags::X11);

async fn run(instance: &dyn Instance) {
    let seat = instance.create_seat();
    let pointer = seat.add_mouse();

    let el = instance.create_event_loop();
    let mut events = el.events();

    let window = el.create_window(WindowBuilder::new().with_inner_size(PhysicalSize {
        width: 100,
        height: 100,
    }));
    window.mapped(true).await;
    seat.set_cursor_position(50, 50);

    events.window_cursor_entered().await;

    window.winit_set_cursor_grab(true);
    instance.cursor_grabbed(true).await;

    pointer.move_(1, 1);
    events.window_cursor_moved().await;

    window.winit_set_cursor_grab(false);
    instance.cursor_grabbed(false).await;
}
