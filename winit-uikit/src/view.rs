#![allow(clippy::unnecessary_cast)]
use std::cell::{Cell, RefCell};

use dpi::PhysicalPosition;
use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, available, define_class, msg_send, sel};
use objc2_core_foundation::{CGFloat, CGPoint, CGRect};
use objc2_foundation::{NSObject, NSSet, NSString};
use objc2_ui_kit::{
    UIEvent, UIForceTouchCapability, UIGestureRecognizer, UIGestureRecognizerDelegate,
    UIGestureRecognizerState, UIKeyInput, UIPanGestureRecognizer, UIPinchGestureRecognizer,
    UIResponder, UIRotationGestureRecognizer, UITapGestureRecognizer, UITextInputTraits, UITouch,
    UITouchPhase, UITouchType, UITraitEnvironment, UIView,
};
use tracing::debug;
use winit_core::event::{
    ButtonSource, ElementState, FingerId, Force, KeyEvent, PointerKind, PointerSource, TouchPhase,
    WindowEvent,
};
use winit_core::keyboard::{Key, KeyCode, KeyLocation, NamedKey, NativeKeyCode, PhysicalKey};

use super::app_state::{self, EventWrapper};
use super::window::WinitUIWindow;

pub struct WinitViewState {
    pinch_gesture_recognizer: RefCell<Option<Retained<UIPinchGestureRecognizer>>>,
    doubletap_gesture_recognizer: RefCell<Option<Retained<UITapGestureRecognizer>>>,
    rotation_gesture_recognizer: RefCell<Option<Retained<UIRotationGestureRecognizer>>>,
    pan_gesture_recognizer: RefCell<Option<Retained<UIPanGestureRecognizer>>>,

    // for iOS delta references the start of the Gesture
    rotation_last_delta: Cell<CGFloat>,
    pinch_last_delta: Cell<CGFloat>,
    pan_last_delta: Cell<CGPoint>,

    primary_finger: Cell<Option<FingerId>>,
    fingers: Cell<u8>,
}

