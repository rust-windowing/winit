use super::app_state::AppState;
use cocoa::base::id;
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Sel},
};

pub struct AppDelegateClass(pub *const Class);
unsafe impl Send for AppDelegateClass {}
unsafe impl Sync for AppDelegateClass {}

lazy_static! {
    pub static ref APP_DELEGATE_CLASS: AppDelegateClass = unsafe {
        let superclass = class!(NSResponder);
        let mut decl = ClassDecl::new("WinitAppDelegate", superclass).unwrap();

        decl.add_method(
            sel!(applicationDidFinishLaunching:),
            did_finish_launching as extern "C" fn(&Object, Sel, id),
        );

        AppDelegateClass(decl.register())
    };
}

extern "C" fn did_finish_launching(_: &Object, _: Sel, _: id) {
    trace!("Triggered `applicationDidFinishLaunching`");
    AppState::launched();
    trace!("Completed `applicationDidFinishLaunching`");
}
