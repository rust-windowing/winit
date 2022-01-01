use crate::{platform::macos::ActivationPolicy, platform_impl::platform::app_state::AppState};

use cocoa::base::id;
use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Sel},
};
use std::{
    cell::{RefCell, RefMut, Ref},
    os::raw::c_void, sync::atomic::{AtomicUsize, Ordering}, collections::HashMap, any::Any,
};

static AUX_DELEGATE_STATE_NAME: &str = "auxState";

#[derive(Default)]
pub struct AuxDelegateState {
    /// We store this value in order to be able to defer setting the activation policy until
    /// after the app has finished launching. If the activation policy is set earlier, the
    /// menubar is initially unresponsive on macOS 10.15 for example.
    pub activation_policy: ActivationPolicy,

    pub create_default_menu: bool,

    /// Each key is a selector name and each value is a colsure that handles the
    /// callback
    pub methods: HashMap<String, Box<dyn Any>>,
}

pub struct AppDelegateClass(pub *const Class);
unsafe impl Send for AppDelegateClass {}
unsafe impl Sync for AppDelegateClass {}

pub fn create_delegate_class() -> ClassDecl {
    static CLASS_SEQ_NUM: AtomicUsize = AtomicUsize::new(0);
    unsafe {
        let superclass = class!(NSResponder);
        let curr_seq_num = CLASS_SEQ_NUM.fetch_add(1, Ordering::Relaxed);
        let class_name = format!("WinitAppDelegate{}", curr_seq_num);
        let mut decl = ClassDecl::new(&class_name, superclass).unwrap();
    
        decl.add_class_method(sel!(new), new as extern "C" fn(&Class, Sel) -> id);
        decl.add_method(sel!(dealloc), dealloc as extern "C" fn(&Object, Sel));
    
        decl.add_method(
            sel!(applicationDidFinishLaunching:),
            did_finish_launching as extern "C" fn(&Object, Sel, id),
        );
        decl.add_ivar::<*mut c_void>(AUX_DELEGATE_STATE_NAME);
        decl
    }
}

lazy_static! {
    pub static ref APP_DELEGATE_CLASS: AppDelegateClass = {
        let decl = create_delegate_class();
        AppDelegateClass(decl.register())
    };
}

/// Safety: Assumes that Object is an instance of APP_DELEGATE_CLASS
pub unsafe fn get_aux_state_mut(this: &Object) -> RefMut<'_, AuxDelegateState> {
    let ptr: *mut c_void = *this.get_ivar(AUX_DELEGATE_STATE_NAME);
    // Watch out that this needs to be the correct type
    (*(ptr as *mut RefCell<AuxDelegateState>)).borrow_mut()
}

/// Safety: Assumes that Object is an instance of APP_DELEGATE_CLASS
pub unsafe fn get_aux_state_ref(this: &Object) -> Ref<'_, AuxDelegateState> {
    let ptr: *mut c_void = *this.get_ivar(AUX_DELEGATE_STATE_NAME);
    // Watch out that this needs to be the correct type
    (*(ptr as *mut RefCell<AuxDelegateState>)).borrow()
}

extern "C" fn new(class: &Class, _: Sel) -> id {
    unsafe {
        let this: id = msg_send![class, alloc];
        let this: id = msg_send![this, init];
        (*this).set_ivar(
            AUX_DELEGATE_STATE_NAME,
            Box::into_raw(Box::new(RefCell::new(AuxDelegateState {
                activation_policy: ActivationPolicy::Regular,
                create_default_menu: true,
                methods: HashMap::default(),
            }))) as *mut c_void,
        );
        this
    }
}

extern "C" fn dealloc(this: &Object, _: Sel) {
    unsafe {
        let state_ptr: *mut c_void = *(this.get_ivar(AUX_DELEGATE_STATE_NAME));
        // As soon as the box is constructed it is immediately dropped, releasing the underlying
        // memory
        Box::from_raw(state_ptr as *mut RefCell<AuxDelegateState>);
    }
}

extern "C" fn did_finish_launching(this: &Object, _: Sel, _: id) {
    trace!("Triggered `applicationDidFinishLaunching`");
    AppState::launched(this);
    trace!("Completed `applicationDidFinishLaunching`");
}
