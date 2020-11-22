use crate::event::Event;

use super::{
    activation_hack,
    app_state::AppState,
    event::EventWrapper,
};
use cocoa::base::id;
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Sel, BOOL, YES},
};
use std::os::raw::c_void;

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

/// Copies the contents of the ns string into a `String` which gets returned.
fn ns_string_to_rust(ns_string: id) -> String {
    use cocoa::foundation::NSString;

    let utf8_len = unsafe { ns_string.len() };
    let utf8_ptr = unsafe { ns_string.UTF8String() } as *mut u8;
    let utf8_slice = unsafe { std::slice::from_raw_parts(utf8_ptr, utf8_len) };
    let mut utf8_vec = Vec::<u8>::with_capacity(utf8_len);
    unsafe {
        utf8_vec.set_len(utf8_len);
    }
    utf8_vec.copy_from_slice(utf8_slice);

    unsafe { String::from_utf8_unchecked(utf8_vec) }
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
    let mut filenames_vec = Vec::with_capacity(1);
    let string = ns_string_to_rust(filename);
    filenames_vec.push(string.into());
    let event = Event::OpenFiles(filenames_vec);
    AppState::queue_event(EventWrapper::StaticEvent(event));
    trace!("Completed `application:openFile:`");

    // Return true to indicate to the OS that the file type is supported.
    // (If the filetype turns out not to be supported, it's the application's
    // responsibility to inform the user)
    YES
}

extern "C" fn application_open_files(_this: &Object, _: Sel, _sender: id, filenames: id) {
    use cocoa::foundation::NSArray;

    #[allow(non_upper_case_globals)]
    const NSApplicationDelegateReplySuccess: i32 = 0;

    trace!("Triggered `application:openFiles:`");
    let filenames_len = unsafe { filenames.count() };
    let mut filenames_vec = Vec::with_capacity(filenames_len as usize);
    for i in 0..filenames_len {
        let filename = unsafe { filenames.objectAtIndex(i) };
        let name_string = ns_string_to_rust(filename);
        filenames_vec.push(name_string.into());
    }
    let event = Event::OpenFiles(filenames_vec);
    AppState::queue_event(EventWrapper::StaticEvent(event));

    let cls = objc::runtime::Class::get("NSApplication").unwrap();
    let app: cocoa::base::id = unsafe { msg_send![cls, sharedApplication] };
    // Indicate to the OS that the file types are supported.
    // (If a filetype turns out not to be supported, it's the application's
    // responsibility to inform the user)
    unsafe { msg_send![app, replyToOpenOrPrint: NSApplicationDelegateReplySuccess] }

    trace!("Completed `application:openFiles:`");
}
