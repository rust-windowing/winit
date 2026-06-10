#![allow(clippy::unnecessary_cast)]
use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_foundation::{
    CGFloat, CGPoint, CGRect, CGSize, MainThreadMarker, NSArray, NSAttributedStringKey,
    NSComparisonResult, NSDictionary, NSInteger, NSObject, NSRange, NSSet, NSString,
};
use objc2_ui_kit::{
    UICoordinateSpace, UIEvent, UIForceTouchCapability, UIGestureRecognizer,
    UIGestureRecognizerDelegate, UIGestureRecognizerState, UIKeyInput, UIPanGestureRecognizer,
    UIPinchGestureRecognizer, UIResponder, UIRotationGestureRecognizer, UITapGestureRecognizer,
    UITextInput, UITextInputDelegate, UITextInputStringTokenizer, UITextInputTokenizer,
    UITextInputTraits, UITextLayoutDirection, UITextPosition, UITextRange, UITextSelectionRect,
    UITouch, UITouchPhase, UITouchType, UITraitEnvironment, UIView,
};

use super::app_state::{self, EventWrapper};
use super::ime::{ImeState, WinitTextPosition, WinitTextRange};
use super::window::WinitUIWindow;
use crate::dpi::PhysicalPosition;
use crate::event::{ElementState, Event, Force, Ime, KeyEvent, Touch, TouchPhase, WindowEvent};
use crate::keyboard::{Key, KeyCode, KeyLocation, NamedKey, NativeKeyCode, PhysicalKey};
use crate::platform_impl::platform::DEVICE_ID;
use crate::platform_impl::KeyEventExtra;
use crate::window::{WindowAttributes, WindowId as RootWindowId};

pub struct WinitViewState {
    pinch_gesture_recognizer: RefCell<Option<Retained<UIPinchGestureRecognizer>>>,
    doubletap_gesture_recognizer: RefCell<Option<Retained<UITapGestureRecognizer>>>,
    rotation_gesture_recognizer: RefCell<Option<Retained<UIRotationGestureRecognizer>>>,
    pan_gesture_recognizer: RefCell<Option<Retained<UIPanGestureRecognizer>>>,

    // for iOS delta references the start of the Gesture
    rotation_last_delta: Cell<CGFloat>,
    pinch_last_delta: Cell<CGFloat>,
    pan_last_delta: Cell<CGPoint>,

    // Active IME / marked-text composition state. Driven by the `UITextInput`
    // protocol methods so that multi-stage input methods (Chinese pinyin,
    // Japanese kana, Korean hangul, ...) can be received.
    ime: RefCell<ImeState>,
}

