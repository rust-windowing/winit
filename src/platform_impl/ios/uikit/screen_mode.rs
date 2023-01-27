use objc2::foundation::{CGSize, NSObject};
use objc2::{extern_class, extern_methods, ClassType};

extern_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct UIScreenMode;

    unsafe impl ClassType for UIScreenMode {
        type Super = NSObject;
    }
);

extern_methods!(
    unsafe impl UIScreenMode {
        #[sel(size)]
        pub fn size(&self) -> CGSize;
    }
);
