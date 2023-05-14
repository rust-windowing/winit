use std::collections::{BTreeSet, HashSet};

use winit::event;

macro_rules! foreach_event {
    ($closure:expr) => {{
        #[allow(unused_mut)]
        let mut x = $closure;
        let did = unsafe { event::DeviceId::dummy() };

        #[allow(deprecated)]
        {
            use winit::event::{Event::*, Ime::Enabled, WindowEvent::*};
            use winit::window::WindowId;

            // Mainline events.
            let wid = unsafe { WindowId::dummy() };
            x(UserEvent(()));
            x(NewEvents(event::StartCause::Init));
            x(MainEventsCleared);
            x(RedrawRequested(wid));
            x(RedrawEventsCleared);
            x(LoopDestroyed);
            x(Suspended);
            x(Resumed);

            // Window events.
            let with_window_event = |wev| {
                x(WindowEvent {
                    window_id: wid,
                    event: wev,
                })
            };

            with_window_event(CloseRequested);
            with_window_event(Destroyed);
            with_window_event(Focused(true));
            with_window_event(Moved((0, 0).into()));
            with_window_event(Resized((0, 0).into()));
            with_window_event(ReceivedCharacter('a'));
            with_window_event(DroppedFile("x.txt".into()));
            with_window_event(HoveredFile("x.txt".into()));
            with_window_event(HoveredFileCancelled);
            with_window_event(KeyboardInput {
                device_id: did,
                is_synthetic: false,
                input: event::KeyboardInput {
                    scancode: 0,
                    state: event::ElementState::Pressed,
                    virtual_keycode: Some(event::VirtualKeyCode::A),
                    modifiers: event::ModifiersState::default(),
                },
            });
            with_window_event(Ime(Enabled));
            with_window_event(CursorMoved {
                device_id: did,
                position: (0, 0).into(),
                modifiers: event::ModifiersState::default(),
            });
            with_window_event(ModifiersChanged(event::ModifiersState::default()));
            with_window_event(CursorEntered { device_id: did });
            with_window_event(CursorLeft { device_id: did });
            with_window_event(MouseWheel {
                device_id: did,
                delta: event::MouseScrollDelta::LineDelta(0.0, 0.0),
                phase: event::TouchPhase::Started,
                modifiers: event::ModifiersState::default(),
            });
            with_window_event(MouseInput {
                device_id: did,
                state: event::ElementState::Pressed,
                button: event::MouseButton::Other(0),
                modifiers: event::ModifiersState::default(),
            });
            with_window_event(TouchpadMagnify {
                device_id: did,
                delta: 0.0,
                phase: event::TouchPhase::Started,
            });
            with_window_event(SmartMagnify { device_id: did });
            with_window_event(TouchpadRotate {
                device_id: did,
                delta: 0.0,
                phase: event::TouchPhase::Started,
            });
            with_window_event(TouchpadPressure {
                device_id: did,
                pressure: 0.0,
                stage: 0,
            });
            with_window_event(AxisMotion {
                device_id: did,
                axis: 0,
                value: 0.0,
            });
            with_window_event(Touch(event::Touch {
                device_id: did,
                phase: event::TouchPhase::Started,
                location: (0.0, 0.0).into(),
                id: 0,
                force: Some(event::Force::Normalized(0.0)),
            }));
            with_window_event(ThemeChanged(winit::window::Theme::Light));
            with_window_event(Occluded(true));
        }

        #[allow(deprecated)]
        {
            use event::DeviceEvent::*;

            let with_device_event = |dev_ev| {
                x(event::Event::DeviceEvent {
                    device_id: did,
                    event: dev_ev,
                })
            };

            with_device_event(Added);
            with_device_event(Removed);
            with_device_event(MouseMotion {
                delta: (0.0, 0.0).into(),
            });
            with_device_event(MouseWheel {
                delta: event::MouseScrollDelta::LineDelta(0.0, 0.0),
            });
            with_device_event(Motion {
                axis: 0,
                value: 0.0,
            });
            with_device_event(Button {
                button: 0,
                state: event::ElementState::Pressed,
            });
            with_device_event(Key(event::KeyboardInput {
                scancode: 0,
                state: event::ElementState::Pressed,
                virtual_keycode: Some(event::VirtualKeyCode::A),
                modifiers: event::ModifiersState::default(),
            }));
            with_device_event(Text { codepoint: 'a' });
        }
    }};
}