declare_class!(
    pub(crate) struct WinitView;

    unsafe impl ClassType for WinitView {
        #[inherits(UIResponder, NSObject)]
        type Super = UIView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitUIView";
    }

    impl DeclaredClass for WinitView {
        type Ivars = WinitViewState;
    }

    unsafe impl WinitView {
        #[method(drawRect:)]
        fn draw_rect(&self, rect: CGRect) {
            let mtm = MainThreadMarker::new().unwrap();
            let window = self.window().unwrap();
            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::RedrawRequested,
                }),
            );
            let _: () = unsafe { msg_send![super(self), drawRect: rect] };
        }

        #[method(layoutSubviews)]
        fn layout_subviews(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };

            let window = self.window().unwrap();
            let window_bounds = window.bounds();
            let screen = window.screen();
            let screen_space = screen.coordinateSpace();
            let screen_frame = self.convertRect_toCoordinateSpace(window_bounds, &screen_space);
            let scale_factor = screen.scale();
            let size = crate::dpi::LogicalSize {
                width: screen_frame.size.width as f64,
                height: screen_frame.size.height as f64,
            }
            .to_physical(scale_factor as f64);

            // If the app is started in landscape, the view frame and window bounds can be mismatched.
            // The view frame will be in portrait and the window bounds in landscape. So apply the
            // window bounds to the view frame to make it consistent.
            let view_frame = self.frame();
            if view_frame != window_bounds {
                self.setFrame(window_bounds);
            }

            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::Resized(size),
                }),
            );
        }

        #[method(setContentScaleFactor:)]
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
            let bounds = self.bounds();
            let screen = window.screen();
            let screen_space = screen.coordinateSpace();
            let screen_frame = self.convertRect_toCoordinateSpace(bounds, &screen_space);
            let size = crate::dpi::LogicalSize {
                width: screen_frame.size.width as f64,
                height: screen_frame.size.height as f64,
            };
            let window_id = RootWindowId(window.id());
            app_state::handle_nonuser_events(
                mtm,
                std::iter::once(EventWrapper::ScaleFactorChanged(
                    app_state::ScaleFactorChanged {
                        window,
                        scale_factor,
                        suggested_size: size.to_physical(scale_factor),
                    },
                ))
                .chain(std::iter::once(EventWrapper::StaticEvent(
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::Resized(size.to_physical(scale_factor)),
                    },
                ))),
            );
        }

        #[method(touchesBegan:withEvent:)]
        fn touches_began(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesMoved:withEvent:)]
        fn touches_moved(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesEnded:withEvent:)]
        fn touches_ended(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesCancelled:withEvent:)]
        fn touches_cancelled(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(pinchGesture:)]
        fn pinch_gesture(&self, recognizer: &UIPinchGestureRecognizer) {
            let window = self.window().unwrap();

            let (phase, delta) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().pinch_last_delta.set(recognizer.scale());
                    (TouchPhase::Started, 0.0)
                }
                UIGestureRecognizerState::Changed => {
                    let last_scale: f64 = self.ivars().pinch_last_delta.replace(recognizer.scale());
                    (TouchPhase::Moved, recognizer.scale() - last_scale)
                }
                UIGestureRecognizerState::Ended => {
                    let last_scale: f64 = self.ivars().pinch_last_delta.replace(0.0);
                    (TouchPhase::Moved, recognizer.scale() - last_scale)
                }
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    self.ivars().rotation_last_delta.set(0.0);
                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -recognizer.scale())
                }
                state => panic!("unexpected recognizer state: {state:?}"),
            };

            let gesture_event = EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::PinchGesture {
                    device_id: DEVICE_ID,
                    delta: delta as f64,
                    phase,
                },
            });

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[method(doubleTapGesture:)]
        fn double_tap_gesture(&self, recognizer: &UITapGestureRecognizer) {
            let window = self.window().unwrap();

            if recognizer.state() == UIGestureRecognizerState::Ended {
                let gesture_event = EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::DoubleTapGesture {
                        device_id: DEVICE_ID,
                    },
                });

                let mtm = MainThreadMarker::new().unwrap();
                app_state::handle_nonuser_event(mtm, gesture_event);
            }
        }

        #[method(rotationGesture:)]
        fn rotation_gesture(&self, recognizer: &UIRotationGestureRecognizer) {
            let window = self.window().unwrap();

            let (phase, delta) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().rotation_last_delta.set(0.0);

                    (TouchPhase::Started, 0.0)
                }
                UIGestureRecognizerState::Changed => {
                    let last_rotation = self.ivars().rotation_last_delta.replace(recognizer.rotation());

                    (TouchPhase::Moved, recognizer.rotation() - last_rotation)
                }
                UIGestureRecognizerState::Ended => {
                    let last_rotation = self.ivars().rotation_last_delta.replace(0.0);

                    (TouchPhase::Ended, recognizer.rotation() - last_rotation)
                }
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    self.ivars().rotation_last_delta.set(0.0);

                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -recognizer.rotation())
                }
                state => panic!("unexpected recognizer state: {state:?}"),
            };

            // Make delta negative to match macos, convert to degrees
            let gesture_event = EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::RotationGesture {
                    device_id: DEVICE_ID,
                    delta: -delta.to_degrees() as _,
                    phase,
                },
            });

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[method(panGesture:)]
        fn pan_gesture(&self, recognizer: &UIPanGestureRecognizer) {
            let window = self.window().unwrap();

            let translation = recognizer.translationInView(Some(self));

            let (phase, dx, dy) = match recognizer.state() {
                UIGestureRecognizerState::Began => {
                    self.ivars().pan_last_delta.set(translation);

                    (TouchPhase::Started, 0.0, 0.0)
                }
                UIGestureRecognizerState::Changed => {
                    let last_pan: CGPoint = self.ivars().pan_last_delta.replace(translation);

                    let dx = translation.x - last_pan.x;
                    let dy = translation.y - last_pan.y;

                    (TouchPhase::Moved, dx, dy)
                }
                UIGestureRecognizerState::Ended => {
                    let last_pan: CGPoint = self.ivars().pan_last_delta.replace(CGPoint{x:0.0, y:0.0});

                    let dx = translation.x - last_pan.x;
                    let dy = translation.y - last_pan.y;

                    (TouchPhase::Ended, dx, dy)
                }
                UIGestureRecognizerState::Cancelled | UIGestureRecognizerState::Failed => {
                    let last_pan: CGPoint = self.ivars().pan_last_delta.replace(CGPoint{x:0.0, y:0.0});

                    // Pass -delta so that action is reversed
                    (TouchPhase::Cancelled, -last_pan.x, -last_pan.y)
                }
                state => panic!("unexpected recognizer state: {state:?}"),
            };


            let gesture_event = EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::PanGesture {
                    device_id: DEVICE_ID,
                    delta: PhysicalPosition::new(dx as _, dy as _),
                    phase,
                },
            });

            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, gesture_event);
        }

        #[method(canBecomeFirstResponder)]
        fn can_become_first_responder(&self) -> bool {
            true
        }
    }

    unsafe impl NSObjectProtocol for WinitView {}

    unsafe impl UIGestureRecognizerDelegate for WinitView {
        #[method(gestureRecognizer:shouldRecognizeSimultaneouslyWithGestureRecognizer:)]
        fn should_recognize_simultaneously(&self, _gesture_recognizer: &UIGestureRecognizer, _other_gesture_recognizer: &UIGestureRecognizer) -> bool {
            true
        }
    }

    unsafe impl UITextInputTraits for WinitView {
    }

    unsafe impl UIKeyInput for WinitView {
        #[method(hasText)]
        fn has_text(&self) -> bool {
            true
        }

        #[method(insertText:)]
        fn insert_text(&self, text: &NSString) {
            self.handle_insert_text(text)
        }

        #[method(deleteBackward)]
        fn delete_backward(&self) {
            self.handle_delete_backward()
        }
    }

    // ----------------------------------------------------------------------
    // UITextInput
    //
    // Apple requires that a view adopt *both* `UIKeyInput` *and* `UITextInput`
    // for multi-stage input methods (Chinese / Japanese / Korean / Vietnamese
    // / ...) to deliver any events at all. Without `UITextInput` the system
    // silently drops every key press while such an IME is active, so the user
    // sees a blank text field. See:
    //   https://developer.apple.com/documentation/uikit/uitextinput
    //
    // We model the "document" that `UITextInput` operates over as just the
    // current marked (preedit) string — committed text lives on the
    // application side and we forward it through the existing
    // `Ime::Commit` event.
    // ----------------------------------------------------------------------
    unsafe impl UITextInput for WinitView {
        // --- marked text (preedit) -----------------------------------

        #[method(setMarkedText:selectedRange:)]
        unsafe fn set_marked_text(
            &self,
            marked_text: Option<&NSString>,
            selected_range: NSRange,
        ) {
            let new_text = marked_text.map(|s| s.to_string()).unwrap_or_default();

            // `setMarkedText:nil` (or with an empty string) is how iOS
            // typically tears down an in-progress composition — e.g. the
            // user tapped the `123` key to switch the soft keyboard from
            // pinyin to numbers. The system does **not** follow up with a
            // separate `insertText:` for what's already been typed, so if
            // we simply cleared the preedit the user would see their
            // letters silently vanish. Commit the previously marked text
            // as literal characters instead, matching how UIKit's own
            // `UITextField` behaves.
            if new_text.is_empty() {
                let prev = std::mem::take(&mut self.ivars().ime.borrow_mut().marked_text);
                self.ivars().ime.borrow_mut().selected_range = (0, 0);
                if !prev.is_empty() {
                    self.emit_ime_event(Ime::Preedit(String::new(), None));
                    self.emit_ime_event(Ime::Commit(prev));
                }
                return;
            }

            let was_empty = !self.ivars().ime.borrow().is_marked();
            {
                let mut state = self.ivars().ime.borrow_mut();
                state.marked_text = new_text;
                // UIKit reports the selection as character offsets inside the
                // marked text. winit's `Ime::Preedit` wants UTF-8 byte offsets.
                let chars: Vec<char> = state.marked_text.chars().collect();
                let to_byte = |char_idx: usize| -> usize {
                    chars.iter().take(char_idx).map(|c| c.len_utf8()).sum()
                };
                let total = chars.len();
                let start = selected_range.location.min(total);
                let end = start
                    .checked_add(selected_range.length)
                    .unwrap_or(total)
                    .min(total);
                state.selected_range = (to_byte(start), to_byte(end));
            }

            if was_empty {
                self.emit_ime_event(Ime::Enabled);
            }
            let snapshot = self.ivars().ime.borrow();
            let event = Ime::Preedit(snapshot.marked_text.clone(), Some(snapshot.selected_range));
            drop(snapshot);
            self.emit_ime_event(event);
        }

        #[method(unmarkText)]
        unsafe fn unmark_text(&self) {
            // Same reasoning as `setMarkedText:` with an empty string:
            // commit the in-progress text instead of dropping it on the
            // floor, otherwise the user's keystrokes disappear when the
            // IME ends a session without picking a candidate (e.g. on
            // keyboard layout switch).
            let prev = std::mem::take(&mut self.ivars().ime.borrow_mut().marked_text);
            self.ivars().ime.borrow_mut().selected_range = (0, 0);
            if !prev.is_empty() {
                self.emit_ime_event(Ime::Preedit(String::new(), None));
                self.emit_ime_event(Ime::Commit(prev));
            }
        }

        #[method_id(markedTextRange)]
        unsafe fn marked_text_range(&self) -> Option<Retained<UITextRange>> {
            let state = self.ivars().ime.borrow();
            if state.is_marked() {
                let mtm = MainThreadMarker::new().unwrap();
                let len = state.marked_len_chars() as i64;
                Some(Retained::into_super(WinitTextRange::new(mtm, 0, len)))
            } else {
                None
            }
        }

        #[method_id(markedTextStyle)]
        unsafe fn marked_text_style(
            &self,
        ) -> Option<Retained<NSDictionary<NSAttributedStringKey, AnyObject>>> {
            None
        }

        #[method(setMarkedTextStyle:)]
        unsafe fn set_marked_text_style(
            &self,
            _: Option<&NSDictionary<NSAttributedStringKey, AnyObject>>,
        ) {
        }

        // --- text storage queries (only marked text is "the document") -

        #[method_id(textInRange:)]
        unsafe fn text_in_range(&self, range: &UITextRange) -> Option<Retained<NSString>> {
            match downcast_range(range) {
                Some(win_range) => {
                    let state = self.ivars().ime.borrow();
                    let chars: Vec<char> = state.marked_text.chars().collect();
                    let start = (win_range.start_offset().max(0) as usize).min(chars.len());
                    let end = (win_range.end_offset().max(0) as usize).min(chars.len());
                    let s: String = if start <= end {
                        chars[start..end].iter().collect()
                    } else {
                        String::new()
                    };
                    Some(NSString::from_str(&s))
                }
                None => None,
            }
        }

        #[method(replaceRange:withText:)]
        unsafe fn replace_range_with_text(&self, range: &UITextRange, text: &NSString) {
            // IME-side commit. The typical pinyin / kana flow ends with:
            //   replaceRange:[the marked range] withText:[selected hanzi]
            // Treat that as Commit(new) + clear preedit.
            let replaced_all_marked = if let Some(r) = downcast_range(range) {
                let state = self.ivars().ime.borrow();
                let marked_len = state.marked_len_chars() as i64;
                r.start_offset() == 0 && r.end_offset() == marked_len && state.is_marked()
            } else {
                false
            };
            let new_text = text.to_string();
            if replaced_all_marked {
                self.ivars().ime.borrow_mut().marked_text.clear();
                self.ivars().ime.borrow_mut().selected_range = (0, 0);
                self.emit_ime_event(Ime::Preedit(String::new(), None));
                if !new_text.is_empty() {
                    self.emit_ime_event(Ime::Commit(new_text));
                }
            } else if !new_text.is_empty() {
                // Best-effort: commit the inserted text.
                self.emit_ime_event(Ime::Commit(new_text));
            }
        }

        // --- selection (tracked inside the marked text) ----------------

        #[method_id(selectedTextRange)]
        unsafe fn selected_text_range(&self) -> Option<Retained<UITextRange>> {
            let state = self.ivars().ime.borrow();
            let chars: Vec<char> = state.marked_text.chars().collect();
            // Translate UTF-8 byte offsets back into character indices.
            let byte_to_char = |byte_off: usize| -> i64 {
                let mut acc = 0usize;
                for (i, c) in chars.iter().enumerate() {
                    if acc >= byte_off {
                        return i as i64;
                    }
                    acc += c.len_utf8();
                }
                chars.len() as i64
            };
            let start = byte_to_char(state.selected_range.0);
            let end = byte_to_char(state.selected_range.1);
            let mtm = MainThreadMarker::new().unwrap();
            Some(Retained::into_super(WinitTextRange::new(mtm, start, end)))
        }

        #[method(setSelectedTextRange:)]
        unsafe fn set_selected_text_range(&self, range: Option<&UITextRange>) {
            let Some(range) = range else { return };
            let Some(win) = downcast_range(range) else { return };
            let chars: Vec<char> = self.ivars().ime.borrow().marked_text.chars().collect();
            let start = win.start_offset().max(0) as usize;
            let end = win.end_offset().max(0) as usize;
            let to_byte = |char_idx: usize| -> usize {
                chars.iter().take(char_idx).map(|c| c.len_utf8()).sum()
            };
            self.ivars().ime.borrow_mut().selected_range = (to_byte(start), to_byte(end));
        }

        // --- document boundaries --------------------------------------

        #[method_id(beginningOfDocument)]
        unsafe fn beginning_of_document(&self) -> Retained<UITextPosition> {
            let mtm = MainThreadMarker::new().unwrap();
            Retained::into_super(WinitTextPosition::new(mtm, 0))
        }

        #[method_id(endOfDocument)]
        unsafe fn end_of_document(&self) -> Retained<UITextPosition> {
            let mtm = MainThreadMarker::new().unwrap();
            let end = self.ivars().ime.borrow().marked_len_chars() as i64;
            Retained::into_super(WinitTextPosition::new(mtm, end))
        }

        // --- position / range arithmetic ----------------------------

        #[method_id(textRangeFromPosition:toPosition:)]
        unsafe fn text_range_from_position(
            &self,
            from: &UITextPosition,
            to: &UITextPosition,
        ) -> Option<Retained<UITextRange>> {
            match (downcast_position(from), downcast_position(to)) {
                (Some(f), Some(t)) => {
                    let (a, b) = if f.offset() <= t.offset() {
                        (f.offset(), t.offset())
                    } else {
                        (t.offset(), f.offset())
                    };
                    let mtm = MainThreadMarker::new().unwrap();
                    Some(Retained::into_super(WinitTextRange::new(mtm, a, b)))
                }
                _ => None,
            }
        }

        #[method_id(positionFromPosition:offset:)]
        unsafe fn position_from_position_offset(
            &self,
            position: &UITextPosition,
            offset: NSInteger,
        ) -> Option<Retained<UITextPosition>> {
            match downcast_position(position) {
                Some(p) => {
                    let target = p.offset().saturating_add(offset as i64);
                    let max = self.ivars().ime.borrow().marked_len_chars() as i64;
                    if (0..=max).contains(&target) {
                        let mtm = MainThreadMarker::new().unwrap();
                        Some(Retained::into_super(WinitTextPosition::new(mtm, target)))
                    } else {
                        None
                    }
                }
                None => None,
            }
        }

        #[method_id(positionFromPosition:inDirection:offset:)]
        unsafe fn position_from_position_in_direction_offset(
            &self,
            position: &UITextPosition,
            direction: UITextLayoutDirection,
            offset: NSInteger,
        ) -> Option<Retained<UITextPosition>> {
            // UITextLayoutDirectionRight (2) / Down (3) move forward;
            // Left (1) / Up (0) move backward.
            let signed = match direction.0 {
                2 | 3 => offset as i64,
                _ => -(offset as i64),
            };
            match downcast_position(position) {
                Some(p) => {
                    let target = p.offset().saturating_add(signed);
                    let max = self.ivars().ime.borrow().marked_len_chars() as i64;
                    if (0..=max).contains(&target) {
                        let mtm = MainThreadMarker::new().unwrap();
                        Some(Retained::into_super(WinitTextPosition::new(mtm, target)))
                    } else {
                        None
                    }
                }
                None => None,
            }
        }

        #[method(comparePosition:toPosition:)]
        unsafe fn compare_position_to_position(
            &self,
            position: &UITextPosition,
            other: &UITextPosition,
        ) -> NSComparisonResult {
            let a = downcast_position(position).map(|p| p.offset()).unwrap_or(0);
            let b = downcast_position(other).map(|p| p.offset()).unwrap_or(0);
            if a < b {
                NSComparisonResult::Ascending
            } else if a > b {
                NSComparisonResult::Descending
            } else {
                NSComparisonResult::Same
            }
        }

        #[method(offsetFromPosition:toPosition:)]
        unsafe fn offset_from_position_to_position(
            &self,
            from: &UITextPosition,
            to: &UITextPosition,
        ) -> NSInteger {
            let a = downcast_position(from).map(|p| p.offset()).unwrap_or(0);
            let b = downcast_position(to).map(|p| p.offset()).unwrap_or(0);
            (b - a) as NSInteger
        }

        // --- delegate / tokenizer -------------------------------------

        #[method_id(inputDelegate)]
        unsafe fn input_delegate(
            &self,
        ) -> Option<Retained<ProtocolObject<dyn UITextInputDelegate>>> {
            // UIKit assigns its own private delegate via `setInputDelegate:`
            // to receive selection/text-change notifications. We don't yet
            // notify it (which is fine for the basic IME path), so just hand
            // back nil if asked.
            None
        }

        #[method(setInputDelegate:)]
        unsafe fn set_input_delegate(
            &self,
            _delegate: Option<&ProtocolObject<dyn UITextInputDelegate>>,
        ) {
            // No-op: see `input_delegate` above.
        }

        #[method_id(tokenizer)]
        unsafe fn tokenizer(&self) -> Retained<ProtocolObject<dyn UITextInputTokenizer>> {
            // UITextInputStringTokenizer is UIKit's default tokenizer for
            // anything implementing UITextInput; it just needs a back-pointer
            // to our view (which is a UIResponder). Creating one per call is
            // fine — UIKit caches the result on its side.
            let mtm = MainThreadMarker::new().unwrap();
            let responder: &UIResponder = self.as_super();
            let tokenizer = unsafe {
                UITextInputStringTokenizer::initWithTextInput(mtm.alloc(), responder)
            };
            ProtocolObject::from_retained(tokenizer)
        }

        // --- direction queries ----------------------------------------

        #[method_id(positionWithinRange:farthestInDirection:)]
        unsafe fn position_within_range_in_direction(
            &self,
            range: &UITextRange,
            direction: UITextLayoutDirection,
        ) -> Option<Retained<UITextPosition>> {
            match downcast_range(range) {
                Some(r) => {
                    let off = match direction.0 {
                        2 | 3 => r.end_offset(),
                        _ => r.start_offset(),
                    };
                    let mtm = MainThreadMarker::new().unwrap();
                    Some(Retained::into_super(WinitTextPosition::new(mtm, off)))
                }
                None => None,
            }
        }

        #[method_id(characterRangeByExtendingPosition:inDirection:)]
        unsafe fn character_range_by_extending(
            &self,
            position: &UITextPosition,
            direction: UITextLayoutDirection,
        ) -> Option<Retained<UITextRange>> {
            match downcast_position(position) {
                Some(p) => {
                    let pos = p.offset();
                    let max = self.ivars().ime.borrow().marked_len_chars() as i64;
                    let (start, end) = match direction.0 {
                        2 | 3 => (pos, max),
                        _ => (0, pos),
                    };
                    let mtm = MainThreadMarker::new().unwrap();
                    Some(Retained::into_super(WinitTextRange::new(mtm, start, end)))
                }
                None => None,
            }
        }

        // --- geometry --------------------------------------------------

        #[method(firstRectForRange:)]
        unsafe fn first_rect_for_range(&self, _range: &UITextRange) -> CGRect {
            // UIKit uses this rect to position the IME candidate popup. The
            // application owns text layout, not winit, so we fall back to the
            // view bounds — the popup will at least show somewhere near the
            // input.
            let bounds: CGRect = unsafe { msg_send![self, bounds] };
            bounds
        }

        #[method(caretRectForPosition:)]
        unsafe fn caret_rect_for_position(&self, _position: &UITextPosition) -> CGRect {
            CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize { width: 1.0, height: 16.0 },
            }
        }

        #[method_id(selectionRectsForRange:)]
        unsafe fn selection_rects_for_range(
            &self,
            _range: &UITextRange,
        ) -> Retained<NSArray<UITextSelectionRect>> {
            NSArray::new()
        }

        // --- hit testing ---------------------------------------------

        #[method_id(closestPositionToPoint:)]
        unsafe fn closest_position_to_point(
            &self,
            _point: CGPoint,
        ) -> Option<Retained<UITextPosition>> {
            let mtm = MainThreadMarker::new().unwrap();
            Some(Retained::into_super(WinitTextPosition::new(mtm, 0)))
        }

        #[method_id(closestPositionToPoint:withinRange:)]
        unsafe fn closest_position_to_point_within(
            &self,
            _point: CGPoint,
            range: &UITextRange,
        ) -> Option<Retained<UITextPosition>> {
            let mtm = MainThreadMarker::new().unwrap();
            let off = downcast_range(range).map(|r| r.start_offset()).unwrap_or(0);
            Some(Retained::into_super(WinitTextPosition::new(mtm, off)))
        }

        #[method_id(characterRangeAtPoint:)]
        unsafe fn character_range_at_point(
            &self,
            _point: CGPoint,
        ) -> Option<Retained<UITextRange>> {
            let mtm = MainThreadMarker::new().unwrap();
            Some(Retained::into_super(WinitTextRange::new(mtm, 0, 0)))
        }

        // The UITextInput protocol marks these two as required even when
        // they appear under `#[cfg(feature = "NSText")]` in objc2-ui-kit
        // (the cfg only gates the Rust binding for `NSWritingDirection`,
        // not the underlying Objective-C protocol). Implement them as
        // plain selectors against `NSInteger` so we don't have to pull in
        // the NSText feature.
        #[method(baseWritingDirectionForPosition:inDirection:)]
        unsafe fn base_writing_direction(
            &self,
            _position: &UITextPosition,
            _direction: NSInteger,
        ) -> NSInteger {
            // NSWritingDirectionNatural == 0
            0
        }

        #[method(setBaseWritingDirection:forRange:)]
        unsafe fn set_base_writing_direction(
            &self,
            _direction: NSInteger,
            _range: &UITextRange,
        ) {
        }
    }
);

