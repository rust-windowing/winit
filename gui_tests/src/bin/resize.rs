//! This test was designed to reproduce a bug that caused a panic on Windows with winit 0.21.0

use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use scopeguard::defer;
use std::collections::VecDeque;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();
    let el_proxy = event_loop.create_proxy();
    let (sender, receiver) = channel::<Event<'static, ()>>();
    // Just as in general winit terminology, `outer_pos` refers to the position of the top-left
    // corner of the window decorations. Whereas `inner_pos` refers to the area of the window
    // that's drawn by the application itself.
    let outer_pos = PhysicalPosition::<i32>::new(200, 200);
    let inner_size = PhysicalSize::new(512, 512);
    let create_enigo_thread = move |inner_pos_x: i32, inner_pos_y: i32| {
        thread::spawn(move || {
            // Defer shutdown request.
            defer!(el_proxy.send_event(()).unwrap());
            let pause_time = Duration::from_millis(250);
            let mut enigo = Enigo::new();
            enigo.mouse_move_to(
                inner_pos_x + inner_size.width + 2,
                inner_pos_y + inner_size.height + 2,
            );
            thread::sleep(pause_time);
            enigo.mouse_down(MouseButton::Left);
            let mut width_offset = 0;
            let mut height_offset = 0;
            for i in 0..50 {
                thread::sleep(Duration::from_millis(1));
                width_offset = i * 2;
                height_offset = i;
                enigo.mouse_move_to(
                    inner_pos_x + inner_size.width + 2 + width_offset,
                    inner_pos_y + inner_size.height + 2 + height_offset,
                );
            }
            thread::sleep(Duration::from_millis(1));
            enigo.mouse_up(MouseButton::Left);
            thread::sleep(pause_time);
            enigo.mouse_move_to(inner_pos_x + inner_size.width / 2, inner_pos_y - 2);
            enigo.mouse_down(MouseButton::Left);
            for i in 0..50 {
                thread::sleep(Duration::from_millis(1));
                enigo.mouse_move_to(inner_pos_x + inner_size.width / 2 + i, inner_pos_y - 2 + i);
            }
            thread::sleep(Duration::from_millis(1));
            enigo.mouse_move_to(inner_pos_x + inner_size.width / 2, inner_pos_y - 2);
            enigo.mouse_up(MouseButton::Left);
            thread::sleep(pause_time);

            let mut relevant_events = VecDeque::new();
            while let Ok(event) = receiver.try_recv() {
                match event {
                    event
                    @
                    Event::WindowEvent {
                        event: WindowEvent::Resized(_),
                        ..
                    } => relevant_events.push_back(event),
                    _ => {} // ignore the rest
                }
            }
            let new_width = inner_size.width + width_offset;
            let new_height = inner_size.height + height_offset;
            match relevant_events.pop_back() {
                Some(Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                }) if size.width as i32 == new_width && size.height as i32 == new_height => {}
                _ => panic!("Unexpected size at the end."),
            }
        })
    };

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(inner_size)
        .with_resizable(true)
        .build(&event_loop)
        .unwrap();

    // For some reason the window doesn't have focus when it's ran from the main module (gui_tests)
    // so we are switching to fullscreen and back just to bring the window to the top
    window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(
        window.current_monitor(),
    )));
    thread::sleep(Duration::from_millis(500));
    window.set_fullscreen(None);
    thread::sleep(Duration::from_millis(500));

    window.set_outer_position(outer_pos);
    let inner_pos = window.inner_position().unwrap();
    let mut enigo_handle = Some(create_enigo_thread(inner_pos.x as i32, inner_pos.y as i32));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
            } if window_id == window.id() => *control_flow = ControlFlow::Exit,
            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => *control_flow = ControlFlow::Exit,
            Event::UserEvent(_) => {
                // Shutdown request.
                *control_flow = ControlFlow::Exit;
            }
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            _ => (),
        }
        if enigo_handle.is_some() {
            if let Some(event) = event.to_static() {
                sender.send(event).unwrap();
            }
        }
        if *control_flow == ControlFlow::Exit {
            enigo_handle.take().unwrap().join().unwrap();
        }
    });
}
