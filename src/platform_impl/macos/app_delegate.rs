use icrate::AppKit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use icrate::Foundation::{MainThreadMarker, NSObject, NSObjectProtocol};
use objc2::rc::Id;
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};

use super::app_state::AppState;
use super::menu;

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
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    impl DeclaredClass for ApplicationDelegate {
        type Ivars = State;
    }

    unsafe impl NSObjectProtocol for ApplicationDelegate {}

    unsafe impl NSApplicationDelegate for ApplicationDelegate {
        #[method(applicationDidFinishLaunching:)]
        fn did_finish_launching(&self, _sender: Option<&AnyObject>) {
            trace_scope!("applicationDidFinishLaunching:");

            let mtm = MainThreadMarker::from(self);
            let app = NSApplication::sharedApplication(mtm);
            // We need to delay setting the activation policy and activating the app
            // until `applicationDidFinishLaunching` has been called. Otherwise the
            // menu bar is initially unresponsive on macOS 10.15.
            app.setActivationPolicy(self.ivars().activation_policy);

            window_activation_hack(&app);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(self.ivars().activate_ignoring_other_apps);

            if self.ivars().default_menu {
                // The menubar initialization should be before the `NewEvents` event, to allow
                // overriding of the default menu even if it's created
                menu::initialize(&app);
            }

            AppState::launched();
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
        mtm: MainThreadMarker,
        activation_policy: NSApplicationActivationPolicy,
        default_menu: bool,
        activate_ignoring_other_apps: bool,
    ) -> Id<Self> {
        let this = mtm.alloc().set_ivars(State {
            activation_policy,
            default_menu,
            activate_ignoring_other_apps,
        });
        unsafe { msg_send_id![super(this), init] }
    }
}

/// A hack to make activation of multiple windows work when creating them before
/// `applicationDidFinishLaunching:` / `Event::Event::NewEvents(StartCause::Init)`.
///
/// Alternative to this would be the user calling `window.set_visible(true)` in
/// `StartCause::Init`.
///
/// If this becomes too bothersome to maintain, it can probably be removed
/// without too much damage.
fn window_activation_hack(app: &NSApplication) {
    // TODO: Proper ordering of the windows
    app.windows().into_iter().for_each(|window| {
        // Call `makeKeyAndOrderFront` if it was called on the window in `WinitWindow::new`
        // This way we preserve the user's desired initial visiblity status
        // TODO: Also filter on the type/"level" of the window, and maybe other things?
        if window.isVisible() {
            log::trace!("Activating visible window");
            window.makeKeyAndOrderFront(None);
        } else {
            log::trace!("Skipping activating invisible window");
        }
    })
}
