use crate::platform::macos::FileOpenResult;

use super::{
    activation_hack,
    app_state::AppState,
    ffi::{
        NSApplicationDelegateReplyCancel, NSApplicationDelegateReplyFailure,
        NSApplicationDelegateReplySuccess,
    },
    util::ns_string_to_rust,
};
use cocoa::{base::id, foundation::NSArray};
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Sel, BOOL, NO, YES},
};
use std::{os::raw::c_void, str};

pub struct AppDelegateClass(pub *const Class);
unsafe impl Send for AppDelegateClass {}
unsafe impl Sync for AppDelegateClass {}

lazy_static! {
    pub static ref APP_DELEGATE_CLASS: AppDelegateClass = unsafe {
        let superclass = class!(NSResponder);
        let mut decl = ClassDecl::new("WinitAppDelegate", superclass).unwrap();

        decl.add_class_method(sel!(new), new as extern "C" fn(&Class, Sel) -> id);
        decl.add_method(sel!(dealloc), dealloc as extern "C" fn(&Object, Sel));
        decl.add_method(
            sel!(applicationDidFinishLaunching:),
            did_finish_launching as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationDidBecomeActive:),
            did_become_active as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationDidResignActive:),
            did_resign_active as extern "C" fn(&Object, Sel, id),
        );

        decl.add_ivar::<*mut c_void>(activation_hack::State::name());
        decl.add_method(
            sel!(activationHackMouseMoved:),
            activation_hack::mouse_moved as extern "C" fn(&Object, Sel, id),
        );

        decl.add_method(
            sel!(application:openFile:),
            application_open_file as extern "C" fn(&Object, Sel, id, id) -> BOOL,
        );
        decl.add_method(
            sel!(application:openFiles:),
            application_open_files as extern "C" fn(&Object, Sel, id, id),
        );

        AppDelegateClass(decl.register())
    };
}

extern "C" fn new(class: &Class, _: Sel) -> id {
    unsafe {
        let this: id = msg_send![class, alloc];
        let this: id = msg_send![this, init];
        (*this).set_ivar(
            activation_hack::State::name(),
            activation_hack::State::new(),
        );
        this
    }
}

extern "C" fn dealloc(this: &Object, _: Sel) {
    unsafe {
        activation_hack::State::free(activation_hack::State::get_ptr(this));
    }
}

extern "C" fn did_finish_launching(_: &Object, _: Sel, _: id) {
    trace!("Triggered `applicationDidFinishLaunching`");
    AppState::launched();
    trace!("Completed `applicationDidFinishLaunching`");
}

extern "C" fn did_become_active(this: &Object, _: Sel, _: id) {
    trace!("Triggered `applicationDidBecomeActive`");
    unsafe {
        activation_hack::State::set_activated(this, true);
    }
    trace!("Completed `applicationDidBecomeActive`");
}

extern "C" fn did_resign_active(this: &Object, _: Sel, _: id) {
    trace!("Triggered `applicationDidResignActive`");
    unsafe {
        activation_hack::refocus(this);
    }
    trace!("Completed `applicationDidResignActive`");
}

extern "C" fn application_open_file(_this: &Object, _: Sel, _sender: id, filename: id) -> BOOL {
    trace!("Triggered `application:openFile:`");
    let string = unsafe { ns_string_to_rust(filename) };
    let result = AppState::open_files(vec![string.into()]);
    trace!("Completed `application:openFile:`");

    if result == FileOpenResult::Success {
        YES
    } else {
        NO
    }
}

extern "C" fn application_open_files(_this: &Object, _: Sel, sender: id, filenames: id) {
    trace!("Triggered `application:openFiles:`");
    let filenames_len = unsafe { filenames.count() };
    let filenames_vec = (0..filenames_len)
        .map(|i| unsafe {
            let filename = filenames.objectAtIndex(i);
            ns_string_to_rust(filename).into()
        })
        .collect();
    let result = AppState::open_files(filenames_vec);
    let response = match result {
        FileOpenResult::Success => NSApplicationDelegateReplySuccess,
        FileOpenResult::Cancel => NSApplicationDelegateReplyCancel,
        FileOpenResult::Failure => NSApplicationDelegateReplyFailure,
    };
    unsafe { msg_send![sender, replyToOpenOrPrint: response] }

    trace!("Completed `application:openFiles:`");
}
