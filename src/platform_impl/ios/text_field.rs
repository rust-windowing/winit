#![allow(clippy::unnecessary_cast)]

use icrate::Foundation::{CGPoint, CGRect, CGSize, MainThreadMarker, NSObject, NSObjectProtocol};
use objc2::rc::Id;
use objc2::runtime::ProtocolObject;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};

use super::uikit::{UIResponder, UITextView, UITextViewDelegate};

declare_class!(
    pub(crate) struct WinitTextField;

    unsafe impl ClassType for WinitTextField {
        #[inherits(UIResponder, NSObject)]
        type Super = UITextView;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitUITextView";
    }

    impl DeclaredClass for WinitTextField { }

    unsafe impl WinitTextField { }
);

declare_class!(
    pub(crate) struct WinitTextFieldDelegate;

    unsafe impl ClassType for WinitTextFieldDelegate {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitTextViewDelegate";
    }

    impl DeclaredClass for WinitTextFieldDelegate {
        type Ivars = ();
    }

    unsafe impl NSObjectProtocol for WinitTextFieldDelegate {}

    unsafe impl UITextViewDelegate for WinitTextFieldDelegate {

        #[method(textViewDidBeginEditing:)]
        unsafe fn text_view_did_begin_editing(&self, sender: &UITextView) {
            let text = sender.text();
            println!("DidBeginEditing: {text}");
        }

        #[method(textViewDidEndEditing:)]
        unsafe fn text_view_did_end_editing(&self, sender: &UITextView) {
            let text = sender.text();
            println!("DidEndEditing: {text}");
        }

        #[method(textViewDidChange:)]
        unsafe fn text_view_did_change(&self, sender: &UITextView) {
            let text = sender.text();
            println!("ShouldEndEditing: {text}");
        }

    }
);

impl WinitTextField {
    pub(crate) fn new(mtm: MainThreadMarker) -> Id<Self> {
        // TODO: This should be hidden someplace.
        let frame = CGRect {
            origin: CGPoint { x: 20.0, y: 50.0 },
            size: CGSize {
                width: 200.0,
                height: 40.0,
            },
        };
        let this: Id<WinitTextField> = unsafe { msg_send_id![Self::alloc(), init] };
        this.setFrame(frame);
        let delegate = mtm.alloc();
        let delegate: Id<WinitTextFieldDelegate> = unsafe { msg_send_id![delegate, init] };

        this.setDelegate(Some(ProtocolObject::from_ref(delegate.as_ref())));

        this
    }
}
