use std::cell::{Cell, RefCell};

use objc2::rc::Retained;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSAttributedString, NSObject, NSRange, NSString};
use objc2_ui_kit::{
    UIControl, UIEvent, UIResponder, UITextField, UITextFieldDelegate, UITextInput, UITextRange,
    UIView,
};
use winit_common::text::utf16_to_utf8_offset;
use winit_core::event::Ime;

use super::view::WinitView;

type Preedit = (String, Option<(usize, usize)>);

#[derive(Default)]
pub(crate) struct ImeFieldState {
    enabled: Cell<bool>,
    editing: Cell<bool>,
    preedit: RefCell<Option<Preedit>>,
}

define_class!(
    #[unsafe(super(UITextField, UIControl, UIView, UIResponder, NSObject))]
    #[name = "WinitImeField"]
    #[ivars = ImeFieldState]
    pub(crate) struct ImeField;

    unsafe impl NSObjectProtocol for ImeField {}

    unsafe impl UITextFieldDelegate for ImeField {
        #[unsafe(method(textFieldShouldReturn:))]
        fn should_return(&self, _field: &UITextField) -> bool {
            if let Some(view) = self.view() {
                view.handle_insert_text(&NSString::from_str("\n"));
            }
            false
        }
    }

    impl ImeField {
        #[unsafe(method(pointInside:withEvent:))]
        fn point_inside(&self, _point: CGPoint, _event: Option<&UIEvent>) -> bool {
            false
        }

        #[unsafe(method(hasText))]
        fn has_text(&self) -> bool {
            // The editor can contain text beyond the current composition.
            true
        }

        #[unsafe(method(setMarkedText:selectedRange:))]
        fn set_marked_text(&self, text: Option<&NSString>, selection: NSRange) {
            self.edit(false, None, || {
                let _: () = unsafe { msg_send![super(self), setMarkedText: text, selectedRange: selection] };
            });
        }

        #[unsafe(method(setAttributedMarkedText:selectedRange:))]
        fn set_attributed_marked_text(&self, text: Option<&NSAttributedString>, selection: NSRange) {
            self.edit(false, None, || {
                let _: () = unsafe {
                    msg_send![super(self), setAttributedMarkedText: text, selectedRange: selection]
                };
            });
        }

        #[unsafe(method(unmarkText))]
        fn unmark_text(&self) {
            self.edit(true, None, || {
                let _: () = unsafe { msg_send![super(self), unmarkText] };
            });
        }

        #[unsafe(method(insertText:))]
        fn insert_text(&self, text: &NSString) {
            self.edit(true, Some(text), || {
                let _: () = unsafe { msg_send![super(self), insertText: text] };
            });
        }

        #[unsafe(method(replaceRange:withText:))]
        fn replace_range(&self, range: &UITextRange, text: &NSString) {
            self.edit(true, Some(text), || {
                let _: () = unsafe { msg_send![super(self), replaceRange: range, withText: text] };
            });
        }

        #[unsafe(method(deleteBackward))]
        fn delete_backward(&self) {
            if self.ivars().editing.get() || self.ivars().preedit.borrow().is_some() {
                self.edit(false, None, || {
                    let _: () = unsafe { msg_send![super(self), deleteBackward] };
                });
            } else if let Some(view) = self.view() {
                view.handle_delete_backward();
            }
        }
    }
);

impl ImeField {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ImeFieldState::default());
        let frame = CGRect::new(CGPoint::ZERO, CGSize::new(1.0, 1.0));
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        this.setAlpha(0.0);
        this.setDelegate(Some(ProtocolObject::from_ref(&*this)));
        this
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        if self.ivars().enabled.replace(enabled) == enabled {
            return;
        }
        if enabled {
            self.becomeFirstResponder();
        } else {
            self.resignFirstResponder();
            self.ivars().preedit.borrow_mut().take();
            self.setText(None);
        }
        if let Some(view) = self.view() {
            view.send_ime(if enabled { Ime::Enabled } else { Ime::Disabled });
        }
    }

    fn view(&self) -> Option<Retained<WinitView>> {
        self.superview()?.downcast().ok()
    }

    fn edit(&self, commit_marked: bool, plain: Option<&NSString>, edit: impl FnOnce()) {
        // UIKit can invoke another text-input selector while applying an edit.
        // Only the outermost call should report the final native state.
        if self.ivars().editing.replace(true) {
            edit();
            return;
        }
        edit();
        self.ivars().editing.set(false);
        if !self.ivars().enabled.get() {
            return;
        }
        let Some(view) = self.view() else { return };
        let current = self.marked_text();
        let changed = *self.ivars().preedit.borrow() != current;
        let previous = self.ivars().preedit.replace(current.clone());
        if let Some((text, cursor)) = current {
            if changed {
                view.send_ime(Ime::Preedit(text, cursor));
            }
        } else {
            let committed = self.text().map(|text| text.to_string()).unwrap_or_default();
            // The application owns committed text. Keep only the active UIKit composition.
            self.setText(None);
            if previous.is_some() {
                if commit_marked && !committed.is_empty() {
                    view.send_ime(Ime::Commit(committed));
                } else {
                    view.send_ime(Ime::Preedit(String::new(), None));
                }
            } else if let Some(text) = plain {
                view.handle_insert_text(text);
            }
        }
    }

    fn marked_text(&self) -> Option<Preedit> {
        let range = self.markedTextRange()?;
        let text = self.textInRange(&range)?.to_string();
        if text.is_empty() {
            return None;
        }
        let cursor = self.selectedTextRange().map(|selection| {
            let start = range.start();
            let offset = |position| {
                utf16_to_utf8_offset(
                    &text,
                    self.offsetFromPosition_toPosition(&start, position).max(0) as usize,
                )
            };
            (offset(&selection.start()), offset(&selection.end()))
        });
        Some((text, cursor))
    }
}
