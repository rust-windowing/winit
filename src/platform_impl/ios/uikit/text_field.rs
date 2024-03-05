use super::UIView;
use icrate::Foundation::{NSObject, NSString};
use objc2::mutability::IsMainThreadOnly;
use objc2::rc::Id;
use objc2::runtime::{NSObjectProtocol, ProtocolObject};
use objc2::{extern_class, extern_methods, extern_protocol, mutability, ClassType, ProtocolType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UITextView;

    unsafe impl ClassType for UITextView {
        #[inherits(NSObject)]
        type Super = UIView;
        type Mutability = mutability::InteriorMutable;
    }
);
extern_methods!(
    unsafe impl UITextView {
        #[method(frame)]
        pub fn text(&self) -> &NSString;

        // These are methods from UIResponder
        #[method(becomeFirstResponder)]
        pub fn focus(&self) -> bool;

        #[method(resignFirstResponder)]
        pub fn unfocus(&self) -> bool;

        #[method_id(@__retain_semantics Other delegate)]
        pub unsafe fn delegate(&self) -> Option<Id<ProtocolObject<dyn UITextViewDelegate>>>;

        #[method(setDelegate:)]
        pub fn setDelegate(&self, delegate: Option<&ProtocolObject<dyn UITextViewDelegate>>);
    }
);
extern_protocol!(
    pub unsafe trait UITextViewDelegate: NSObjectProtocol + IsMainThreadOnly {
        #[optional]
        #[method(textViewShouldBeginEditing:)]
        unsafe fn textViewShouldBeginEditing(&self, sender: &UITextView) -> bool;

        #[optional]
        #[method(textViewDidBeginEditing:)]
        unsafe fn textViewDidBeginEditing(&self, sender: &UITextView);

        #[optional]
        #[method(textViewShouldEndEditing:)]
        unsafe fn textViewShouldEndEditing(&self, sender: &UITextView) -> bool;

        #[optional]
        #[method(textViewDidEndEditing:)]
        unsafe fn textViewDidEndEditing(&self, sender: &UITextView);

        #[optional]
        #[method(textViewDidChange:)]
        unsafe fn textViewDidChange(&self, sender: &UITextView);
    }
    unsafe impl ProtocolType for dyn UITextViewDelegate {}
);
