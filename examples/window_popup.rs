#[cfg(any(x11_platform, windows_platform))]
#[path = "util/fill.rs"]
mod fill;

#[cfg(any(x11_platform, windows_platform))]
fn main() {
    use winit::{
        dpi::{LogicalPosition, LogicalSize, Position},
        event::{Event, WindowEvent},
        event_loop::EventLoop,
        platform::popup::WindowBuilderExtPopup,
        window::WindowBuilder,
    };

    let event_loop: EventLoop<()> = EventLoop::new();
    let mut parent_window = Some(
        WindowBuilder::new()
            .with_title("parent window")
            .with_position(Position::Logical(LogicalPosition::new(0.0, 0.0)))
            .with_inner_size(LogicalSize::new(640.0f32, 480.0f32))
            .build(&event_loop)
            .unwrap(),
    );

    println!("parent window: {parent_window:?})");

    let monitor_size = event_loop.primary_monitor().unwrap().size();
    let child_posn = LogicalPosition::new(
        (monitor_size.width as f64 - 200.0) / 2.0,
        (monitor_size.height as f64 - 200.0) / 2.0,
    );
    let mut child_window = Some(
        WindowBuilder::new()
            .with_title("popup window")
            .with_inner_size(LogicalSize::new(200.0f32, 200.0f32))
            .with_position(Position::Logical(child_posn))
            .with_transient_parent(parent_window.as_ref().unwrap())
            .build(&event_loop)
            .unwrap(),
    );

    event_loop.run(move |event: Event<'_, ()>, _, control_flow| {
        control_flow.set_wait();

        if let Event::WindowEvent { event, window_id } = event {
            match event {
                WindowEvent::CloseRequested
                    if Some(window_id) == parent_window.as_ref().map(|w| w.id()) =>
                {
                    parent_window.take();
                    control_flow.set_exit();
                }
                WindowEvent::CloseRequested
                    if Some(window_id) == child_window.as_ref().map(|w| w.id()) =>
                {
                    child_window.take();
                }
                _ => (),
            }
        } else if let Event::RedrawRequested(wid) = event {
            if Some(wid) == parent_window.as_ref().map(|w| w.id()) {
                fill::fill_window(parent_window.as_ref().unwrap());
            } else if Some(wid) == child_window.as_ref().map(|w| w.id()) {
                fill::fill_window(child_window.as_ref().unwrap());
            }
        }
    })
}

#[cfg(not(any(x11_platform, windows_platform)))]
fn main() {
    panic!("This example is supported only on x11 and Windows.");
}
