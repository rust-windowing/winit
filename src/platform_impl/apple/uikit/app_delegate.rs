use objc2::{declare_class, mutability, ClassType, DeclaredClass};
use objc2_foundation::{MainThreadMarker, NSObject};
use objc2_ui_kit::UIApplication;

use super::app_state::{self, send_occluded_event_for_all_windows, EventWrapper};
use crate::event::Event;

declare_class!(
    pub struct AppDelegate;

    unsafe impl ClassType for AppDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    impl DeclaredClass for AppDelegate {}

    // UIApplicationDelegate protocol
    unsafe impl AppDelegate {
        #[method(application:didFinishLaunchingWithOptions:)]
        fn did_finish_launching(&self, _application: &UIApplication, _: *mut NSObject) -> bool {
            app_state::did_finish_launching(MainThreadMarker::new().unwrap());
            true
        }

        #[method(applicationDidBecomeActive:)]
        fn did_become_active(&self, _application: &UIApplication) {
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, EventWrapper::StaticEvent(Event::Resumed))
        }

        #[method(applicationWillResignActive:)]
        fn will_resign_active(&self, _application: &UIApplication) {
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, EventWrapper::StaticEvent(Event::Suspended))
        }

        #[method(applicationWillEnterForeground:)]
        fn will_enter_foreground(&self, application: &UIApplication) {
            send_occluded_event_for_all_windows(application, false);
        }

        #[method(applicationDidEnterBackground:)]
        fn did_enter_background(&self, application: &UIApplication) {
            send_occluded_event_for_all_windows(application, true);
        }

        #[method(applicationWillTerminate:)]
        fn will_terminate(&self, application: &UIApplication) {
            app_state::terminated(application);
        }

        #[method(applicationDidReceiveMemoryWarning:)]
        fn did_receive_memory_warning(&self, _application: &UIApplication) {
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, EventWrapper::StaticEvent(Event::MemoryWarning))
        }
    }
);
