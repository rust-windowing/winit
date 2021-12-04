use crate::backend::{BackendFlags, Instance};
use winit::dpi::PhysicalSize;
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_INNER_SIZE);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.winit_set_inner_size(PhysicalSize {
            width: 300,
            height: 300,
        });
        window.inner_size(300, 300).await;
        window.winit_inner_size(300, 300).await;
        window.winit_set_inner_size(PhysicalSize {
            width: 500,
            height: 100,
        });
        window.inner_size(500, 100).await;
        window.winit_inner_size(500, 100).await;
    }

    {
        let window = el.create_window(WindowBuilder::default().with_inner_size(PhysicalSize {
            width: 600,
            height: 50,
        }));
        window.inner_size(600, 50).await;
        window.winit_inner_size(600, 50).await;
        window.winit_set_inner_size(PhysicalSize {
            width: 200,
            height: 900,
        });
        window.inner_size(200, 900).await;
        window.winit_inner_size(200, 900).await;
    }
}
