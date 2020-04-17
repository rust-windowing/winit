//use std::{rc::Rc, cell::Cell};
pub use smithay_client_toolkit::{reexports::client::protocol::wl_surface::WlSurface, seat::keyboard::{self, KeyState}};
use {crate::event::{ElementState, ModifiersState}, super::conversion};

// Track modifiers and key repetition
#[derive(Default)] pub struct Keyboard {
    focus : Option<WlSurface>,
    modifiers : ModifiersState,
    // Would be async repeat but using calloop sctk map_keyboard_repeat for now
    // repeat : Option<Rc<Cell<crate::event::WindowEvent<'static>>>>,
}

impl Keyboard {
    pub fn handle(&mut self, mut send: impl FnMut(crate::event::WindowEvent, &WlSurface), event: keyboard::Event, is_synthetic: bool) {
        let Self{focus, modifiers/*, repeat*/} = self;
        let device_id = crate::event::DeviceId(super::super::DeviceId::Wayland(super::DeviceId));
        use {keyboard::Event::*, crate::event::WindowEvent::*};
        match event {
            Enter { surface, .. } => {
                send(Focused(true), &surface);
                /*if !modifiers.is_empty() ?*/ {
                    send(ModifiersChanged(*modifiers), &surface);
                }
                *focus = Some(surface);
            }
            Leave { surface, .. } => {
                // Would be async repeat but using calloop sctk map_keyboard_repeat for now
                //*repeat = None; // will drop the timer on its next event (Weak::upgrade=None)
                /*if !modifiers.is_empty() {
                    send(ModifiersChanged(ModifiersState::empty()), surface);
                }*/
                send(Focused(false), &surface);
                *focus = None;
            }
            ref key @ Key{ rawkey, state, ref utf8, .. } => if let Some(focus) = focus /*=>*/ {
                /*//Would be async repeat but using sctk calloop repeat for now
                if state == KeyState::Pressed {
                    if let Some(repeat) = repeat { // Update existing repeat cell (also triggered by the actual repetition => noop)
                        repeat.set(event);
                        // Note: This keeps the same timer on key repeat change. No delay! Nice!
                    } else { // New repeat timer (registers in the reactor on first poll)
                        //assert!(!is_repeat);
                        let repeat = Rc::new(Cell::new(event));
                        use futures::stream;
                        streams.get_mut().push(
                            stream::unfold(Instant::now()+Duration::from_millis(300), {
                                let repeat = Rc::downgrade(&repeat);
                                |last| {
                                    let next = last+Duration::from_millis(100);
                                    smol::Timer::at(next).map(move |_| { repeat.upgrade().map(|x| x.clone().into_inner() ) }) // Option<Key> (None stops the stream, autodrops from streams)
                                }
                            })
                            .map(|(item, _t)| item)
                        );
                        repeat = Some(Cell::new(event));
                    }
                } else {
                    if repeat.filter(|r| r.get()==event).is_some() { repeat = None }
                }*/
                send(
                    #[allow(deprecated)]
                    KeyboardInput {
                        device_id,
                        input: crate::event::KeyboardInput {
                            state: if state == KeyState::Pressed { ElementState::Pressed } else { ElementState::Released },
                            scancode: rawkey,
                            virtual_keycode: conversion::key(key),
                            modifiers: *modifiers,
                        },
                        is_synthetic,
                    },
                    focus,
                );
                if let Some(txt) = utf8 {
                    for char in txt.chars() {
                        send(ReceivedCharacter(char), focus);
                    }
                }
            }
            Modifiers { modifiers: new_modifiers, .. } => if let Some(focus) = focus /*=>*/ {
                *modifiers = conversion::modifiers(new_modifiers);
                send(ModifiersChanged(*modifiers), focus);
            }
            Repeat {..} => {}, // fixme
        }
    }
}
