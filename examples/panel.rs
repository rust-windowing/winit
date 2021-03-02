#[cfg(all(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ),
    feature = "x11"
))]
use winit::{
    dpi::{PhysicalPosition, PhysicalSize, Position},
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    platform::unix::{WindowBuilderExtUnix, XWindowStrut, XWindowType},
    window::{Window, WindowBuilder},
};

#[cfg(all(
    any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ),
    feature = "x11"
))]
fn main() {
    let event_loop = EventLoop::new();

    // Template Window
    let window = |wtype, on_top, with_strut: Option<Vec<XWindowStrut>>| {
        let mut win_builder = WindowBuilder::new()
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top(on_top)
            .with_x11_window_type(wtype);

        if let Some(with_strut) = with_strut {
            win_builder = win_builder.with_x11_window_strut(with_strut);
        }

        win_builder.build(&event_loop).unwrap()
    };

    let desktop = window(vec![XWindowType::Desktop], false, None);
    let primary_monitor = desktop.primary_monitor().unwrap();
    desktop.set_inner_size(primary_monitor.size());
    desktop.set_outer_position(primary_monitor.position());

    let toolbar_height: u32 = 30;
    let toolbar = window(
        vec![XWindowType::Toolbar],
        true,
        Some(vec![XWindowStrut::Strut([0, 0, toolbar_height as u64, 0])]),
    );
    toolbar.set_inner_size(PhysicalSize::new(
        primary_monitor.size().width,
        toolbar_height,
    ));
    toolbar.set_outer_position(Position::Physical(primary_monitor.position()));

    let dock_height: u32 = 50;
    let dock = window(
        vec![XWindowType::Dock],
        true,
        Some(vec![XWindowStrut::Strut([0, 0, 0, dock_height as u64])]),
    );
    dock.set_inner_size(PhysicalSize::new(primary_monitor.size().width, dock_height));
    let bottom_at = primary_monitor.size().height - dock_height;
    dock.set_outer_position(Position::Physical(PhysicalPosition::new(
        0,
        bottom_at as i32,
    )));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::MainEventsCleared => {
                desktop.request_redraw();
                toolbar.request_redraw();
                dock.request_redraw();
            }
            Event::RedrawRequested(_) => {
                // Redraw the application.
                //
                // It's preferable for applications that do not render continuously to render in
                // this event rather than in MainEventsCleared, since rendering in here allows
                // the program to gracefully handle redraws requested by the OS.
            }
            _ => (),
        }
    });
}

#[cfg(all(
    not(target_os = "linux"),
    not(target_os = "dragonfly"),
    not(target_os = "freebsd"),
    not(target_os = "netbsd"),
    not(target_os = "openbsd"),
    not(feature = "x11")
))]
fn main() {
    eprintln!("This example is only supported on Unix X11.")
}
