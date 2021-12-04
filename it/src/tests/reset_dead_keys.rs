use crate::backend::Instance;
use crate::keyboard::Key::{KeyLeftbrace, KeyQ};
use crate::keyboard::Layout;
use winit::event::ElementState;
use winit::keyboard::{Key as WKey, KeyCode, KeyLocation};

test!(run);

async fn run(instance: &dyn Instance) {
    let el = instance.create_event_loop();
    let mut events = el.events();
    let window = el.create_window(Default::default());
    window.mapped(true).await;
    let seat = instance.default_seat();
    seat.focus(&*window);
    let kb = seat.add_keyboard();
    seat.set_layout(Layout::Azerty);

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
        log::info!("Testing dead key reset");
        // LeftBrace Press
        // LeftBrace Release
        {
            kb.press(KeyLeftbrace);
        }
        // 0: LeftBrace pressed
        // 1: LeftBrace released
        for i in 0..2 {
            let (_, ki) = events.window_keyboard_input().await;
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
        }
        window.reset_dead_keys();
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
            if matches!(i, 0) {
                assert_eq!(ki.event.state, ElementState::Pressed);
            } else {
                assert_eq!(ki.event.state, ElementState::Released);
            }
        }
    }
}
