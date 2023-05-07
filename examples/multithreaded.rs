#![allow(clippy::single_match)]

#[cfg(not(wasm_platform))]
fn main() {
    use std::{collections::HashMap, sync::mpsc, thread, time::Duration};

    use simple_logger::SimpleLogger;
    use winit::{
        dpi::{PhysicalPosition, PhysicalSize, Position, Size},
        event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
        event_loop::EventLoop,
        window::{CursorGrabMode, CursorIcon, Fullscreen, WindowBuilder, WindowLevel},
    };

    const WINDOW_COUNT: usize = 3;
    const WINDOW_SIZE: PhysicalSize<u32> = PhysicalSize::new(600, 400);

    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();
    let mut window_senders = HashMap::with_capacity(WINDOW_COUNT);
    for _ in 0..WINDOW_COUNT {
        let window = WindowBuilder::new()
            .with_inner_size(WINDOW_SIZE)
            .build(&event_loop)
            .unwrap();

        let mut video_modes: Vec<_> = window.current_monitor().unwrap().video_modes().collect();
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
                        let previous_video_mode = video_modes.get(video_mode_id).cloned();
                        video_modes = window.current_monitor().unwrap().video_modes().collect();
                        video_mode_id = video_mode_id.min(video_modes.len());
                        let video_mode = video_modes.get(video_mode_id);

                        // Different monitors may support different video modes,
                        // and the index we chose previously may now point to a
                        // completely different video mode, so notify the user
                        if video_mode != previous_video_mode.as_ref() {
                            println!(
                                "Window moved to another monitor, picked video mode: {}",
                                video_modes.get(video_mode_id).unwrap()
                            );
                        }
                    }
                    #[allow(deprecated)]
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
                        window.set_title(&format!("{key:?}"));
                        let state = !modifiers.shift();
                        use VirtualKeyCode::*;
                        match key {
                            Key1 => window.set_window_level(WindowLevel::AlwaysOnTop),
                            Key2 => window.set_window_level(WindowLevel::AlwaysOnBottom),
                            Key3 => window.set_window_level(WindowLevel::Normal),
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
                                println!("Picking video mode: {}", video_modes[video_mode_id]);
                            }
                            F => window.set_fullscreen(match (state, modifiers.alt()) {
                                (true, false) => Some(Fullscreen::Borderless(None)),
                                (true, true) => {
                                    Some(Fullscreen::Exclusive(video_modes[video_mode_id].clone()))
                                }
                                (false, _) => None,
                            }),
                            L if state => {
                                if let Err(err) = window.set_cursor_grab(CursorGrabMode::Locked) {
                                    println!("error: {err}");
                                }
                            }
                            G if state => {
                                if let Err(err) = window.set_cursor_grab(CursorGrabMode::Confined) {
                                    println!("error: {err}");
                                }
                            }
                            G | L if !state => {
                                if let Err(err) = window.set_cursor_grab(CursorGrabMode::None) {
                                    println!("error: {err}");
                                }
                            }
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
                                true => Some(WINDOW_SIZE),
                                false => None,
                            }),
                            M => window.set_maximized(state),
                            P => window.set_outer_position({
                                let mut position = window.outer_position().unwrap();
                                let sign = if state { 1 } else { -1 };
                                position.x += 10 * sign;
                                position.y += 10 * sign;
                                position
                            }),
                            Q => window.request_redraw(),
                            R => window.set_resizable(state),
                            S => window.set_inner_size(match state {
                                true => PhysicalSize::new(
                                    WINDOW_SIZE.width + 100,
                                    WINDOW_SIZE.height + 100,
                                ),
                                false => WINDOW_SIZE,
                            }),
                            W => {
                                if let Size::Physical(size) = WINDOW_SIZE.into() {
                                    window
                                        .set_cursor_position(Position::Physical(
                                            PhysicalPosition::new(
                                                size.width as i32 / 2,
                                                size.height as i32 / 2,
                                            ),
                                        ))
                                        .unwrap()
                                }
                            }
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
        match !window_senders.is_empty() {
            true => control_flow.set_wait(),
            false => control_flow.set_exit(),
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
                        if let Some(event) = event.to_static() {
                            tx.send(event).unwrap();
                        }
                    }
                }
            },
            _ => {}
        }
    })
}

#[cfg(wasm_platform)]
fn main() {
    panic!("Example not supported on Wasm");
}
