use core::cell::Cell;
use std::cell::{RefCell, RefMut};

use icrate::AppKit::{NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate};
use icrate::Foundation::{MainThreadMarker, NSObject, NSObjectProtocol};
use objc2::rc::{autoreleasepool, Id};
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send_id, mutability, ClassType, DeclaredClass};

use crate::event_loop::ControlFlow;

use super::app_state::AppState;
use super::event::dummy_event;
use super::menu;
use super::observer::EventLoopWaker;

#[derive(Debug, Default)]
pub(super) struct State {
    activation_policy: NSApplicationActivationPolicy,
    default_menu: bool,
    activate_ignoring_other_apps: bool,
    stop_on_launch: Cell<bool>,
    /// Whether `applicationDidFinishLaunching:` has been run or not.
    is_launched: Cell<bool>,
    /// Whether an `EventLoop` is currently running.
    is_running: Cell<bool>,
    /// Whether the user has requested the event loop to exit.
    exit: Cell<bool>,
    control_flow: Cell<ControlFlow>,
    waker: RefCell<EventLoopWaker>,
}

declare_class!(
    #[derive(Debug)]
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
        // Note: This will, globally, only be run once, no matter how many
        // `EventLoop`s the user creates.
        #[method(applicationDidFinishLaunching:)]
        fn did_finish_launching(&self, _sender: Option<&AnyObject>) {
            trace_scope!("applicationDidFinishLaunching:");
            self.ivars().is_launched.set(true);

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

            self.ivars().waker.borrow_mut().start();

            self.set_is_running(true);
            AppState::dispatch_init_events();

            // If the application is being launched via `EventLoop::pump_events()` then we'll
            // want to stop the app once it is launched (and return to the external loop)
            //
            // In this case we still want to consider Winit's `EventLoop` to be "running",
            // so we call `start_running()` above.
            if self.ivars().stop_on_launch.get() {
                // Note: the original idea had been to only stop the underlying `RunLoop`
                // for the app but that didn't work as expected (`-[NSApplication run]`
                // effectively ignored the attempt to stop the RunLoop and re-started it).
                //
                // So we return from `pump_events` by stopping the application.
                self.stop_app_immediately();
            }
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
            ..Default::default()
        });
        unsafe { msg_send_id![super(this), init] }
    }

    pub fn get(mtm: MainThreadMarker) -> Id<Self> {
        let app = NSApplication::sharedApplication(mtm);
        let delegate =
            unsafe { app.delegate() }.expect("a delegate was not configured on the application");
        if delegate.is_kind_of::<Self>() {
            // SAFETY: Just checked that the delegate is an instance of `ApplicationDelegate`
            unsafe { Id::cast(delegate) }
        } else {
            panic!("tried to get a delegate that was not the one Winit has registered")
        }
    }

    /// If `pump_events` is called to progress the event loop then we
    /// bootstrap the event loop via `-[NSAppplication run]` but will use
    /// `CFRunLoopRunInMode` for subsequent calls to `pump_events`.
    pub fn request_stop_on_launch(&self) {
        self.ivars().stop_on_launch.set(true);
    }

    pub fn stop_app_immediately(&self) {
        let mtm = MainThreadMarker::from(self);
        let app = NSApplication::sharedApplication(mtm);
        autoreleasepool(|_| {
            app.stop(None);
            // To stop event loop immediately, we need to post some event here.
            app.postEvent_atStart(&dummy_event().unwrap(), true);
        });
    }

    pub fn is_launched(&self) -> bool {
        self.ivars().is_launched.get()
    }

    pub fn set_is_running(&self, value: bool) {
        self.ivars().is_running.set(value)
    }

    pub fn is_running(&self) -> bool {
        self.ivars().is_running.get()
    }

    pub fn exit(&self) {
        self.ivars().exit.set(true)
    }

    pub fn clear_exit(&self) {
        self.ivars().exit.set(false)
    }

    pub fn exiting(&self) -> bool {
        self.ivars().exit.get()
    }

    pub fn set_control_flow(&self, value: ControlFlow) {
        self.ivars().control_flow.set(value)
    }

    pub fn control_flow(&self) -> ControlFlow {
        self.ivars().control_flow.get()
    }

    pub fn waker(&self) -> RefMut<'_, EventLoopWaker> {
        self.ivars().waker.borrow_mut()
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
