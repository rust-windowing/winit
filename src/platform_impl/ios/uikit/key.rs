use core_foundation::mach_port::CFIndex;
use icrate::Foundation::{NSInteger, NSObject, NSString};
use objc2::{extern_class, extern_methods, mutability, ClassType, rc::Id};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIKey;

    unsafe impl ClassType for UIKey {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
    }
);

extern_methods!(
    unsafe impl UIKey {
        // https://developer.apple.com/documentation/uikit/uikey/3526132-keycode?language=objc
        #[method(keyCode)]
        pub fn key_code(&self) -> CFIndex; // -> enum UIKeyboardHIDUsage

        // https://developer.apple.com/documentation/uikit/uikeymodifierflags?language=objc
        #[method(modifierFlags)]
        pub fn modifier_flags(&self) -> NSInteger; // -> enum UIKeyModifierFlags

        #[method_id(characters)]
        pub fn characters(&self) -> Id<NSString>;

        #[method_id(charactersIgnoringModifiers)]
        pub fn characters_ignoring_modifiers(&self) -> Id<NSString>;
    }
);
