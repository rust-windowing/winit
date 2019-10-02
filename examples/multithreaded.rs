extern crate env_logger;
use std::{collections::HashMap, sync::mpsc, thread, time::Duration};

use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{CursorIcon, Fullscreen, WindowBuilder},
};

const WINDOW_COUNT: usize = 3;
const WINDOW_SIZE: (u32, u32) = (600, 400);

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let mut window_senders = HashMap::with_capacity(WINDOW_COUNT);
    for _ in 0..WINDOW_COUNT {
        let window = WindowBuilder::new()
            .with_inner_size(WINDOW_SIZE.into())
            .build(&event_loop)
            .unwrap();

        let mut video_modes: Vec<_> = window.current_monitor().video_modes().collect();
        let mut video_mode_id = 0usize;

        let (tx, rx) = mpsc::channel();
        window_senders.insert(window.id(), tx);
        thread::spawn(move || {
            while let Ok(event) = rx.recv() {
                match event {
                    WindowEvent::Moved { .. } => {
                        // We need to update our chosen video mode if the window
                        // was moved to an another monitor, so that the window
                        // appears on this monitor instead when we go fullscreen
                        let previous_video_mode = video_modes.iter().cloned().nth(video_mode_id);
                        video_modes = window.current_monitor().video_modes().collect();
                        video_mode_id = video_mode_id.min(video_modes.len());
                        let video_mode = video_modes.iter().nth(video_mode_id);

                        // Different monitors may support different video modes,
                        // and the index we chose previously may now point to a
                        // completely different video mode, so notify the user
                        if video_mode != previous_video_mode.as_ref() {
                            println!(
                                "Window moved to another monitor, picked video mode: {}",
                                video_modes.iter().nth(video_mode_id).unwrap()
                            );
                        }
                    }
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Released,
                                virtual_keycode: Some(key),
                                modifiers,
                                ..
                            },
                        ..
                    } => {
                        window.set_title(&format!("{:?}", key));
                        let state = !modifiers.shift;
                        use self::VirtualKeyCode::*;
                        match key {
                            A => window.set_always_on_top(state),
                            C => window.set_cursor_icon(match state {
                                true => CursorIcon::Progress,
                                false => CursorIcon::Default,
                            }),
                            D => window.set_decorations(!state),
                            // Cycle through video modes
                            Right | Left => {
                                video_mode_id = match key {
                                    Left => video_mode_id.saturating_sub(1),
                                    Right => (video_modes.len() - 1).min(video_mode_id + 1),
                                    _ => unreachable!(),
                                };
                                println!(
                                    "Picking video mode: {}",
                                    video_modes.iter().nth(video_mode_id).unwrap()
                                );
                            }
                            F => window.set_fullscreen(match (state, modifiers.alt) {
                                (true, false) => {
                                    Some(Fullscreen::Borderless(window.current_monitor()))
                                }
                                (true, true) => Some(Fullscreen::Exclusive(
                                    video_modes.iter().nth(video_mode_id).unwrap().clone(),
                                )),
                                (false, _) => None,
                            }),
                            G => window.set_cursor_grab(state).unwrap(),
                            H => window.set_cursor_visible(!state),
                            I => {
                                println!("Info:");
                                println!("-> outer_position : {:?}", window.outer_position());
                                println!("-> inner_position : {:?}", window.inner_position());
                                println!("-> outer_size     : {:?}", window.outer_size());
                                println!("-> inner_size     : {:?}", window.inner_size());
                                println!("-> fullscreen     : {:?}", window.fullscreen());
                            }
                            L => window.set_min_inner_size(match state {
                                true => Some(WINDOW_SIZE.into()),
                                false => None,
                            }),
                            M => window.set_maximized(state),
                            P => window.set_outer_position({
                                let mut position = window.outer_position().unwrap();
                                let sign = if state { 1.0 } else { -1.0 };
                                position.x += 10.0 * sign;
                                position.y += 10.0 * sign;
                                position
                            }),
                            Q => window.request_redraw(),
                            R => window.set_resizable(state),
                            S => window.set_inner_size(
                                match state {
                                    true => (WINDOW_SIZE.0 + 100, WINDOW_SIZE.1 + 100),
                                    false => WINDOW_SIZE,
                                }
                                .into(),
                            ),
                            W => window
                                .set_cursor_position(
                                    (WINDOW_SIZE.0 as i32 / 2, WINDOW_SIZE.1 as i32 / 2).into(),
                                )
                                .unwrap(),
                            Z => {
                                window.set_visible(false);
                                thread::sleep(Duration::from_secs(1));
                                window.set_visible(true);
                            }
                            _ => (),
                        }
                    }
                    _ => (),
                }
            }
        });
    }
    event_loop.run(move |event, _event_loop, control_flow| {
        *control_flow = match !window_senders.is_empty() {
            true => ControlFlow::Wait,
            false => ControlFlow::Exit,
        };
        match event {
            Event::WindowEvent { event, window_id } => match event {
                WindowEvent::CloseRequested
                | WindowEvent::Destroyed
                | WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            state: ElementState::Released,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                    ..
                } => {
                    window_senders.remove(&window_id);
                }
                _ => {
                    if let Some(tx) = window_senders.get(&window_id) {
                        tx.send(event).unwrap();
                    }
                }
            },
            _ => (),
        }
    })
}
