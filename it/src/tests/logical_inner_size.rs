use crate::backend::{BackendFlags, Instance};
use winit::dpi::LogicalSize;

test!(
    run,
    BackendFlags::WINIT_SET_INNER_SIZE | BackendFlags::SECOND_MONITOR
);

async fn run(instance: &dyn Instance) {
    instance.enable_second_monitor(true);

    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.mapped(true).await;
        window.winit_set_inner_size(LogicalSize {
            width: 300.0,
            height: 300.0,
        });
        window.inner_size(300, 300).await;
        window.winit_set_inner_size(LogicalSize {
            width: 500.0,
            height: 100.0,
        });
        window.inner_size(500, 100).await;
        window.winit_inner_size(500, 100).await;
    }

    {
        let window = el.create_window(Default::default());
        window.mapped(true).await;
        window.set_outer_position(1024, 0);
        assert!(window.winit().scale_factor() > 0.0);
        window.winit_outer_position(1024, 0).await;
        window.winit_set_inner_size(LogicalSize {
            width: 300.0,
            height: 400.0,
        });
        let expected_width = 300.0 * window.winit().scale_factor();
        let expected_height = 400.0 * window.winit().scale_factor();
        loop {
            let props = window.properties();
            if (props.width() as f64 - expected_width).abs() < 2.0
                && (props.height() as f64 - expected_height).abs() < 2.0
            {
                break;
            }
            window.properties_changed().await;
        }
    }
}
