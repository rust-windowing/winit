use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::{
    atomic::{AtomicIsize, Ordering},
    mpsc::channel,
};
use std::thread;
use std::time::Duration;

use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let (sender, receiver) = channel::<Event<'static, ()>>();
    let window_pos = PhysicalPosition::<i32>::new(200, 200);
    let create_enigo_thread = move |client_pos_x: i32, client_pos_y: i32| {
        thread::spawn(move || {
            let cursor_x = 20;
            let cursor_y = 20;
            let pause_time = Duration::from_millis(500);
            let mut enigo = Enigo::new();
            enigo.mouse_move_to(client_pos_x - 1, client_pos_y - 1);
            thread::sleep(pause_time);
            enigo.mouse_move_to(client_pos_x + cursor_x + 1, client_pos_y + cursor_y + 1);
            thread::sleep(pause_time);
            enigo.key_click(Key::Escape);
            thread::sleep(pause_time);

            let mut relevant_events = VecDeque::new();
            while let Ok(event) = receiver.recv_timeout(Duration::from_millis(200)) {
                match event {
                    event @ Event::WindowEvent { .. } => relevant_events.push_back(event),
                    _ => {} // ignore the rest
                }
            }
            match relevant_events.pop_front() {
                Some(Event::WindowEvent {
                    event: WindowEvent::Moved(pos),
                    ..
                }) if pos.x as i32 == window_pos.x && pos.y as i32 == window_pos.y => {}
                _ => unreachable!(),
            }
            match relevant_events.pop_front() {
                Some(Event::WindowEvent {
                    event: WindowEvent::CursorEntered { .. },
                    ..
                }) => {}
                _ => unreachable!(),
            }
            match relevant_events.pop_front() {
                Some(Event::WindowEvent {
                    event:
                        WindowEvent::CursorMoved {
                            position: PhysicalPosition { x, y },
                            ..
                        },
                    ..
                }) if x as i32 == cursor_x && y as i32 == cursor_y => {}
                _ => unreachable!(),
            }
            // match relevant_events.pop_front() {
            //     Some(Event::WindowEvent {
            //         event: WindowEvent::KeyboardInput {
            //             input: KeyboardInput {
            //                 state,
            //                 virtual_keycode,
            //                 ..
            //             }, ..
            //         }, ..
            //     }) if state == ElementState::Pressed && virtual_keycode == Some(VirtualKeyCode::Escape) => {},
            //     event @ _ => {
            //         println!("Unexpected event: {:?}", event);
            //         unreachable!()
            //     },
            // }
        })
    };

    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_inner_size(winit::dpi::LogicalSize::new(256.0, 256.0))
        .build(&event_loop)
        .unwrap();

    window.set_outer_position(window_pos);
    let inner_pos = window.inner_position().unwrap();
    let mut enigo_handle = Some(create_enigo_thread(inner_pos.x as i32, inner_pos.y as i32));

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        println!("{:?}", event);

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
                window_id,
            } => *control_flow = ControlFlow::Exit,
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
