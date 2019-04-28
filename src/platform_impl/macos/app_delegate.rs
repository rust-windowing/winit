use cocoa::base::id;
use objc::{runtime::{Class, Object, Sel, BOOL, YES}, declare::ClassDecl};

use platform_impl::platform::app_state::AppState;

pub struct AppDelegateClass(pub *const Class);
unsafe impl Send for AppDelegateClass {}
unsafe impl Sync for AppDelegateClass {}

lazy_static! {
    pub static ref APP_DELEGATE_CLASS: AppDelegateClass = unsafe {
        let superclass = class!(NSResponder);
        let mut decl = ClassDecl::new("WinitAppDelegate", superclass).unwrap();

        decl.add_method(
            sel!(applicationDidFinishLaunching:),
            did_finish_launching as extern fn(&Object, Sel, id) -> BOOL,
        );
        decl.add_method(
            sel!(applicationDidBecomeActive:),
            did_become_active as extern fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationWillResignActive:),
            will_resign_active as extern fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationWillEnterForeground:),
            will_enter_foreground as extern fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationDidEnterBackground:),
            did_enter_background as extern fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationWillTerminate:),
            will_terminate as extern fn(&Object, Sel, id),
        );

        AppDelegateClass(decl.register())
    };
}

extern fn did_finish_launching(_: &Object, _: Sel, _: id) -> BOOL {
    trace!("Triggered `didFinishLaunching`");
    AppState::launched();
    trace!("Completed `didFinishLaunching`");
    YES
}

extern fn did_become_active(_: &Object, _: Sel, _: id) {
    trace!("Triggered `didBecomeActive`");
    /*unsafe {
        HANDLER.lock().unwrap().handle_nonuser_event(Event::Suspended(false))
    }*/
    trace!("Completed `didBecomeActive`");
}

extern fn will_resign_active(_: &Object, _: Sel, _: id) {
    trace!("Triggered `willResignActive`");
    /*unsafe {
        HANDLER.lock().unwrap().handle_nonuser_event(Event::Suspended(true))
    }*/
    trace!("Completed `willResignActive`");
}

extern fn will_enter_foreground(_: &Object, _: Sel, _: id) {
    trace!("Triggered `willEnterForeground`");
    trace!("Completed `willEnterForeground`");
}

extern fn did_enter_background(_: &Object, _: Sel, _: id) {
    trace!("Triggered `didEnterBackground`");
    trace!("Completed `didEnterBackground`");
}

extern fn will_terminate(_: &Object, _: Sel, _: id) {
    trace!("Triggered `willTerminate`");
    /*unsafe {
        let app: id = msg_send![class!(UIApplication), sharedApplication];
        let windows: id = msg_send![app, windows];
        let windows_enum: id = msg_send![windows, objectEnumerator];
        let mut events = Vec::new();
        loop {
            let window: id = msg_send![windows_enum, nextObject];
            if window == nil {
                break
            }
            let is_winit_window: BOOL = msg_send![window, isKindOfClass:class!(WinitUIWindow)];
            if is_winit_window == YES {
                events.push(Event::WindowEvent {
                    window_id: RootWindowId(window.into()),
                    event: WindowEvent::Destroyed,
                });
            }
        }
        HANDLER.lock().unwrap().handle_nonuser_events(events);
        HANDLER.lock().unwrap().terminated();
    }*/
    trace!("Completed `willTerminate`");
}