define_class!(
    #[unsafe(super(UIView, UIResponder, NSObject))]
    #[name = "WinitUIView"]
    #[ivars = WinitViewState]
    pub(crate) struct WinitView;

    /// This documentation attribute makes rustfmt work for some reason?
    impl WinitView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, rect: CGRect) {
            let mtm = MainThreadMarker::new().unwrap();
            let window = self.window().unwrap();
            app_state::handle_nonuser_event(mtm, EventWrapper::Window {
                window_id: window.id(),
                event: WindowEvent::RedrawRequested,
            });
            let _: () = unsafe { msg_send![super(self), drawRect: rect] };
        }

        #[unsafe(method(layoutSubviews))]
        fn layout_subviews(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };

            let frame = self.frame();
            let scale_factor = self.contentScaleFactor() as f64;
            let size = dpi::LogicalSize {
                width: frame.size.width as f64,
                height: frame.size.height as f64,
            }
            .to_physical(scale_factor);

            let window = self.window().unwrap();
            app_state::handle_nonuser_event(mtm, EventWrapper::Window {
                window_id: window.id(),
                event: WindowEvent::SurfaceResized(size),
            });
        }

        #[unsafe(method(setContentScaleFactor:))]
        fn set_content_scale_factor(&self, untrusted_scale_factor: CGFloat) {
            let mtm = MainThreadMarker::new().unwrap();
            let _: () =
                unsafe { msg_send![super(self), setContentScaleFactor: untrusted_scale_factor] };

            // `window` is null when `setContentScaleFactor` is invoked prior to `[UIWindow
            // makeKeyAndVisible]` at window creation time (either manually or internally by
            // UIKit when the `UIView` is first created), in which case we send no events here
            let window = match self.window() {
                Some(window) => window,
                None => return,
            };
            // `setContentScaleFactor` may be called with a value of 0, which means "reset the
            // content scale factor to a device-specific default value", so we can't use the
            // parameter here. We can query the actual factor using the getter
            let scale_factor = self.contentScaleFactor();
            assert!(
                !scale_factor.is_nan()
                    && scale_factor.is_finite()
                    && scale_factor.is_sign_positive()
                    && scale_factor > 0.0,
                "invalid scale_factor set on UIView",
            );
            let scale_factor = scale_factor as f64;
            let frame = self.frame();
            let size = dpi::LogicalSize {
                width: frame.size.width as f64,
                height: frame.size.height as f64,
            };
            let window_id = window.id();
            app_state::handle_nonuser_events(
                mtm,
                std::iter::once(EventWrapper::ScaleFactorChanged(app_state::ScaleFactorChanged {
                    window,
                    scale_factor,
                    suggested_size: size.to_physical(scale_factor),
                }))
                .chain(std::iter::once(EventWrapper::Window {
                    window_id,
                    event: WindowEvent::SurfaceResized(size.to_physical(scale_factor)),
                })),
            );
        }

        #[unsafe(method(safeAreaInsetsDidChange))]
        fn safe_area_changed(&self) {
            debug!("safeAreaInsetsDidChange was called, requesting redraw");
            // When the safe area changes we want to make sure to emit a redraw event
            self.setNeedsDisplay();
        }

        #[unsafe(method(touchesBegan:withEvent:))]
        fn touches_began(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[unsafe(method(touchesMoved:withEvent:))]
        fn touches_moved(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[unsafe(method(touchesEnded:withEvent:))]
        fn touches_ended(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[unsafe(method(touchesCancelled:withEvent:))]
        fn touches_cancelled(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[unsafe(method(pinchGesture:))]
        fn pinch_gesture(&self, recognizer: &UIPinchGestureRecognizer) {
            let window = self.window().unwrap();

            let (phase, delta) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().pinch_last_delta.set(recognizer.scale());
                    (TouchPhase::Started, 0.0)
                },
                UIGestureRecognizerState::Changed => {
                    let last_scale: f64 = self.ivars().pinch_last_delta.replace(recognizer.scale());
                    (TouchPhase::Moved, recognizer.scale() - last_scale)
                },
                UIGestureRecognizerState::Ended => {
                    let last_scale: f64 = self.ivars().pinch_last_delta.replace(0.0);
                    (TouchPhase::Moved, recognizer.scale() - last_scale)
                },
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    self.ivars().rotation_last_delta.set(0.0);
                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -recognizer.scale())
                },
                state => panic!("unexpected recognizer state: {state:?}"),
            };

            let gesture_event = EventWrapper::Window {
                window_id: window.id(),
                event: WindowEvent::PinchGesture { device_id: None, delta: delta as f64, phase },
            };

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[unsafe(method(doubleTapGesture:))]
        fn double_tap_gesture(&self, recognizer: &UITapGestureRecognizer) {
            let window = self.window().unwrap();

            if recognizer.state() == UIGestureRecognizerState::Ended {
                let gesture_event = EventWrapper::Window {
                    window_id: window.id(),
                    event: WindowEvent::DoubleTapGesture { device_id: None },
                };

                let mtm = MainThreadMarker::new().unwrap();
                app_state::handle_nonuser_event(mtm, gesture_event);
            }
        }

        #[unsafe(method(rotationGesture:))]
        fn rotation_gesture(&self, recognizer: &UIRotationGestureRecognizer) {
            let window = self.window().unwrap();

            let (phase, delta) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().rotation_last_delta.set(0.0);

                    (TouchPhase::Started, 0.0)
                },
                UIGestureRecognizerState::Changed => {
                    let last_rotation =
                        self.ivars().rotation_last_delta.replace(recognizer.rotation());

                    (TouchPhase::Moved, recognizer.rotation() - last_rotation)
                },
                UIGestureRecognizerState::Ended => {
                    let last_rotation = self.ivars().rotation_last_delta.replace(0.0);

                    (TouchPhase::Ended, recognizer.rotation() - last_rotation)
                },
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    self.ivars().rotation_last_delta.set(0.0);

                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -recognizer.rotation())
                },
                state => panic!("unexpected recognizer state: {state:?}"),
            };

            // Make delta negative to match macos, convert to degrees
            let gesture_event = EventWrapper::Window {
                window_id: window.id(),
                event: WindowEvent::RotationGesture {
                    device_id: None,
                    delta: -delta.to_degrees() as _,
                    phase,
                },
            };

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[unsafe(method(panGesture:))]
        fn pan_gesture(&self, recognizer: &UIPanGestureRecognizer) {
            let window = self.window().unwrap();

            let translation = recognizer.translationInView(Some(self));

            let (phase, dx, dy) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().pan_last_delta.set(translation);

                    (TouchPhase::Started, 0.0, 0.0)
                },
                UIGestureRecognizerState::Changed => {
                    let last_pan: CGPoint = self.ivars().pan_last_delta.replace(translation);

                    let dx = translation.x - last_pan.x;
                    let dy = translation.y - last_pan.y;

                    (TouchPhase::Moved, dx, dy)
                },
                UIGestureRecognizerState::Ended => {
                    let last_pan: CGPoint =
                        self.ivars().pan_last_delta.replace(CGPoint { x: 0.0, y: 0.0 });

                    let dx = translation.x - last_pan.x;
                    let dy = translation.y - last_pan.y;

                    (TouchPhase::Ended, dx, dy)
                },
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    let last_pan: CGPoint =
                        self.ivars().pan_last_delta.replace(CGPoint { x: 0.0, y: 0.0 });

                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -last_pan.x, -last_pan.y)
                },
                state => panic!("unexpected recognizer state: {state:?}"),
            };

            let gesture_event = EventWrapper::Window {
                window_id: window.id(),
                event: WindowEvent::PanGesture {
                    device_id: None,
                    delta: PhysicalPosition::new(dx as _, dy as _),
                    phase,
                },
            };

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[unsafe(method(canBecomeFirstResponder))]
        fn can_become_first_responder(&self) -> bool {
            true
        }
    }

    unsafe impl NSObjectProtocol for WinitView {}

    unsafe impl UIGestureRecognizerDelegate for WinitView {
        #[unsafe(method(gestureRecognizer:shouldRecognizeSimultaneouslyWithGestureRecognizer:))]
        fn should_recognize_simultaneously(
            &self,
            _gesture_recognizer: &UIGestureRecognizer,
            _other_gesture_recognizer: &UIGestureRecognizer,
        ) -> bool {
            true
        }
    }

    unsafe impl UITextInputTraits for WinitView {}

    unsafe impl UIKeyInput for WinitView {
        #[unsafe(method(hasText))]
        fn has_text(&self) -> bool {
            true
        }

        #[unsafe(method(insertText:))]
        fn insert_text(&self, text: &NSString) {
            self.handle_insert_text(text)
        }

        #[unsafe(method(deleteBackward))]
        fn delete_backward(&self) {
            self.handle_delete_backward()
        }
    }
);