#[test]
fn test_event_clone() {
    foreach_event!(|event: event::Event<'static, ()>| {
        let event2 = event.clone();
        assert_eq!(event, event2);
    })
}

#[test]
#[should_panic]
fn test_cant_clone_scale_factor_changed() {
    let inner_size = Box::new((0, 0).into());
    let ev: event::Event<'_, ()> = event::Event::WindowEvent {
        window_id: unsafe { winit::window::WindowId::dummy() },
        event: event::WindowEvent::ScaleFactorChanged {
            scale_factor: 1.0,
            new_inner_size: Box::leak(inner_size),
        },
    };
    let _ = ev.clone();
}

#[test]
fn test_map_nonuser_event() {
    foreach_event!(|event: event::Event<'static, ()>| {
        let is_user = matches!(event, event::Event::UserEvent(()));
        let event2 = event.map_nonuser_event::<()>();
        if is_user {
            assert_eq!(event2, Err(event::Event::UserEvent(())));
        } else {
            assert!(event2.is_ok());
        }
    })
}

#[test]
fn test_to_static() {
    foreach_event!(|event: event::Event<'static, ()>| {
        let event2 = event.clone().to_static();
        assert_eq!(Some(event), event2);
    })
}

#[test]
fn test_scale_factor_changed_to_static() {
    let mut inner_size = (0, 0).into();
    let ev: event::Event<'_, ()> = event::Event::WindowEvent {
        window_id: unsafe { winit::window::WindowId::dummy() },
        event: event::WindowEvent::ScaleFactorChanged {
            scale_factor: 1.0,
            new_inner_size: &mut inner_size,
        },
    };
    assert!(ev.to_static().is_none());
}

#[test]
fn test_force_normalize() {
    let force = event::Force::Normalized(0.0);
    assert_eq!(force.normalized(), 0.0);

    let force2 = event::Force::Calibrated {
        force: 5.0,
        max_possible_force: 2.5,
        altitude_angle: None,
    };
    assert_eq!(force2.normalized(), 2.0);

    let force3 = event::Force::Calibrated {
        force: 5.0,
        max_possible_force: 2.5,
        altitude_angle: Some(std::f64::consts::PI / 2.0),
    };
    assert_eq!(force3.normalized(), 2.0);
}

#[test]
fn test_modifiers() {
    assert!(event::ModifiersState::SHIFT.shift());
    assert!(event::ModifiersState::CTRL.ctrl());
    assert!(event::ModifiersState::ALT.alt());
    assert!(event::ModifiersState::LOGO.logo());
}

#[test]
fn attr_coverage() {
    foreach_event!(|event: event::Event<'static, ()>| {
        let _ = format!("{:?}", event);
    });
    let _ = event::StartCause::Init.clone();

    let did = unsafe { winit::event::DeviceId::dummy() }.clone();
    HashSet::new().insert(did);
    let mut set = [did, did, did];
    set.sort_unstable();
    let mut set2 = BTreeSet::new();
    set2.insert(did);
    set2.insert(did);

    HashSet::new().insert(event::KeyboardInput {
        scancode: 0,
        state: event::ElementState::Pressed,
        virtual_keycode: Some(event::VirtualKeyCode::A),
        #[allow(deprecated)]
        modifiers: event::ModifiersState::default(),
    });
    HashSet::new().insert(event::TouchPhase::Started.clone());
    HashSet::new().insert(event::MouseButton::Left.clone());
    HashSet::new().insert(event::Ime::Enabled);

    let _ = event::Touch {
        device_id: did,
        phase: event::TouchPhase::Started,
        location: (0.0, 0.0).into(),
        id: 0,
        force: Some(event::Force::Normalized(0.0)),
    }
    .clone();
    let _ = event::Force::Calibrated {
        force: 0.0,
        max_possible_force: 0.0,
        altitude_angle: None,
    }
    .clone();

    let mut set = [
        event::VirtualKeyCode::A,
        event::VirtualKeyCode::C,
        event::VirtualKeyCode::B,
    ];
    set.sort_unstable();
    let mut set2 = BTreeSet::new();
    set2.insert(event::VirtualKeyCode::A);
    set2.insert(event::VirtualKeyCode::C);
    set2.insert(event::VirtualKeyCode::B.clone());
}
