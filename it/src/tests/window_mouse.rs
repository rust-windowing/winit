use crate::backend::{Button, Instance};
use crate::eventstash::EventStash;
use crate::keyboard::Key;
use std::collections::HashSet;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, TouchPhase};
use winit::keyboard::{KeyCode, ModifiersState};
use winit::window::WindowBuilder;

test!(run);

async fn run(instance: &dyn Instance) {
    let seat = instance.default_seat();
    seat.set_cursor_position(500, 500);
    let mouse1 = seat.add_mouse();
    let kb1 = seat.add_keyboard();

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

    seat.focus(&*window);

    {
        log::info!("Checking button presses inside and outside of window");

        mouse1.press(Button::Right);
        mouse1.move_(-450, -450);
        mouse1.press(Button::Left);

        let (we, mi) = events.window_mouse_input_event().await;
        assert_eq!(we.window_id, window.winit_id());
        assert_eq!(mi.button, MouseButton::Left);
        assert_eq!(mi.state, ElementState::Pressed);

        let (we, mi) = events.window_mouse_input_event().await;
        assert_eq!(we.window_id, window.winit_id());
        assert_eq!(mi.button, MouseButton::Left);
        assert_eq!(mi.state, ElementState::Released);
    }

    {
        log::info!("Checking cursor-left/entered events");

        mouse1.move_(450, 450);

        let (we, cl) = events.window_cursor_left().await;
        assert_eq!(we.window_id, window.winit_id());
        assert!(seat.is(cl.device_id));

        mouse1.move_(-450, -450);

        let (we, cl) = events.window_cursor_entered().await;
        assert_eq!(we.window_id, window.winit_id());
        assert!(seat.is(cl.device_id));
    }

    {
        log::info!("Mouse movement inside of window");
        el.barrier().await;

        mouse1.move_(1, 1);

        let (we, cl) = events.window_cursor_moved().await;
        assert_eq!(we.window_id, window.winit_id());
        assert!(seat.is(cl.device_id));
        assert_eq!(cl.position, PhysicalPosition { x: 51.0, y: 51.0 });
    }

    {
        log::info!("Checking scrolling inside of window");
        el.barrier().await;

        mouse1.scroll(1, 2);

        let (we, cl) = events.window_mouse_wheel().await;
        assert_eq!(we.window_id, window.winit_id());
        assert!(seat.is(cl.device_id));
        assert_eq!(cl.phase, TouchPhase::Moved);
        let mut delta = match cl.delta {
            MouseScrollDelta::LineDelta(dx, dy) => (dx, dy),
            _ => unreachable!(),
        };
        if delta != (1.0, 2.0) {
            let (we, cl) = events.window_mouse_wheel().await;
            assert_eq!(we.window_id, window.winit_id());
            assert!(seat.is(cl.device_id));
            assert_eq!(cl.phase, TouchPhase::Moved);
            match cl.delta {
                MouseScrollDelta::LineDelta(dx, dy) => {
                    delta.0 += dx;
                    delta.1 += dy;
                }
                _ => unreachable!(),
            }
        }
        assert_eq!(delta, (1.0, 2.0));
    }

    {
        log::info!("Checking that scrolling outside of window is ignored");
        el.barrier().await;

        let mut stash = EventStash::new();

        {
            let mut stash2 = stash.stash(&mut *events);

            // move cursor out of window before scrolling
            mouse1.move_(450, 450);
            mouse1.scroll(1, 0);
            // move cursor over window
            mouse1.move_(-450, -450);
            // X11 backend requires this barrier
            stash2.window_cursor_entered().await;
            mouse1.scroll(0, 1);

            let (we, cl) = stash2.window_mouse_wheel().await;
            assert_eq!(we.window_id, window.winit_id());
            assert!(seat.is(cl.device_id));
            assert_eq!(cl.phase, TouchPhase::Moved);
            assert_eq!(cl.delta, MouseScrollDelta::LineDelta(0.0, 1.0));
        }

        let mut consume = stash.consume();
        let (we, cl) = consume.window_mouse_wheel().await;
        assert_eq!(we.window_id, window.winit_id());
        assert!(seat.is(cl.device_id));
        assert_eq!(cl.phase, TouchPhase::Moved);
        assert_eq!(cl.delta, MouseScrollDelta::LineDelta(0.0, 1.0));
    }

    {
        log::info!("Checking that unfocused windows do not get keyboard input");
        el.barrier().await;
        seat.un_focus();
        kb1.press(Key::KeyEsc);
        seat.focus(&*window);
        kb1.press(Key::KeyA);
        let (_, ke) = events.window_keyboard_input().await;
        assert_eq!(ke.event.physical_key, KeyCode::KeyA);
        seat.un_focus();
    }

    {
        log::info!("Checking modifiers changed on cursor-enter");
        // move cursor out of window
        mouse1.move_(450, 450);
        el.barrier().await;
        kb1.press(Key::KeyLeftctrl);
        let _shift = kb1.press(Key::KeyLeftshift);
        mouse1.move_(-450, -450);
        let (_, mc) = events.window_modifiers().await;
        assert_eq!(mc, ModifiersState::SHIFT);
    }

    {
        log::info!("Testing multi-window modifiers events");
        let w2 = el.create_window(WindowBuilder::new().with_inner_size(PhysicalSize {
            width: 100,
            height: 100,
        }));
        w2.mapped(true).await;
        w2.set_outer_position(300, 300);
        w2.outer_position(300, 300).await;
        seat.focus(&*window);
        {
            seat.set_cursor_position(310 + w2.inner_offset().0, 310 + w2.inner_offset().1);
            loop {
                let (we, cp) = events.window_cursor_moved().await;
                if we.window_id == w2.winit_id() && cp.position.x == 10.0 && cp.position.y == 10.0 {
                    break;
                }
            }
            let _shift = kb1.press(Key::KeyLeftshift);
            let mut targets = HashSet::new();
            targets.insert(window.winit_id());
            targets.insert(w2.winit_id());
            while !targets.is_empty() {
                let (we, mo) = events.window_modifiers().await;
                assert_eq!(mo, ModifiersState::SHIFT);
                targets.remove(&we.window_id);
            }
        }
        el.barrier().await;
        {
            seat.set_cursor_position(500, 500);
            events.window_cursor_left().await;
            let _shift = kb1.press(Key::KeyLeftshift);
            let (we, mo) = events.window_modifiers().await;
            assert_eq!(mo, ModifiersState::SHIFT);
            assert_eq!(we.window_id, window.winit_id());
            seat.set_cursor_position(310 + w2.inner_offset().0, 310 + w2.inner_offset().1);
            let (we, mo) = events.window_modifiers().await;
            assert_eq!(mo, ModifiersState::SHIFT);
            assert_eq!(we.window_id, w2.winit_id());
        }
        el.barrier().await;
        {
            seat.set_cursor_position(500, 500);
            events.window_cursor_left().await;
            {
                let _shift = kb1.press(Key::KeyLeftshift);
                let (we, mo) = events.window_modifiers().await;
                assert_eq!(mo, ModifiersState::SHIFT);
                assert_eq!(we.window_id, window.winit_id());
            }
            let (we, mo) = events.window_modifiers().await;
            assert_eq!(mo, ModifiersState::empty());
            assert_eq!(we.window_id, window.winit_id());
            el.barrier().await;
            seat.set_cursor_position(310 + w2.inner_offset().0, 310 + w2.inner_offset().1);
            let (we, mo) = events.window_modifiers().await;
            assert_eq!(mo, ModifiersState::empty());
            assert_eq!(we.window_id, w2.winit_id());
        }
    }
}
