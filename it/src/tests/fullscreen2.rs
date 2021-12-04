use crate::backend::{BackendFlags, Instance};
use winit::dpi::PhysicalSize;
use winit::window::{Fullscreen, WindowBuilder};

test!(run, BackendFlags::SECOND_MONITOR);

async fn run(instance: &dyn Instance) {
    instance.enable_second_monitor(true);

    let el = instance.create_event_loop();
    let mut events = el.events();

    let window = el.create_window(WindowBuilder::new().with_inner_size(PhysicalSize {
        width: 500,
        height: 400,
    }));
    window.mapped(true).await;
    window.set_outer_position(100, 100);
    window.outer_position(100, 100).await;
    window.inner_size(500, 400).await;

    let mon = el.primary_monitor().unwrap();
    let other_mon = el
        .available_monitors()
        .iter()
        .find(|m| m.position() != mon.position())
        .unwrap()
        .clone();

    log::info!("Testing borderless fs on primary monitor");

    el.barrier().await;
    window.winit_set_fullscreen(Some(Fullscreen::Borderless(Some(mon.clone()))));
    window.outer_position(1024, 0).await;
    window.inner_size(mon.size().width, mon.size().height).await;
    events.window_scale_factor_changed().await;
    events.window_resize_event().await;

    log::info!("Testing borderless fs on secondary monitor");

    el.barrier().await;
    window.winit_set_fullscreen(Some(Fullscreen::Borderless(Some(other_mon.clone()))));
    window.outer_position(0, 0).await;
    window
        .inner_size(other_mon.size().width, other_mon.size().height)
        .await;
    events.window_scale_factor_changed().await;

    window.winit_set_fullscreen(None);
    window.outer_position(100, 100).await;
    window.inner_size(500, 400).await;
}