impl WinitView {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        window_attributes: &WindowAttributes,
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

            ime: RefCell::new(ImeState::default()),
        });
        let this: Retained<Self> = unsafe { msg_send_id![super(this), initWithFrame: frame] };

        this.setMultipleTouchEnabled(true);

        if let Some(scale_factor) = window_attributes.platform_specific.scale_factor {
            this.setContentScaleFactor(scale_factor as _);
        }

        this
    }

    fn window(&self) -> Option<Retained<WinitUIWindow>> {
        // SAFETY: `WinitView`s are always installed in a `WinitUIWindow`
        (**self).window().map(|window| unsafe { Retained::cast(window) })
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
        let os_supports_force = app_state::os_capabilities().force_touch;
        for touch in touches {
            let logical_location = touch.locationInView(None);
            let touch_type = touch.r#type();
            let force = if os_supports_force {
                let trait_collection = self.traitCollection();
                let touch_capability = trait_collection.forceTouchCapability();
                // Both the OS _and_ the device need to be checked for force touch support.
                if touch_capability == UIForceTouchCapability::Available
                    || touch_type == UITouchType::Pencil
                {
                    let force = touch.force();
                    let max_possible_force = touch.maximumPossibleForce();
                    let altitude_angle: Option<f64> = if touch_type == UITouchType::Pencil {
                        let angle = touch.altitudeAngle();
                        Some(angle as _)
                    } else {
                        None
                    };
                    Some(Force::Calibrated {
                        force: force as _,
                        max_possible_force: max_possible_force as _,
                        altitude_angle,
                    })
                } else {
                    None
                }
            } else {
                None
            };
            let touch_id = touch as *const UITouch as u64;
            let phase = touch.phase();
            let phase = match phase {
                UITouchPhase::Began => TouchPhase::Started,
                UITouchPhase::Moved => TouchPhase::Moved,
                // 2 is UITouchPhase::Stationary and is not expected here
                UITouchPhase::Ended => TouchPhase::Ended,
                UITouchPhase::Cancelled => TouchPhase::Cancelled,
                _ => panic!("unexpected touch phase: {phase:?}"),
            };

            let physical_location = {
                let scale_factor = self.contentScaleFactor();
                PhysicalPosition::from_logical::<(f64, f64), f64>(
                    (logical_location.x as _, logical_location.y as _),
                    scale_factor as f64,
                )
            };
            touch_events.push(EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::Touch(Touch {
                    device_id: DEVICE_ID,
                    id: touch_id,
                    location: physical_location,
                    force,
                    phase,
                }),
            }));
        }
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(mtm, touch_events);
    }

    fn emit_ime_event(&self, ime: Ime) {
        let Some(window) = self.window() else {
            return;
        };
        let window_id = RootWindowId(window.id());
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_event(
            mtm,
            EventWrapper::StaticEvent(Event::WindowEvent {
                window_id,
                event: WindowEvent::Ime(ime),
            }),
        );
    }

    fn handle_insert_text(&self, text: &NSString) {
        let window = self.window().unwrap();
        let window_id = RootWindowId(window.id());
        let mtm = MainThreadMarker::new().unwrap();
        // send individual events for each character
        app_state::handle_nonuser_events(
            mtm,
            text.to_string().chars().flat_map(|c| {
                let text = smol_str::SmolStr::from_iter([c]);
                // Emit both press and release events
                [ElementState::Pressed, ElementState::Released].map(|state| {
                    EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id,
                        event: WindowEvent::KeyboardInput {
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
                                physical_key: PhysicalKey::Unidentified(
                                    NativeKeyCode::Unidentified,
                                ),
                                platform_specific: KeyEventExtra {},
                            },
                            is_synthetic: false,
                            device_id: DEVICE_ID,
                        },
                    })
                })
            }),
        );
    }

    fn handle_delete_backward(&self) {
        let window = self.window().unwrap();
        let window_id = RootWindowId(window.id());
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(
            mtm,
            [ElementState::Pressed, ElementState::Released].map(|state| {
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id,
                    event: WindowEvent::KeyboardInput {
                        device_id: DEVICE_ID,
                        event: KeyEvent {
                            state,
                            logical_key: Key::Named(NamedKey::Backspace),
                            physical_key: PhysicalKey::Code(KeyCode::Backspace),
                            platform_specific: KeyEventExtra {},
                            repeat: false,
                            location: KeyLocation::Standard,
                            text: None,
                        },
                        is_synthetic: false,
                    },
                })
            }),
        );
    }
}

/// Cast a `&UITextPosition` to our concrete subclass if it is one, otherwise
/// return `None` (UIKit should only ever hand us positions we ourselves
/// produced, but we still check defensively).
fn downcast_position(position: &UITextPosition) -> Option<&WinitTextPosition> {
    use objc2::msg_send;
    let cls = WinitTextPosition::class();
    let is_kind: bool = unsafe { msg_send![position, isKindOfClass: cls] };
    if is_kind {
        // SAFETY: `isKindOfClass:` returned true, so the pointer can be
        // reinterpreted as our subclass.
        Some(unsafe { &*(position as *const UITextPosition as *const WinitTextPosition) })
    } else {
        None
    }
}

fn downcast_range(range: &UITextRange) -> Option<&WinitTextRange> {
    use objc2::msg_send;
    let cls = WinitTextRange::class();
    let is_kind: bool = unsafe { msg_send![range, isKindOfClass: cls] };
    if is_kind {
        Some(unsafe { &*(range as *const UITextRange as *const WinitTextRange) })
    } else {
        None
    }
}
