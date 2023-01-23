use std::ptr::NonNull;

use icrate::Foundation::NSObject;
use objc2::declare::{IvarBool, IvarEncode};
use objc2::rc::Id;
use objc2::runtime::Object;
use objc2::{declare_class, msg_send, msg_send_id, mutability, ClassType};

use super::app_state::AppState;
use super::appkit::NSApplicationActivationPolicy;

declare_class!(
    #[derive(Debug)]
    pub(super) struct ApplicationDelegate {
        activation_policy: IvarEncode<NSApplicationActivationPolicy, "_activation_policy">,
        default_menu: IvarBool<"_default_menu">,
        activate_ignoring_other_apps: IvarBool<"_activate_ignoring_other_apps">,
    }

    mod ivars;

    unsafe impl ClassType for ApplicationDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    unsafe impl ApplicationDelegate {
        #[method(initWithActivationPolicy:defaultMenu:activateIgnoringOtherApps:)]
        unsafe fn init(
            this: *mut Self,
            activation_policy: NSApplicationActivationPolicy,
            default_menu: bool,
            activate_ignoring_other_apps: bool,
        ) -> Option<NonNull<Self>> {
            let this: Option<&mut Self> = unsafe { msg_send![super(this), init] };
            this.map(|this| {
                *this.activation_policy = activation_policy;
                *this.default_menu = default_menu;
                *this.activate_ignoring_other_apps = activate_ignoring_other_apps;
                NonNull::from(this)
            })
        }

        #[method(applicationDidFinishLaunching:)]
        fn did_finish_launching(&self, _sender: Option<&Object>) {
            trace_scope!("applicationDidFinishLaunching:");
            AppState::launched(
                *self.activation_policy,
                *self.default_menu,
                *self.activate_ignoring_other_apps,
            );
        }

        #[method(applicationWillTerminate:)]
        fn will_terminate(&self, _sender: Option<&Object>) {
            trace_scope!("applicationWillTerminate:");
            // TODO: Notify every window that it will be destroyed, like done in iOS?
            AppState::exit();
        }
    }
);

impl ApplicationDelegate {
    pub(super) fn new(
        activation_policy: NSApplicationActivationPolicy,
        default_menu: bool,
        activate_ignoring_other_apps: bool,
    ) -> Id<Self> {
        unsafe {
            msg_send_id![
                Self::alloc(),
                initWithActivationPolicy: activation_policy,
                defaultMenu: default_menu,
                activateIgnoringOtherApps: activate_ignoring_other_apps,
            ]
        }
    }
}
