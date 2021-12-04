use crate::backend::{BackendFlags, Instance, NONE_SIZE};
use winit::dpi::PhysicalSize;
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_SIZE_BOUNDS);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.max_size(None).await;
        window.min_size(None).await;
        window.winit_set_max_size(Some(PhysicalSize {
            width: 100,
            height: 200,
        }));
        window.max_size(Some((100, 200))).await;
        window.min_size(None).await;
        window.winit_set_min_size(Some(PhysicalSize {
            width: 300,
            height: 400,
        }));
        window.min_size(Some((300, 400))).await;
        window.max_size(Some((100, 200))).await;
        window.winit_set_max_size(Some(PhysicalSize {
            width: 500,
            height: 600,
        }));
        window.max_size(Some((500, 600))).await;
        window.min_size(Some((300, 400))).await;
        window.winit_set_max_size(NONE_SIZE);
        window.max_size(None).await;
        window.min_size(Some((300, 400))).await;
        window.winit_set_min_size(Some(PhysicalSize {
            width: 700,
            height: 800,
        }));
        window.min_size(Some((700, 800))).await;
        window.max_size(None).await;
        window.winit_set_min_size(NONE_SIZE);
        window.min_size(None).await;
        window.max_size(None).await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_max_inner_size(PhysicalSize {
            width: 900,
            height: 1000,
        }));
        window.max_size(Some((900, 1000))).await;
        window.min_size(None).await;
        window.winit_set_min_size(Some(PhysicalSize {
            width: 1100,
            height: 1200,
        }));
        window.min_size(Some((1100, 1200))).await;
        window.max_size(Some((900, 1000))).await;
        window.winit_set_max_size(Some(PhysicalSize {
            width: 1300,
            height: 1400,
        }));
        window.max_size(Some((1300, 1400))).await;
        window.min_size(Some((1100, 1200))).await;
        window.winit_set_min_size(NONE_SIZE);
        window.winit_set_max_size(NONE_SIZE);
        window.max_size(None).await;
        window.min_size(None).await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_min_inner_size(PhysicalSize {
            width: 1500,
            height: 1600,
        }));
        window.min_size(Some((1500, 1600))).await;
        window.max_size(None).await;
        window.winit_set_max_size(Some(PhysicalSize {
            width: 1700,
            height: 1800,
        }));
        window.max_size(Some((1700, 1800))).await;
        window.min_size(Some((1500, 1600))).await;
        window.winit_set_min_size(NONE_SIZE);
        window.winit_set_max_size(NONE_SIZE);
        window.max_size(None).await;
        window.min_size(None).await;
    }

    {
        let window = el.create_window(
            WindowBuilder::default()
                .with_min_inner_size(PhysicalSize {
                    width: 1900,
                    height: 2000,
                })
                .with_max_inner_size(PhysicalSize {
                    width: 2100,
                    height: 2200,
                }),
        );
        window.min_size(Some((1900, 2000))).await;
        window.max_size(Some((2100, 2200))).await;
        window.winit_set_max_size(Some(PhysicalSize {
            width: 2300,
            height: 2400,
        }));
        window.winit_set_min_size(Some(PhysicalSize {
            width: 2500,
            height: 2600,
        }));
        window.max_size(Some((2300, 2400))).await;
        window.min_size(Some((2500, 2600))).await;
        window.winit_set_min_size(NONE_SIZE);
        window.winit_set_max_size(NONE_SIZE);
        window.max_size(None).await;
        window.min_size(None).await;
    }
}
