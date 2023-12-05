use icrate::Foundation::NSObject;
use objc2::rc::Id;
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};

use super::app_state::AppState;
use super::appkit::NSApplicationActivationPolicy;

#[derive(Debug)]
pub(super) struct State {
    activation_policy: NSApplicationActivationPolicy,
    default_menu: bool,
    activate_ignoring_other_apps: bool,
}

declare_class!(
    pub(super) struct ApplicationDelegate;

    unsafe impl ClassType for ApplicationDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    impl DeclaredClass for ApplicationDelegate {
        type Ivars = State;
    }

    unsafe impl ApplicationDelegate {
        #[method(applicationDidFinishLaunching:)]
        fn did_finish_launching(&self, _sender: Option<&AnyObject>) {
            trace_scope!("applicationDidFinishLaunching:");
            AppState::launched(
                self.ivars().activation_policy,
                self.ivars().default_menu,
                self.ivars().activate_ignoring_other_apps,
            );
        }

        #[method(applicationWillTerminate:)]
        fn will_terminate(&self, _sender: Option<&AnyObject>) {
            trace_scope!("applicationWillTerminate:");
            // TODO: Notify every window that it will be destroyed, like done in iOS?
            AppState::internal_exit();
        }
    }
);

impl ApplicationDelegate {
    pub(super) fn new(
        activation_policy: NSApplicationActivationPolicy,
        default_menu: bool,
        activate_ignoring_other_apps: bool,
    ) -> Id<Self> {
        let this = Self::alloc().set_ivars(State {
            activation_policy,
            default_menu,
            activate_ignoring_other_apps,
        });
        unsafe { msg_send_id![super(this), init] }
    }
}
