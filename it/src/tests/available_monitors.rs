use crate::backend::{BackendFlags, Instance};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::monitor::VideoMode;

test!(run, BackendFlags::SECOND_MONITOR);

async fn run(instance: &dyn Instance) {
    let monitor_names = instance
        .backend()
        .flags()
        .contains(BackendFlags::MONITOR_NAMES);

    let el = instance.create_event_loop();
    el.num_available_monitors(1).await;
    let monitors = el.available_monitors();
    assert_eq!(monitors[0].scale_factor(), 1.0);
    assert_eq!(
        monitors[0].size(),
        PhysicalSize {
            width: 1024,
            height: 768
        }
    );
    assert_eq!(monitors[0].position(), PhysicalPosition { x: 0, y: 0 });
    assert_modes(monitors[0].video_modes());
    if monitor_names {
        assert_eq!(monitors[0].name().as_deref(), Some("output0"));
    }

    instance.enable_second_monitor(true);

    el.num_available_monitors(2).await;
    let monitors = el.available_monitors();
    let (left, right) = if monitors[0].scale_factor() == 1.0 {
        (monitors[0].clone(), monitors[1].clone())
    } else {
        (monitors[1].clone(), monitors[0].clone())
    };
    assert_eq!(left.scale_factor(), 1.0);
    assert!(right.scale_factor() > 1.0);
    assert_eq!(
        left.size(),
        PhysicalSize {
            width: 1024,
            height: 768
        }
    );
    assert_eq!(
        right.size(),
        PhysicalSize {
            width: 800,
            height: 600
        }
    );
    assert_eq!(left.position(), PhysicalPosition { x: 0, y: 0 });
    assert_eq!(right.position(), PhysicalPosition { x: 1024, y: 0 });
    assert_modes(left.video_modes());
    assert_modes(right.video_modes());
    if monitor_names {
        assert_eq!(left.name().as_deref(), Some("output0"));
        assert_eq!(right.name().as_deref(), Some("output1"));
    }

    instance.enable_second_monitor(false);

    el.num_available_monitors(1).await;
    let monitors = el.available_monitors();
    assert_eq!(monitors[0].scale_factor(), 1.0);
    assert_eq!(
        monitors[0].size(),
        PhysicalSize {
            width: 1024,
            height: 768
        }
    );
    assert_eq!(monitors[0].position(), PhysicalPosition { x: 0, y: 0 });
    assert_modes(monitors[0].video_modes());
    if monitor_names {
        assert_eq!(monitors[0].name().as_deref(), Some("output0"));
    }
}

fn assert_modes(modes: impl Iterator<Item = VideoMode>) {
    for mode in modes {
        match mode.size() {
            PhysicalSize {
                width: 1024,
                height: 768,
            } => {
                assert_eq!(mode.refresh_rate(), 60);
                assert_eq!(mode.bit_depth(), 24);
            }
            PhysicalSize {
                width: 800,
                height: 600,
            } => {
                assert_eq!(mode.refresh_rate(), 120);
                assert_eq!(mode.bit_depth(), 24);
            }
            _ => panic!("Unexpected mode: {:?}", mode),
        }
    }
}
