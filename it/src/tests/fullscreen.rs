use crate::backend::Instance;
use winit::dpi::PhysicalSize;
use winit::window::{Fullscreen, WindowBuilder};

test!(run);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    let window = el.create_window(WindowBuilder::new().with_inner_size(PhysicalSize {
        width: 500,
        height: 400,
    }));
    window.mapped(true).await;
    window.set_outer_position(100, 100);
    window.outer_position(100, 100).await;
    window.inner_size(500, 400).await;

    let mon = window.winit().current_monitor().unwrap();
    let modes: Vec<_> = mon.video_modes().collect();

    window.winit_set_fullscreen(Some(Fullscreen::Borderless(None)));
    window.outer_position(0, 0).await;
    window.inner_size(mon.size().width, mon.size().height).await;

    window.winit_set_fullscreen(None);
    window.outer_position(100, 100).await;
    window.inner_size(500, 400).await;

    let original_monitor_size = mon.size();

    log::info!("Testing exculsive fullscreen changes");

    for mode in modes.clone() {
        window.winit_set_fullscreen(Some(Fullscreen::Exclusive(mode.clone())));
        window.outer_position(0, 0).await;
        window
            .inner_size(mode.size().width, mode.size().height)
            .await;
        let mon = window.winit().current_monitor().unwrap();
        assert_eq!(mon.size(), mode.size());

        window.winit_set_fullscreen(None);
        window.outer_position(100, 100).await;
        window.inner_size(500, 400).await;
        let mon = window.winit().current_monitor().unwrap();
        assert_eq!(mon.size(), original_monitor_size);
    }

    log::info!("Testing switching between exclusive fullscreen modes");

    for mode in modes.clone() {
        window.winit_set_fullscreen(Some(Fullscreen::Exclusive(mode.clone())));
        window.outer_position(0, 0).await;
        window
            .inner_size(mode.size().width, mode.size().height)
            .await;
        let mon = window.winit().current_monitor().unwrap();
        assert_eq!(mon.size(), mode.size());
    }

    window.winit_set_fullscreen(None);
    window.outer_position(100, 100).await;
    window.inner_size(500, 400).await;
    let mon = window.winit().current_monitor().unwrap();
    assert_eq!(mon.size(), original_monitor_size);
}
