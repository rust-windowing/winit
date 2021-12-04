use crate::backend::{BackendFlags, Instance};
use winit::dpi::LogicalSize;

test!(
    run,
    BackendFlags::WINIT_SET_SIZE_BOUNDS | BackendFlags::SECOND_MONITOR
);

async fn run(instance: &dyn Instance) {
    instance.enable_second_monitor(true);

    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.mapped(true).await;
        window.set_outer_position(1024, 0);
        window.outer_position(1024, 0).await;
        let sf = window.winit().scale_factor();
        assert!(sf > 1.0);
        window.winit_set_max_size(Some(LogicalSize {
            width: 300.0,
            height: 400.0,
        }));
        loop {
            if let Some((mw, mh)) = window.properties().max_size() {
                if (mw as f64 - 300.0 * sf).abs() < 2.0 && (mh as f64 - 400.0 * sf).abs() < 2.0 {
                    break;
                }
            }
            window.properties_changed().await;
        }
        window.winit_set_min_size(Some(LogicalSize {
            width: 100.0,
            height: 200.0,
        }));
        loop {
            if let Some((mw, mh)) = window.properties().min_size() {
                if (mw as f64 - 100.0 * sf).abs() < 2.0 && (mh as f64 - 200.0 * sf).abs() < 2.0 {
                    break;
                }
            }
            window.properties_changed().await;
        }
    }
}
