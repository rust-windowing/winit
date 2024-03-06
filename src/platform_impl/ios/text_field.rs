#![allow(clippy::unnecessary_cast)]

use icrate::Foundation::{CGPoint, CGRect, CGSize, MainThreadMarker, NSObject, NSObjectProtocol};
use objc2::rc::Id;
use objc2::runtime::ProtocolObject;
use objc2::{declare_class, msg_send_id, msg_send, mutability, ClassType, DeclaredClass, extern_methods};

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
extern_methods!(
    unsafe impl WinitTextField {
        // These are methods from UIResponder
        #[method(becomeFirstResponder)]
        pub fn focus(&self) -> bool;

        #[method(resignFirstResponder)]
        pub fn unfocus(&self) -> bool;
    }
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
        unsafe fn textViewDidBeginEditing(&self, sender: &UITextView) {
            let text = "dummy text"; // sender.text()
            println!("DidBeginEditing: {text}");
        }

        #[method(textViewDidEndEditing:)]
        unsafe fn textViewDidEndEditing(&self, sender: &UITextView) {
            let text = "dummy text"; 
            println!("DidEndEditing: {text}");
        }

        #[method(textViewDidChange:)]
        unsafe fn textViewDidChange(&self, sender: &UITextView) {
            let text = "dummy text"; // sender.text()
            println!("textViewDidChange: {text}");
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

        unsafe{ println!("this.get_delegate(): {:?}", this.delegate()) };

        //let _ : () = unsafe{ msg_send![&delegate, textViewDidBeginEditing: Id::as_ptr(&this) ] };

        this
    }
}
