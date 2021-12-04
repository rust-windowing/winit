use crate::backend::{BackendFlags, Instance};
use crate::sleep::sleep_ms;
use winit::dpi::PhysicalPosition;

test!(run, BackendFlags::WINIT_SET_CURSOR_POSITION);

async fn run(instance: &dyn Instance) {
    let seat = instance.default_seat();
    seat.set_cursor_position(0, 0);

    let el = instance.create_event_loop();

    let window = el.create_window(Default::default());
    window.mapped(true).await;
    window.set_outer_position(100, 100);
    window.outer_position(100, 100).await;
    window.winit_set_cursor_position(PhysicalPosition { x: 20, y: 30 });

    loop {
        let pos = seat.cursor_position();
        if pos == (120 + window.inner_offset().0, 130 + window.inner_offset().1) {
            break;
        }
        log::info!("cursor position = {:?}", pos);
        sleep_ms(10).await;
    }
}
