use objc2::foundation::NSObject;
use objc2::rc::{Id, Shared};
use objc2::runtime::Object;
use objc2::{declare_class, msg_send, msg_send_id, ClassType};

use super::app_state::AppState;
use super::appkit::NSApplicationActivationPolicy;

declare_class!(
    #[derive(Debug)]
    pub(super) struct ApplicationDelegate {
        activation_policy: NSApplicationActivationPolicy,
        default_menu: bool,
        activate_ignoring_other_apps: bool,
    }

    unsafe impl ClassType for ApplicationDelegate {
        type Super = NSObject;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    unsafe impl ApplicationDelegate {
        #[sel(initWithActivationPolicy:defaultMenu:activateIgnoringOtherApps:)]
        fn init(
            &mut self,
            activation_policy: NSApplicationActivationPolicy,
            default_menu: bool,
            activate_ignoring_other_apps: bool,
        ) -> Option<&mut Self> {
            let this: Option<&mut Self> = unsafe { msg_send![super(self), init] };
            this.map(|this| {
                *this.activation_policy = activation_policy;
                *this.default_menu = default_menu;
                *this.activate_ignoring_other_apps = activate_ignoring_other_apps;
                this
            })
        }

        #[sel(applicationDidFinishLaunching:)]
        fn did_finish_launching(&self, _sender: *const Object) {
            trace_scope!("applicationDidFinishLaunching:");
            AppState::launched(
                *self.activation_policy,
                *self.default_menu,
                *self.activate_ignoring_other_apps,
            );
        }

        #[sel(applicationWillTerminate:)]
        fn will_terminate(&self, _sender: *const Object) {
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
    ) -> Id<Self, Shared> {
        unsafe {
            msg_send_id![
                msg_send_id![Self::class(), alloc],
                initWithActivationPolicy: activation_policy,
                defaultMenu: default_menu,
                activateIgnoringOtherApps: activate_ignoring_other_apps,
            ]
        }
    }
}
