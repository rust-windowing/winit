#![allow(clippy::unnecessary_cast)]

use std::cell::RefCell;

use icrate::Foundation::{CGPoint, CGRect, CGSize, MainThreadMarker, NSObject, NSObjectProtocol};
use objc2::rc::Id;
use objc2::runtime::ProtocolObject;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass, extern_methods};
use super::app_state::{self, EventWrapper};

use super::uikit::{UIResponder, UITextView, UITextViewDelegate, UIView};
use super::window::WinitUIWindow;
use crate::{
    keyboard::{
        KeyCode,
        PhysicalKey,
        Key,
        KeyLocation,
    },
    dpi::PhysicalPosition,
    event::{Event, KeyEvent, Force, Touch, TouchPhase, WindowEvent},
    platform_impl::platform::DEVICE_ID,
    window::{WindowAttributes, WindowId as RootWindowId},
};

pub struct WinitTextFieldState {
    delegate: RefCell<Id<WinitTextFieldDelegate>>,
}

declare_class!(
    pub(crate) struct WinitTextField;

    unsafe impl ClassType for WinitTextField {
        #[inherits(UIView, UIResponder, NSObject)]
        type Super = UITextView;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitUITextView";
    }

    impl DeclaredClass for WinitTextField {
        type Ivars = WinitTextFieldState;
    }

    unsafe impl WinitTextField { }
);
extern_methods!(
    unsafe impl WinitTextField {
        fn window(&self) -> Option<Id<WinitUIWindow>> {
            unsafe { msg_send_id![self, window] }
        }
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
        unsafe fn text_field_did_begin_editing(&self, sender: &WinitTextField) {
            let text = sender.text();
            //println!("DidBeginEditing: {text}");
        }

        #[method(textViewDidEndEditing:)]
        unsafe fn text_field_did_end_editing(&self, sender: &WinitTextField) {
            let text = sender.text();
            //println!("DidEndEditing: {text}");
        }

        #[method(textViewDidChange:)]
        unsafe fn text_field_did_change(&self, sender: &WinitTextField) {
            let text = sender.text();
            //println!("textViewDidChange: {text}");
            sender.text_changed();
        }
    }
);


impl WinitTextField {

    pub(crate) fn new(mtm: MainThreadMarker) -> Id<Self> {
        // TODO: This should be hidden someplace.
        let frame = CGRect {
            origin: CGPoint { x: -20.0, y: -50.0 },
            size: CGSize {
                width: 200.0,
                height: 40.0,
            },
        };
        let delegate: Id<WinitTextFieldDelegate> = unsafe { objc2::msg_send_id![mtm.alloc(), init]};
        let this = Self::alloc().set_ivars( WinitTextFieldState{
            delegate: RefCell::new(delegate),
        });
        let this: Id<WinitTextField> = unsafe { msg_send_id![super(this), init] };

        {
            let delegate = this.ivars().delegate.borrow();
            this.setDelegate(Some(ProtocolObject::from_ref(&*delegate.clone())));
        }

        this.setFrame(frame);

        this
    }
    fn text_changed(&self) {
            let window = self.window().unwrap();
            let mtm = MainThreadMarker::new().unwrap();
            let text = self.text();
            let text = text.to_string();
            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::KeyboardInput {
                        device_id: DEVICE_ID,
                        event: KeyEvent {
                            physical_key: PhysicalKey::Code(KeyCode::F35),
                            logical_key: Key::Character(text.clone().into()),
                            text: Some(text.into()),
                            location: KeyLocation::Standard,
                            state: crate::event::ElementState::Pressed,
                            repeat: false,
                            platform_specific: super::KeyEventExtra{},
                        },
                        is_synthetic: false,
                    },
                }),
            );
    }
}