impl WinitView {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        scale_factor: Option<f64>,
        frame: CGRect,
    ) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(WinitViewState {
            pinch_gesture_recognizer: RefCell::new(None),
            doubletap_gesture_recognizer: RefCell::new(None),
            rotation_gesture_recognizer: RefCell::new(None),
            pan_gesture_recognizer: RefCell::new(None),

            rotation_last_delta: Cell::new(0.0),
            pinch_last_delta: Cell::new(0.0),
            pan_last_delta: Cell::new(CGPoint { x: 0.0, y: 0.0 }),

            primary_finger: Cell::new(None),
            fingers: Cell::new(0),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };

        this.setMultipleTouchEnabled(true);

        if let Some(scale_factor) = scale_factor {
            this.setContentScaleFactor(scale_factor as _);
        }

        this
    }

    fn window(&self) -> Option<Retained<WinitUIWindow>> {
        // `WinitView`s should always be installed in a `WinitUIWindow`
        (**self).window().map(|window| window.downcast().unwrap())
    }

    pub(crate) fn recognize_pinch_gesture(&self, should_recognize: bool) {
        let mtm = MainThreadMarker::from(self);
        if should_recognize {
            if self.ivars().pinch_gesture_recognizer.borrow().is_none() {
                let pinch = unsafe {
                    UIPinchGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(pinchGesture:)),
                    )
                };
                pinch.setDelegate(Some(ProtocolObject::from_ref(self)));
                self.addGestureRecognizer(&pinch);
                self.ivars().pinch_gesture_recognizer.replace(Some(pinch));
            }
        } else if let Some(recognizer) = self.ivars().pinch_gesture_recognizer.take() {
            self.removeGestureRecognizer(&recognizer);
        }
    }

    pub(crate) fn recognize_pan_gesture(
        &self,
        should_recognize: bool,
        minimum_number_of_touches: u8,
        maximum_number_of_touches: u8,
    ) {
        let mtm = MainThreadMarker::from(self);
        if should_recognize {
            if self.ivars().pan_gesture_recognizer.borrow().is_none() {
                let pan = unsafe {
                    UIPanGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(panGesture:)),
                    )
                };
                pan.setDelegate(Some(ProtocolObject::from_ref(self)));
                pan.setMinimumNumberOfTouches(minimum_number_of_touches as _);
                pan.setMaximumNumberOfTouches(maximum_number_of_touches as _);
                self.addGestureRecognizer(&pan);
                self.ivars().pan_gesture_recognizer.replace(Some(pan));
            }
        } else if let Some(recognizer) = self.ivars().pan_gesture_recognizer.take() {
            self.removeGestureRecognizer(&recognizer);
        }
    }

    pub(crate) fn recognize_doubletap_gesture(&self, should_recognize: bool) {
        let mtm = MainThreadMarker::from(self);
        if should_recognize {
            if self.ivars().doubletap_gesture_recognizer.borrow().is_none() {
                let tap = unsafe {
                    UITapGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(doubleTapGesture:)),
                    )
                };
                tap.setDelegate(Some(ProtocolObject::from_ref(self)));
                tap.setNumberOfTapsRequired(2);
                tap.setNumberOfTouchesRequired(1);
                self.addGestureRecognizer(&tap);
                self.ivars().doubletap_gesture_recognizer.replace(Some(tap));
            }
        } else if let Some(recognizer) = self.ivars().doubletap_gesture_recognizer.take() {
            self.removeGestureRecognizer(&recognizer);
        }
    }

    pub(crate) fn recognize_rotation_gesture(&self, should_recognize: bool) {
        let mtm = MainThreadMarker::from(self);
        if should_recognize {
            if self.ivars().rotation_gesture_recognizer.borrow().is_none() {
                let rotation = unsafe {
                    UIRotationGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(rotationGesture:)),
                    )
                };
                rotation.setDelegate(Some(ProtocolObject::from_ref(self)));
                self.addGestureRecognizer(&rotation);
                self.ivars().rotation_gesture_recognizer.replace(Some(rotation));
            }
        } else if let Some(recognizer) = self.ivars().rotation_gesture_recognizer.take() {
            self.removeGestureRecognizer(&recognizer);
        }
    }

    fn handle_touches(&self, touches: &NSSet<UITouch>) {
        let window = self.window().unwrap();
        let mut touch_events = Vec::new();
        for touch in touches {
            if let UITouchType::Pencil = touch.r#type() {
                continue;
            }

            let logical_location = touch.locationInView(None);
            let touch_type = touch.r#type();
            let force = if let UITouchType::Pencil = touch_type {
                None
            } else if available!(ios = 9.0, tvos = 9.0, visionos = 1.0) {
                let trait_collection = self.traitCollection();
                let touch_capability = trait_collection.forceTouchCapability();
                // Both the OS _and_ the device need to be checked for force touch support.
                if touch_capability == UIForceTouchCapability::Available {
                    let force = touch.force();
                    let max_possible_force = touch.maximumPossibleForce();
                    Some(Force::Calibrated {
                        force: force as _,
                        max_possible_force: max_possible_force as _,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            let touch_id = Retained::as_ptr(&touch) as usize;
            let phase = touch.phase();
            let position = {
                let scale_factor = self.contentScaleFactor();
                PhysicalPosition::from_logical::<(f64, f64), f64>(
                    (logical_location.x as _, logical_location.y as _),
                    scale_factor as f64,
                )
            };
            let window_id = window.id();
            let finger_id = FingerId::from_raw(touch_id);

            let ivars = self.ivars();

            match phase {
                UITouchPhase::Began => {
                    let primary = if let UITouchType::Pencil = touch_type {
                        true
                    } else {
                        ivars.fingers.set(ivars.fingers.get() + 1);
                        // Keep the primary finger around until we clear all the fingers to
                        // recognize it when user briefly removes it.
                        match ivars.primary_finger.get() {
                            Some(primary_id) => primary_id == finger_id,
                            None => {
                                debug_assert_eq!(
                                    ivars.fingers.get(),
                                    1,
                                    "number of fingers were not counted correctly"
                                );
                                ivars.primary_finger.set(Some(finger_id));
                                true
                            },
                        }
                    };

                    touch_events.push(EventWrapper::Window {
                        window_id,
                        event: WindowEvent::PointerEntered {
                            device_id: None,
                            primary,
                            position,
                            kind: if let UITouchType::Pencil = touch_type {
                                PointerKind::Unknown
                            } else {
                                PointerKind::Touch(finger_id)
                            },
                        },
                    });
                    touch_events.push(EventWrapper::Window {
                        window_id,
                        event: WindowEvent::PointerButton {
                            device_id: None,
                            primary,
                            state: ElementState::Pressed,
                            position,
                            button: if let UITouchType::Pencil = touch_type {
                                ButtonSource::Unknown(0)
                            } else {
                                ButtonSource::Touch { finger_id, force }
                            },
                        },
                    });
                },
                UITouchPhase::Moved => {
                    let (primary, source) = if let UITouchType::Pencil = touch_type {
                        (true, PointerSource::Unknown)
                    } else {
                        (ivars.primary_finger.get().unwrap() == finger_id, PointerSource::Touch {
                            finger_id,
                            force,
                        })
                    };

                    touch_events.push(EventWrapper::Window {
                        window_id,
                        event: WindowEvent::PointerMoved {
                            device_id: None,
                            primary,
                            position,
                            source,
                        },
                    });
                },
                // 2 is UITouchPhase::Stationary and is not expected here
                UITouchPhase::Ended | UITouchPhase::Cancelled => {
                    let primary = if let UITouchType::Pencil = touch_type {
                        true
                    } else {
                        ivars.fingers.set(ivars.fingers.get() - 1);
                        let primary = ivars.primary_finger.get().unwrap() == finger_id;
                        if ivars.fingers.get() == 0 {
                            ivars.primary_finger.set(None);
                        }
                        primary
                    };

                    if let UITouchPhase::Ended = phase {
                        touch_events.push(EventWrapper::Window {
                            window_id,
                            event: WindowEvent::PointerButton {
                                device_id: None,
                                primary,
                                state: ElementState::Released,
                                position,
                                button: if let UITouchType::Pencil = touch_type {
                                    ButtonSource::Unknown(0)
                                } else {
                                    ButtonSource::Touch { finger_id, force }
                                },
                            },
                        });
                    }

                    touch_events.push(EventWrapper::Window {
                        window_id,
                        event: WindowEvent::PointerLeft {
                            device_id: None,
                            primary,
                            position: Some(position),
                            kind: if let UITouchType::Pencil = touch_type {
                                PointerKind::Unknown
                            } else {
                                PointerKind::Touch(finger_id)
                            },
                        },
                    });
                },
                _ => panic!("unexpected touch phase: {phase:?}"),
            }
        }
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(mtm, touch_events);
    }

    fn handle_insert_text(&self, text: &NSString) {
        let window = self.window().unwrap();
        let window_id = window.id();
        let mtm = MainThreadMarker::new().unwrap();
        // send individual events for each character
        app_state::handle_nonuser_events(
            mtm,
            text.to_string().chars().flat_map(|c| {
                let text = smol_str::SmolStr::from_iter([c]);
                // Emit both press and release events
                [ElementState::Pressed, ElementState::Released].map(|state| EventWrapper::Window {
                    window_id,
                    event: WindowEvent::KeyboardInput {
                        device_id: None,
                        event: KeyEvent {
                            text: if state == ElementState::Pressed {
                                Some(text.clone())
                            } else {
                                None
                            },
                            state,
                            location: KeyLocation::Standard,
                            repeat: false,
                            logical_key: Key::Character(text.clone()),
                            physical_key: PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
                            text_with_all_modifiers: if state == ElementState::Pressed {
                                Some(text.clone())
                            } else {
                                None
                            },
                            key_without_modifiers: Key::Character(text.clone()),
                        },
                        is_synthetic: false,
                    },
                })
            }),
        );
    }

    fn handle_delete_backward(&self) {
        let window = self.window().unwrap();
        let window_id = window.id();
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(
            mtm,
            [ElementState::Pressed, ElementState::Released].map(|state| EventWrapper::Window {
                window_id,
                event: WindowEvent::KeyboardInput {
                    device_id: None,
                    event: KeyEvent {
                        state,
                        logical_key: Key::Named(NamedKey::Backspace),
                        physical_key: PhysicalKey::Code(KeyCode::Backspace),
                        repeat: false,
                        location: KeyLocation::Standard,
                        text: None,
                        text_with_all_modifiers: None,
                        key_without_modifiers: Key::Named(NamedKey::Backspace),
                    },
                    is_synthetic: false,
                },
            }),
        );
    }
}
