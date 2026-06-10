//! Helpers for the iOS `UITextInput` protocol implementation.
//!
//! UIKit requires that anything which can be the target of a multi-stage
//! (CJK) input method adopt `UITextInput`, which talks in terms of opaque
//! `UITextPosition` / `UITextRange` objects. iOS will call back into our view
//! with these objects, so we need concrete subclasses we can downcast and read
//! offsets from.
//!
//! We treat the "document" that `UITextInput` sees as just the current marked
//! (preedit) text — outside of an active composition we report an empty
//! document. The committed text lives on the application side; we only forward
//! `Ime::Preedit` / `Ime::Commit` events.

use std::cell::Cell;

use objc2::rc::Retained;
use objc2::runtime::NSObjectProtocol;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_foundation::{MainThreadMarker, NSObject};
use objc2_ui_kit::{UITextPosition, UITextRange};

/// State that the view tracks for an active IME composition.
///
/// `marked_text` is the current preedit string. `selected_range` is the
/// selection inside that preedit string (start/end in UTF-8 byte offsets,
/// matching what `Ime::Preedit` expects).
#[derive(Default)]
pub(crate) struct ImeState {
    pub(crate) marked_text: String,
    pub(crate) selected_range: (usize, usize),
}

impl ImeState {
    pub(crate) fn is_marked(&self) -> bool {
        !self.marked_text.is_empty()
    }

    pub(crate) fn marked_len_chars(&self) -> usize {
        self.marked_text.chars().count()
    }
}

pub(crate) struct WinitTextPositionState {
    offset: Cell<i64>,
}

declare_class!(
    /// `UITextPosition` subclass carrying a character offset into the marked
    /// text.
    pub(crate) struct WinitTextPosition;

    unsafe impl ClassType for WinitTextPosition {
        #[inherits(NSObject)]
        type Super = UITextPosition;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitTextPosition";
    }

    impl DeclaredClass for WinitTextPosition {
        type Ivars = WinitTextPositionState;
    }
);

impl WinitTextPosition {
    pub(crate) fn new(mtm: MainThreadMarker, offset: i64) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(WinitTextPositionState { offset: Cell::new(offset) });
        unsafe { msg_send_id![super(this), init] }
    }

    pub(crate) fn offset(&self) -> i64 {
        self.ivars().offset.get()
    }
}

pub(crate) struct WinitTextRangeState {
    start: Cell<i64>,
    end: Cell<i64>,
}

declare_class!(
    /// `UITextRange` subclass holding a `[start, end)` interval of character
    /// offsets into the marked text.
    pub(crate) struct WinitTextRange;

    unsafe impl ClassType for WinitTextRange {
        #[inherits(NSObject)]
        type Super = UITextRange;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitTextRange";
    }

    impl DeclaredClass for WinitTextRange {
        type Ivars = WinitTextRangeState;
    }

    unsafe impl WinitTextRange {
        #[method(isEmpty)]
        fn is_empty(&self) -> bool {
            self.ivars().start.get() == self.ivars().end.get()
        }

        #[method_id(start)]
        fn start_position(&self) -> Retained<UITextPosition> {
            let mtm = MainThreadMarker::new().expect("WinitTextRange used off the main thread");
            let p = WinitTextPosition::new(mtm, self.ivars().start.get());
            Retained::into_super(p)
        }

        #[method_id(end)]
        fn end_position(&self) -> Retained<UITextPosition> {
            let mtm = MainThreadMarker::new().expect("WinitTextRange used off the main thread");
            let p = WinitTextPosition::new(mtm, self.ivars().end.get());
            Retained::into_super(p)
        }
    }
);

impl WinitTextRange {
    pub(crate) fn new(mtm: MainThreadMarker, start: i64, end: i64) -> Retained<Self> {
        let this = mtm
            .alloc()
            .set_ivars(WinitTextRangeState { start: Cell::new(start), end: Cell::new(end) });
        unsafe { msg_send_id![super(this), init] }
    }

    pub(crate) fn start_offset(&self) -> i64 {
        self.ivars().start.get()
    }

    pub(crate) fn end_offset(&self) -> i64 {
        self.ivars().end.get()
    }
}

unsafe impl NSObjectProtocol for WinitTextPosition {}
unsafe impl NSObjectProtocol for WinitTextRange {}
