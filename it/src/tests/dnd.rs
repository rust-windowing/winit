use crate::backend::Instance;
use winit::dpi::PhysicalSize;
use winit::window::WindowBuilder;

test!(run);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();

    let window = el.create_window(WindowBuilder::new().with_inner_size(PhysicalSize {
        width: 100,
        height: 100,
    }));
    window.mapped(true).await;
    window.set_outer_position(-window.inner_offset().0, -window.inner_offset().1);
    window
        .outer_position(-window.inner_offset().0, -window.inner_offset().1)
        .await;

    let path = instance.create_dnd_path("test.txt");

    {
        let process = instance.start_dnd_process(&path);
        process.drag_to(50, 50);

        let (_, hf) = events.window_hovered_file().await;
        assert_eq!(hf, path);

        process.do_drop();

        let (_, hf) = events.window_dropped_file().await;
        assert_eq!(hf, path);
    }

    {
        el.barrier().await;

        let process = instance.start_dnd_process(&path);
        process.drag_to(50, 50);

        let (_, hf) = events.window_hovered_file().await;
        assert_eq!(hf, path);

        process.drag_to(500, 500);

        events.window_hovered_file_canceled().await;
    }

    {
        el.barrier().await;

        let process = instance.start_dnd_process(&path);
        process.drag_to(50, 50);

        let (_, hf) = events.window_hovered_file().await;
        assert_eq!(hf, path);

        drop(process);

        events.window_hovered_file_canceled().await;
    }
}
