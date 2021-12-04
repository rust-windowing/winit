use crate::backend::{BackendFlags, Instance};
use winit::dpi::PhysicalPosition;
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_OUTER_POSITION);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    {
        let window = el.create_window(Default::default());
        window.winit_set_outer_position(PhysicalPosition { x: 300, y: 300 });
        window.outer_position(300, 300).await;
        window.winit_outer_position(300, 300).await;
        window.winit_set_outer_position(PhysicalPosition { x: 500, y: 250 });
        window.outer_position(500, 250).await;
        window.winit_outer_position(500, 250).await;
    }

    {
        let window = el.create_window(
            WindowBuilder::default().with_position(PhysicalPosition { x: 300, y: 50 }),
        );
        window.outer_position(300, 50).await;
        window.winit_outer_position(300, 50).await;
        window.winit_set_outer_position(PhysicalPosition { x: 550, y: 350 });
        window.outer_position(550, 350).await;
        window.winit_outer_position(550, 350).await;
    }
}
