// Ideally this will go into the "apple" platform module to be shared with AppKit and UIKit

// Winit domain object for https://developer.apple.com/documentation/uikit/uigesturerecognizerdelegate?language=objc
//  and https://developer.apple.com/documentation/appkit/nsgesturerecognizerdelegate?language=objc
pub trait GestureRecognizerDelegate<GestureRecognizer, Touch, Press, Event> {
    fn should_recognize_simultaneously(
        &self,
        _gesture_recognizer: &GestureRecognizer,
        _other_gesture_recognizer: &GestureRecognizer,
    ) -> bool {
        true
    }

    fn should_require_failure_of_gesture_recognizer(
        &self,
        _gesture_recognizer: &GestureRecognizer,
        _other_gesture_recognizer: &GestureRecognizer,
    ) -> bool {
        false
    }

    fn should_be_required_to_fail_by_gesture_recognizer(
        &self,
        _gesture_recognizer: &GestureRecognizer,
        _other_gesture_recognizer: &GestureRecognizer,
    ) -> bool {
        false
    }

    fn should_begin(&self, _gesture_recognizer: &GestureRecognizer) -> bool {
        true
    }

    fn should_receive_touch(
        &self,
        _gesture_recognizer: &GestureRecognizer,
        _touch: &Touch,
    ) -> bool {
        true
    }

    // IOS only
    fn should_receive_press(
        &self,
        _gesture_recognizer: &GestureRecognizer,
        _press: &Press,
    ) -> bool {
        true
    }

    // IOS only
    fn should_receive_event(
        &self,
        _gesture_recognizer: &GestureRecognizer,
        _event: &Event,
    ) -> bool {
        true
    }
}
