use crate::backend::{BackendFlags, Instance};
use winit::dpi::PhysicalSize;
use winit::window::{CursorIcon, WindowBuilder};

test!(run, BackendFlags::MANUAL_VERIFICATION);

async fn run(instance: &dyn Instance) {
    let seat = instance.default_seat();

    let el = instance.create_event_loop();
    let mut events = el.events();

    let window = el.create_window(WindowBuilder::new().with_inner_size(PhysicalSize {
        width: 100,
        height: 100,
    }));
    window.mapped(true).await;
    window.set_background_color(100, 100, 150);
    window.set_outer_position(100, 100);
    window.outer_position(100, 100).await;
    el.barrier();
    seat.set_cursor_position(window.inner_offset().0 + 150, window.inner_offset().1 + 150);
    events.window_cursor_moved().await;

    window.winit_set_cursor_icon(CursorIcon::Hand);
    log::info!("Verify that the screenshot displays a 'Hand' cursor.");
    instance.take_screenshot();

    window.winit_set_cursor_icon(CursorIcon::Arrow);
    log::info!("Verify that the screenshot displays a 'Arrow' cursor.");
    instance.take_screenshot();
}
