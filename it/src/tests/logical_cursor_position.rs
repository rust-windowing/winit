use crate::backend::{BackendFlags, Instance};
use crate::sleep::sleep_ms;
use winit::dpi::LogicalPosition;

test!(
    run,
    BackendFlags::WINIT_SET_CURSOR_POSITION | BackendFlags::SECOND_MONITOR
);

async fn run(instance: &dyn Instance) {
    instance.enable_second_monitor(true);

    let seat = instance.default_seat();
    seat.set_cursor_position(0, 0);

    let el = instance.create_event_loop();

    let window = el.create_window(Default::default());
    window.mapped(true).await;

    window.set_outer_position(100, 100);
    window.outer_position(100, 100).await;
    window.winit_set_cursor_position(LogicalPosition { x: 20.0, y: 30.0 });

    loop {
        if seat.cursor_position() == (120 + window.inner_offset().0, 130 + window.inner_offset().1)
        {
            break;
        }
        sleep_ms(10).await;
    }

    window.set_outer_position(1024, 100);
    window.outer_position(1024, 100).await;
    assert!(window.winit().scale_factor() > 0.0);
    window.winit_set_cursor_position(LogicalPosition { x: 20.0, y: 30.0 });

    let expected_x = 1024.0 + window.inner_offset().0 as f64 + 20.0 * window.winit().scale_factor();
    let expected_y = 100.0 + window.inner_offset().1 as f64 + 30.0 * window.winit().scale_factor();
    loop {
        let (x, y) = seat.cursor_position();
        if (x as f64 - expected_x) < 2.0 && (y as f64 - expected_y) < 2.0 {
            break;
        }
        sleep_ms(10).await;
    }
}
