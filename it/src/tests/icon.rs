use crate::backend::{BackendFlags, BackendIcon, Instance};
use winit::window::WindowBuilder;

test!(run, BackendFlags::WINIT_SET_ICON);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();

    let icon1 = BackendIcon {
        rgba: vec![1, 2, 3, 4],
        width: 1,
        height: 1,
    };

    let icon2 = BackendIcon {
        rgba: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        width: 2,
        height: 2,
    };

    {
        let window = el.create_window(Default::default());
        window.icon(None).await;
        window.winit_set_window_icon(Some(icon1.clone().into()));
        window.icon(Some(&icon1)).await;
        window.winit_set_window_icon(Some(icon2.clone().into()));
        window.icon(Some(&icon2)).await;
        window.winit_set_window_icon(None);
        window.icon(None).await;
    }

    {
        let window =
            el.create_window(WindowBuilder::default().with_window_icon(Some(icon1.clone().into())));
        window.icon(Some(&icon1)).await;
        window.winit_set_window_icon(Some(icon2.clone().into()));
        window.icon(Some(&icon2)).await;
        window.winit_set_window_icon(None);
        window.icon(None).await;
    }
}
