use crate::backend::Instance;
use crate::keyboard::Key::{
    KeyEsc, KeyL, KeyLeftbrace, KeyLeftctrl, KeyLeftshift, KeyQ, KeyRightalt, KeyRightctrl,
};
use crate::keyboard::Layout;
use winit::event::ElementState;
use winit::keyboard::{Key as WKey, KeyCode, KeyLocation, ModifiersState};

test!(run);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();
    let window = el.create_window(Default::default());
    window.mapped(true).await;
    let seat = instance.default_seat();
    seat.focus(&*window);
    let kb = seat.add_keyboard();

    {
        log::info!("Testing L");
        // L Press
        // L Release
        kb.press(KeyL);
        for i in 0..2 {
            let (_, ki) = events.window_keyboard_input().await;
            assert_eq!(ki.event.physical_key, KeyCode::KeyL);
            assert_eq!(ki.event.logical_key, WKey::Character("l"));
            assert_eq!(ki.event.location, KeyLocation::Standard);
            if i == 0 {
                assert_eq!(ki.event.text, Some("l"));
                assert_eq!(ki.event.state, ElementState::Pressed);
            } else {
                assert_eq!(ki.event.text, None);
                assert_eq!(ki.event.state, ElementState::Released);
            }
            assert_eq!(ki.event.repeat, false);
            #[cfg(have_mod_supplement)]
            {
                assert_eq!(
                    ki.event.mod_supplement.key_without_modifiers,
                    WKey::Character("l")
                );
                if i == 0 {
                    assert_eq!(
                        ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                        Some("l")
                    );
                } else {
                    assert_eq!(
                        ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                        None
                    );
                }
            }
        }
    }

    {
        log::info!("Testing Shift-L");
        // LeftShift Press
        // L Press
        // L Release
        // LeftShift Release
        {
            let _shift = kb.press(KeyLeftshift);
            kb.press(KeyL);
        }
        // 0: Shift pressed
        // 1: Modifiers changed
        // 2: L pressed
        // 3: L released
        // 4: Shift released
        // 5: Modifiers changed
        for i in 0..6 {
            if i == 1 || i == 5 {
                let (_, mo) = events.window_modifiers().await;
                if i == 1 {
                    assert_eq!(mo, ModifiersState::SHIFT);
                } else {
                    assert_eq!(mo, ModifiersState::empty());
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                assert_eq!(ki.event.repeat, false);
                if i == 0 || i == 4 {
                    assert_eq!(ki.event.physical_key, KeyCode::ShiftLeft);
                    assert_eq!(ki.event.logical_key, WKey::Shift);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Left);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Shift);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else {
                    assert_eq!(ki.event.physical_key, KeyCode::KeyL);
                    assert_eq!(ki.event.logical_key, WKey::Character("L"));
                    if i == 2 {
                        assert_eq!(ki.event.text, Some("L"));
                    } else {
                        assert_eq!(ki.event.text, None);
                    }
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Character("l")
                        );
                        if i == 2 {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                Some("L")
                            );
                        } else {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                None
                            );
                        }
                    }
                }
                if i == 0 || i == 2 {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }

    {
        log::info!("Testing Shift-L (not nested)");
        // LeftShift Press
        // L Press
        // LeftShift Release
        // L Release
        {
            let shift = kb.press(KeyLeftshift);
            let _l = kb.press(KeyL);
            drop(shift);
        }
        // 0: Shift pressed
        // 1: Modifiers changed
        // 2: L pressed
        // 3: Shift released
        // 4: Modifiers changed
        // 5: L released
        for i in 0..6 {
            if i == 1 || i == 4 {
                let (_, mo) = events.window_modifiers().await;
                if i == 1 {
                    assert_eq!(mo, ModifiersState::SHIFT);
                } else {
                    assert_eq!(mo, ModifiersState::empty());
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                assert_eq!(ki.event.repeat, false);
                if i == 0 || i == 3 {
                    assert_eq!(ki.event.physical_key, KeyCode::ShiftLeft);
                    assert_eq!(ki.event.logical_key, WKey::Shift);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Left);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Shift);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else {
                    assert_eq!(ki.event.physical_key, KeyCode::KeyL);
                    #[cfg(have_mod_supplement)]
                    assert_eq!(
                        ki.event.mod_supplement.key_without_modifiers,
                        WKey::Character("l")
                    );
                    if i == 2 {
                        assert_eq!(ki.event.logical_key, WKey::Character("L"));
                        assert_eq!(ki.event.text, Some("L"));
                        #[cfg(have_mod_supplement)]
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            Some("L")
                        );
                    } else {
                        assert_eq!(ki.event.logical_key, WKey::Character("l"));
                        assert_eq!(ki.event.text, None);
                        #[cfg(have_mod_supplement)]
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                }
                if i == 0 || i == 2 {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }

    {
        log::info!("Testing Ctrl-L");
        // RightCtrl Press
        // L Press
        // L Release
        // RightCtrl Release
        {
            let _shift = kb.press(KeyRightctrl);
            kb.press(KeyL);
        }
        // 0: Ctrl pressed
        // 1: Modifiers changed
        // 2: L pressed
        // 3: L released
        // 4: Ctrl released
        // 5: Modifiers changed
        for i in 0..6 {
            if i == 1 || i == 5 {
                let (_, mo) = events.window_modifiers().await;
                if i == 1 {
                    assert_eq!(mo, ModifiersState::CONTROL);
                } else {
                    assert_eq!(mo, ModifiersState::empty());
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                assert_eq!(ki.event.repeat, false);
                if i == 0 || i == 4 {
                    assert_eq!(ki.event.physical_key, KeyCode::ControlRight);
                    assert_eq!(ki.event.logical_key, WKey::Control);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Right);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Control);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else {
                    assert_eq!(ki.event.physical_key, KeyCode::KeyL);
                    assert_eq!(ki.event.logical_key, WKey::Character("l"));
                    if i == 2 {
                        assert_eq!(ki.event.text, Some("l"));
                    } else {
                        assert_eq!(ki.event.text, None);
                    }
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Character("l")
                        );
                        if i == 2 {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                Some("\x0c")
                            );
                        } else {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                None
                            );
                        }
                    }
                }
                if i == 0 || i == 2 {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }
    {
        log::info!("Testing Ctrl-Shift-L");
        // RightCtrl Press
        // LiftShift Press
        // L Press
        // L Release
        // RightCtrl Release
        // LiftShift Release
        {
            let ctrl = kb.press(KeyRightctrl);
            let _shift = kb.press(KeyLeftshift);
            kb.press(KeyL);
            drop(ctrl);
        }
        // 0: Ctrl pressed
        // 1: Modifiers changed
        // 2: Shift pressed
        // 3: Modifiers changed
        // 4: L pressed
        // 5: L released
        // 6: Ctrl released
        // 7: Modifiers changed
        // 8: Shift released
        // 9: Modifiers changed
        for i in 0..10 {
            if matches!(i, 1 | 3 | 7 | 9) {
                let (_, mo) = events.window_modifiers().await;
                match i {
                    1 => assert_eq!(mo, ModifiersState::CONTROL),
                    3 => assert_eq!(mo, ModifiersState::CONTROL | ModifiersState::SHIFT),
                    7 => assert_eq!(mo, ModifiersState::SHIFT),
                    9 => assert_eq!(mo, ModifiersState::empty()),
                    _ => unreachable!(),
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                assert_eq!(ki.event.repeat, false);
                if matches!(i, 0 | 6) {
                    assert_eq!(ki.event.physical_key, KeyCode::ControlRight);
                    assert_eq!(ki.event.logical_key, WKey::Control);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Right);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Control);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else if matches!(i, 2 | 8) {
                    assert_eq!(ki.event.physical_key, KeyCode::ShiftLeft);
                    assert_eq!(ki.event.logical_key, WKey::Shift);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Left);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Shift);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else {
                    assert_eq!(ki.event.physical_key, KeyCode::KeyL);
                    assert_eq!(ki.event.logical_key, WKey::Character("L"));
                    if i == 4 {
                        assert_eq!(ki.event.text, Some("L"));
                    } else {
                        assert_eq!(ki.event.text, None);
                    }
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Character("l")
                        );
                        if i == 4 {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                Some("\x0c")
                            );
                        } else {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                None
                            );
                        }
                    }
                }
                if matches!(i, 0 | 2 | 4) {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }

    {
        log::info!("Testing Alt");
        // LeftAlt Press
        // LeftAlt Release
        {
            kb.press(KeyRightalt);
        }
        // 0: Alt pressed
        // 1: Modifiers changed
        // 2: Alt released
        // 3: Modifiers changed
        for i in 0..4 {
            if i == 1 || i == 3 {
                let (_, mo) = events.window_modifiers().await;
                if i == 1 {
                    assert_eq!(mo, ModifiersState::ALT);
                } else {
                    assert_eq!(mo, ModifiersState::empty());
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                assert_eq!(ki.event.repeat, false);
                assert_eq!(ki.event.physical_key, KeyCode::AltRight);
                assert_eq!(ki.event.logical_key, WKey::Alt);
                assert_eq!(ki.event.text, None);
                assert_eq!(ki.event.location, KeyLocation::Right);
                #[cfg(have_mod_supplement)]
                {
                    assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Alt);
                    assert_eq!(
                        ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                        None
                    );
                }
                if i == 0 {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }
    {
        log::info!("Testing Ctrl-Shift-L");
        // RightCtrl Press
        // LiftShift Press
        // L Press
        // L Release
        // RightCtrl Release
        // LiftShift Release
        {
            let ctrl = kb.press(KeyRightctrl);
            let _shift = kb.press(KeyLeftshift);
            kb.press(KeyL);
            drop(ctrl);
        }
        // 0: Ctrl pressed
        // 1: Modifiers changed
        // 2: Shift pressed
        // 3: Modifiers changed
        // 4: L pressed
        // 5: L released
        // 6: Ctrl released
        // 7: Modifiers changed
        // 8: Shift released
        // 9: Modifiers changed
        for i in 0..10 {
            if matches!(i, 1 | 3 | 7 | 9) {
                let (_, mo) = events.window_modifiers().await;
                match i {
                    1 => assert_eq!(mo, ModifiersState::CONTROL),
                    3 => assert_eq!(mo, ModifiersState::CONTROL | ModifiersState::SHIFT),
                    7 => assert_eq!(mo, ModifiersState::SHIFT),
                    9 => assert_eq!(mo, ModifiersState::empty()),
                    _ => unreachable!(),
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                assert_eq!(ki.event.repeat, false);
                if matches!(i, 0 | 6) {
                    assert_eq!(ki.event.physical_key, KeyCode::ControlRight);
                    assert_eq!(ki.event.logical_key, WKey::Control);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Right);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Control);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else if matches!(i, 2 | 8) {
                    assert_eq!(ki.event.physical_key, KeyCode::ShiftLeft);
                    assert_eq!(ki.event.logical_key, WKey::Shift);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Left);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Shift);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else {
                    assert_eq!(ki.event.physical_key, KeyCode::KeyL);
                    assert_eq!(ki.event.logical_key, WKey::Character("L"));
                    if i == 4 {
                        assert_eq!(ki.event.text, Some("L"));
                    } else {
                        assert_eq!(ki.event.text, None);
                    }
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Character("l")
                        );
                        if i == 4 {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                Some("\x0c")
                            );
                        } else {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                None
                            );
                        }
                    }
                }
                if matches!(i, 0 | 2 | 4) {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }

    log::info!("Switching to Azerty layout.");
    seat.set_layout(Layout::Azerty);

    {
        log::info!("Testing A (KeyQ)");
        // Q Press
        // Q Release
        {
            kb.press(KeyQ);
        }
        // 0: Q pressed
        // 1: Q released
        for i in 0..2 {
            let (_, ki) = events.window_keyboard_input().await;
            assert_eq!(ki.event.physical_key, KeyCode::KeyQ);
            assert_eq!(ki.event.logical_key, WKey::Character("a"));
            if i == 0 {
                assert_eq!(ki.event.text, Some("a"));
            } else {
                assert_eq!(ki.event.text, None);
            }
            assert_eq!(ki.event.location, KeyLocation::Standard);
            #[cfg(have_mod_supplement)]
            {
                assert_eq!(
                    ki.event.mod_supplement.key_without_modifiers,
                    WKey::Character("a")
                );
                if i == 0 {
                    assert_eq!(
                        ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                        Some("a")
                    );
                } else {
                    assert_eq!(
                        ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                        None
                    );
                }
            }
            if i == 0 {
                assert_eq!(ki.event.state, ElementState::Pressed);
            } else {
                assert_eq!(ki.event.state, ElementState::Released);
            }
        }
    }
    {
        log::info!("Testing â (KeyLeftbrace, KeyQ)");
        // LeftBrace Press
        // LeftBrace Release
        // Q Press
        // Q Release
        {
            kb.press(KeyLeftbrace);
            kb.press(KeyQ);
        }
        // 0: LeftBrace pressed
        // 1: LeftBrace released
        // 2: Q pressed
        // 3: Q released
        for i in 0..4 {
            let (_, ki) = events.window_keyboard_input().await;
            if matches!(i, 0 | 1) {
                assert_eq!(ki.event.physical_key, KeyCode::BracketLeft);
                assert_eq!(ki.event.logical_key, WKey::Dead(Some('^')));
                assert_eq!(ki.event.text, None);
                assert_eq!(ki.event.location, KeyLocation::Standard);
                #[cfg(have_mod_supplement)]
                {
                    assert_eq!(
                        ki.event.mod_supplement.key_without_modifiers,
                        WKey::Dead(Some('^'))
                    );
                    assert_eq!(
                        ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                        None
                    );
                }
                if i == 0 {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            } else {
                assert_eq!(ki.event.physical_key, KeyCode::KeyQ);
                assert_eq!(ki.event.logical_key, WKey::Character("a"));
                if i == 2 {
                    assert_eq!(ki.event.text, Some("â"));
                } else {
                    assert_eq!(ki.event.text, None);
                }
                assert_eq!(ki.event.location, KeyLocation::Standard);
                #[cfg(have_mod_supplement)]
                {
                    assert_eq!(
                        ki.event.mod_supplement.key_without_modifiers,
                        WKey::Character("a")
                    );
                    if i == 2 {
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            Some("â")
                        );
                    } else {
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                }
            }
            if matches!(i, 0 | 2) {
                assert_eq!(ki.event.state, ElementState::Pressed);
            } else {
                assert_eq!(ki.event.state, ElementState::Released);
            }
        }
    }
    {
        log::info!("Testing Â (KeyLeftbrace, KeyShiftLeft, KeyQ)");
        // LeftBrace Press
        // LeftBrace Release
        // LeftShift Press
        // Q Press
        // Q Release
        // LeftShift Release
        {
            kb.press(KeyLeftbrace);
            let _shift = kb.press(KeyLeftshift);
            kb.press(KeyQ);
        }
        // 0: LeftBrace pressed
        // 1: LeftBrace released
        // 2: LeftShift pressed
        // 3: ModifiersChanged
        // 4: Q pressed
        // 5: Q released
        // 6: LeftShift released
        // 7: ModifiersChanged
        for i in 0..8 {
            if matches!(i, 3 | 7) {
                let (_, mc) = events.window_modifiers().await;
                if i == 3 {
                    assert_eq!(mc, ModifiersState::SHIFT);
                } else {
                    assert_eq!(mc, ModifiersState::empty());
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                if matches!(i, 0 | 1) {
                    assert_eq!(ki.event.physical_key, KeyCode::BracketLeft);
                    assert_eq!(ki.event.logical_key, WKey::Dead(Some('^')));
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Dead(Some('^'))
                        );
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else if matches!(i, 2 | 6) {
                    assert_eq!(ki.event.physical_key, KeyCode::ShiftLeft);
                    assert_eq!(ki.event.logical_key, WKey::Shift);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Left);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Shift);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else {
                    assert_eq!(ki.event.physical_key, KeyCode::KeyQ);
                    assert_eq!(ki.event.logical_key, WKey::Character("A"));
                    if i == 4 {
                        assert_eq!(ki.event.text, Some("Â"));
                    } else {
                        assert_eq!(ki.event.text, None);
                    }
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Character("a")
                        );
                        if i == 4 {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                Some("Â")
                            );
                        } else {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                None
                            );
                        }
                    }
                }
                if matches!(i, 0 | 2 | 4) {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }
    {
        log::info!("Testing Ctrl-â (KeyLeftctrl, KeyLeftbrace, KeyQ)");
        // LeftCtrl Press
        // LeftBrace Press
        // LeftBrace Release
        // Q Press
        // Q Release
        // LeftCtrl Release
        {
            let _ctrl = kb.press(KeyLeftctrl);
            kb.press(KeyLeftbrace);
            kb.press(KeyQ);
        }
        // 0: LeftCtrl pressed
        // 1: ModifiersChanged
        // 2: LeftBrace pressed
        // 3: LeftBrace released
        // 4: Q pressed
        // 5: Q released
        // 6: LeftCtrl released
        // 7: ModifiersChanged
        for i in 0..8 {
            if matches!(i, 1 | 7) {
                let (_, mc) = events.window_modifiers().await;
                if i == 1 {
                    assert_eq!(mc, ModifiersState::CONTROL);
                } else {
                    assert_eq!(mc, ModifiersState::empty());
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                if matches!(i, 2 | 3) {
                    assert_eq!(ki.event.physical_key, KeyCode::BracketLeft);
                    assert_eq!(ki.event.logical_key, WKey::Dead(Some('^')));
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Dead(Some('^'))
                        );
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else if matches!(i, 0 | 6) {
                    assert_eq!(ki.event.physical_key, KeyCode::ControlLeft);
                    assert_eq!(ki.event.logical_key, WKey::Control);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Left);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Control);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else {
                    assert_eq!(ki.event.physical_key, KeyCode::KeyQ);
                    assert_eq!(ki.event.logical_key, WKey::Character("a"));
                    if i == 4 {
                        assert_eq!(ki.event.text, Some("â"));
                    } else {
                        assert_eq!(ki.event.text, None);
                    }
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Character("a")
                        );
                        if i == 4 {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                Some("â")
                            );
                        } else {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                None
                            );
                        }
                    }
                }
                if matches!(i, 0 | 2 | 4) {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }

    log::info!("Switching to QwertySwapped layout.");
    seat.set_layout(Layout::QwertySwapped);

    {
        log::info!("Testing CapsLock Q (KeyEsc, KeyQ)");
        // Esc Press
        // Esc Release
        // Q Press
        // Q Release
        // Esc Press
        // Esc Release
        {
            kb.press(KeyEsc);
            kb.press(KeyQ);
            kb.press(KeyEsc);
        }
        // 0: Esc pressed
        // 1: Esc released
        // 2: Q pressed
        // 3: Q released
        // 4: Esc pressed
        // 5: Esc released
        for i in 0..6 {
            let (_, ki) = events.window_keyboard_input().await;
            if matches!(i, 0 | 1 | 4 | 5) {
                assert_eq!(ki.event.physical_key, KeyCode::Escape);
                assert_eq!(ki.event.logical_key, WKey::CapsLock);
                assert_eq!(ki.event.text, None);
                assert_eq!(ki.event.location, KeyLocation::Standard);
                #[cfg(have_mod_supplement)]
                {
                    assert_eq!(
                        ki.event.mod_supplement.key_without_modifiers,
                        WKey::CapsLock
                    );
                    assert_eq!(
                        ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                        None
                    );
                }
            } else {
                assert_eq!(ki.event.physical_key, KeyCode::KeyQ);
                assert_eq!(ki.event.logical_key, WKey::Character("Q"));
                if i == 2 {
                    assert_eq!(ki.event.text, Some("Q"));
                } else {
                    assert_eq!(ki.event.text, None);
                }
                assert_eq!(ki.event.location, KeyLocation::Standard);
                #[cfg(have_mod_supplement)]
                {
                    assert_eq!(
                        ki.event.mod_supplement.key_without_modifiers,
                        WKey::Character("q")
                    );
                    if i == 2 {
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            Some("Q")
                        );
                    } else {
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                }
            }
            if matches!(i, 0 | 2 | 4) {
                assert_eq!(ki.event.state, ElementState::Pressed);
            } else {
                assert_eq!(ki.event.state, ElementState::Released);
            }
        }
    }

    {
        log::info!("Testing Shift Q (KeyLeftshift, KeyQ)");
        // Leftshift Press
        // Q Press
        // Q Release
        // Leftshift Release
        {
            let _shift = kb.press(KeyLeftshift);
            kb.press(KeyQ);
        }
        // 0: Leftshift pressed
        // 1: ModifiersChanged
        // 2: Q pressed
        // 3: Q released
        // 4: Leftshift released
        // 5: ModifiersChanged
        for i in 0..6 {
            if matches!(i, 1 | 5) {
                let (_, mc) = events.window_modifiers().await;
                if i == 1 {
                    assert_eq!(mc, ModifiersState::SHIFT);
                } else {
                    assert_eq!(mc, ModifiersState::empty());
                }
            } else {
                let (_, ki) = events.window_keyboard_input().await;
                if matches!(i, 0 | 4) {
                    // assert_eq!(ki.event.physical_key, KeyCode::ShiftRight);
                    assert_eq!(ki.event.physical_key, KeyCode::ShiftLeft);
                    assert_eq!(ki.event.logical_key, WKey::Shift);
                    assert_eq!(ki.event.text, None);
                    assert_eq!(ki.event.location, KeyLocation::Right);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(ki.event.mod_supplement.key_without_modifiers, WKey::Shift);
                        assert_eq!(
                            ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                            None
                        );
                    }
                } else {
                    assert_eq!(ki.event.physical_key, KeyCode::KeyQ);
                    assert_eq!(ki.event.logical_key, WKey::Character("Q"));
                    if i == 2 {
                        assert_eq!(ki.event.text, Some("Q"));
                    } else {
                        assert_eq!(ki.event.text, None);
                    }
                    assert_eq!(ki.event.location, KeyLocation::Standard);
                    #[cfg(have_mod_supplement)]
                    {
                        assert_eq!(
                            ki.event.mod_supplement.key_without_modifiers,
                            WKey::Character("q")
                        );
                        if i == 2 {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                Some("Q")
                            );
                        } else {
                            assert_eq!(
                                ki.event.mod_supplement.text_with_all_modifiers.as_deref(),
                                None
                            );
                        }
                    }
                }
                if matches!(i, 0 | 2) {
                    assert_eq!(ki.event.state, ElementState::Pressed);
                } else {
                    assert_eq!(ki.event.state, ElementState::Released);
                }
            }
        }
    }
}
