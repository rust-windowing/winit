#![allow(clippy::single_match)]

// This example is used by developers to test various window functions.

use simple_logger::SimpleLogger;
use winit::{
    dpi::{LogicalPosition, LogicalSize},
    event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{DeviceEventFilter, EventLoop},
    window::WindowBuilder,
};

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();
    event_loop.set_device_event_filter(DeviceEventFilter::Never);

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(LogicalSize::new(200.0, 200.0))
        .build(&event_loop)
        .unwrap();

    event_loop.set_device_event_filter(DeviceEventFilter::Never);

    eprintln!(
        r#"
Steps to reproduce:
    1. Focus window and hit (X) to maximize the window
    2. Focus window and hit (M) to minimize the maximized window
    3. Hit (M) to restore the minimied window to a maximized window
            => Notice that the restored window is not actually maximized,
               it has the size of a maximized window but missing `WS_MAXIMIZE` style
    4. Hit (X) to restore the maximized window to a normal window.
            => Notice that nothing changes because the window has been restored
               to a different position and size than the previous position and the size
               before maximizing.
    5. Hit (D) to restore the window to 200x200
    6. Focus window and hit (X) to maximize the window
    7. Focus window and hit (M) to minimize the maximized window
    8. Hit (V) to activate a magical flag that always sets `WS_MAXIMIZE` when `WindowFlags::apply_diff` is called.
    9. Hit (M) to restore the minimied window to a maximized window
            => Notice that the restored window is maximized correctly.
    10. Hit (V) to disable the magical flag that always sets `WS_MAXIMIZE` when `WindowFlags::apply_diff` is called.
    11. Hit (X) to restore the maximized window to a normal window.
            => Notice that the window has been restored to the previous size and position correctly.
    "#
    );

    let mut minimized = false;

    event_loop.run(move |event, _, control_flow| {
        control_flow.set_wait();

        match event {
            Event::DeviceEvent {
                event:
                    DeviceEvent::Key(KeyboardInput {
                        virtual_keycode: Some(key),
                        state: ElementState::Pressed,
                        ..
                    }),
                ..
            } => match key {
                VirtualKeyCode::M => {
                    if minimized {
                        minimized = !minimized;
                        window.set_minimized(minimized);
                        window.focus_window();
                    }
                }
                VirtualKeyCode::V => {
                    window.toggle_magic_flag();
                }
                _ => (),
            },
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(key),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => match key {
                VirtualKeyCode::M => {
                    minimized = !minimized;
                    window.set_minimized(minimized);
                }

                VirtualKeyCode::X => {
                    let is_maximized = window.is_maximized();
                    window.set_maximized(!is_maximized);
                }

                VirtualKeyCode::D => {
                    window.set_inner_size::<LogicalSize<u32>>((200, 200).into());
                    window.set_outer_position::<LogicalPosition<u32>>((200, 200).into());
                }
                _ => (),
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => control_flow.set_exit(),
            _ => (),
        }
    });
}
