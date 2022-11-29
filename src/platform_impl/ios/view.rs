#![allow(clippy::unnecessary_cast)]

use objc2::foundation::{CGFloat, CGPoint, CGRect, MainThreadMarker, NSObject};
use objc2::rc::{Id, Shared};
use objc2::{declare_class, msg_send, msg_send_id, extern_methods, ClassType};

use super::uikit::{UIInterfaceOrientationMask, UIApplication, UIDevice, UIResponder, UIView, UIViewController, UIWindow};
use super::window::WindowId;
use crate::{
    dpi::PhysicalPosition,
    event::{DeviceId as RootDeviceId, Event, Force, Touch, TouchPhase, WindowEvent},
    platform_impl::platform::{
        app_state,
        event_loop::{EventProxy, EventWrapper},
        ffi::{
            id, nil, UIForceTouchCapability, UIRectEdge, UITouchPhase,
            UITouchType, UIUserInterfaceIdiom
        },
        window::PlatformSpecificWindowBuilderAttributes,
        DeviceId, Fullscreen,
    },
    platform::ios::ValidOrientations,
    window::{WindowAttributes, WindowId as RootWindowId},
};

declare_class!(
    struct WinitView {}

    unsafe impl ClassType for WinitView {
        #[inherits(UIResponder, NSObject)]
        type Super = UIView;
        const NAME: &'static str = "WinitUIView";
    }

    unsafe impl WinitView {
        #[sel(drawRect:)]
        fn draw_rect(&self, rect: CGRect) {
            unsafe {
                let window: id = msg_send![self, window];
                assert!(!window.is_null());
                app_state::handle_nonuser_events(
                    std::iter::once(EventWrapper::StaticEvent(Event::RedrawRequested(
                        RootWindowId(window.into()),
                    )))
                    .chain(std::iter::once(EventWrapper::StaticEvent(
                        Event::RedrawEventsCleared,
                    ))),
                );
                let _: () = msg_send![super(self), drawRect: rect];
            }
        }

        #[sel(layoutSubviews)]
        fn layout_subviews(&self) {
            unsafe {
                let _: () = msg_send![super(self), layoutSubviews];

                let window: id = msg_send![self, window];
                assert!(!window.is_null());
                let window_bounds: CGRect = msg_send![window, bounds];
                let screen: id = msg_send![window, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect = msg_send![
                    self,
                    convertRect: window_bounds,
                    toCoordinateSpace: screen_space,
                ];
                let scale_factor: CGFloat = msg_send![screen, scale];
                let size = crate::dpi::LogicalSize {
                    width: screen_frame.size.width as f64,
                    height: screen_frame.size.height as f64,
                }
                .to_physical(scale_factor as f64);

                // If the app is started in landscape, the view frame and window bounds can be mismatched.
                // The view frame will be in portrait and the window bounds in landscape. So apply the
                // window bounds to the view frame to make it consistent.
                let view_frame: CGRect = msg_send![self, frame];
                if view_frame != window_bounds {
                    let _: () = msg_send![self, setFrame: window_bounds];
                }

                app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.into()),
                    event: WindowEvent::Resized(size),
                }));
            }
        }

        #[sel(setContentScaleFactor:)]
        fn set_content_scale_factor(&self, untrusted_scale_factor: CGFloat) {
            unsafe {
                let _: () = msg_send![super(self), setContentScaleFactor: untrusted_scale_factor];

                let window: Option<Id<WinitUIWindow, Shared>> = msg_send_id![self, window];
                // `window` is null when `setContentScaleFactor` is invoked prior to `[UIWindow
                // makeKeyAndVisible]` at window creation time (either manually or internally by
                // UIKit when the `UIView` is first created), in which case we send no events here
                let window = match window {
                    Some(window) => window,
                    None => return,
                };
                // `setContentScaleFactor` may be called with a value of 0, which means "reset the
                // content scale factor to a device-specific default value", so we can't use the
                // parameter here. We can query the actual factor using the getter
                let scale_factor: CGFloat = msg_send![self, contentScaleFactor];
                assert!(
                    !scale_factor.is_nan()
                        && scale_factor.is_finite()
                        && scale_factor.is_sign_positive()
                        && scale_factor > 0.0,
                    "invalid scale_factor set on UIView",
                );
                let bounds: CGRect = msg_send![self, bounds];
                let screen = window.screen();
                let screen_space = screen.coordinateSpace();
                let screen_frame: CGRect =
                    msg_send![self, convertRect: bounds, toCoordinateSpace: &*screen_space];
                let size = crate::dpi::LogicalSize {
                    width: screen_frame.size.width as _,
                    height: screen_frame.size.height as _,
                };
                let window_id = RootWindowId(window.id());
                app_state::handle_nonuser_events(
                    std::iter::once(EventWrapper::EventProxy(EventProxy::DpiChangedProxy {
                        window,
                        scale_factor,
                        suggested_size: size,
                    }))
                    .chain(std::iter::once(EventWrapper::StaticEvent(
                        Event::WindowEvent {
                            window_id,
                            event: WindowEvent::Resized(size.to_physical(scale_factor)),
                        },
                    ))),
                );
            }
        }

        #[sel(touchesBegan:withEvent:)]
        fn touches_began(&self, touches: id, _: id) {
            self.handle_touches(touches)
        }

        #[sel(touchesMoved:withEvent:)]
        fn touches_moved(&self, touches: id, _: id) {
            self.handle_touches(touches)
        }

        #[sel(touchesEnded:withEvent:)]
        fn touches_ended(&self, touches: id, _: id) {
            self.handle_touches(touches)
        }

        #[sel(touchesCancelled:withEvent:)]
        fn touches_cancelled(&self, touches: id, _: id) {
            self.handle_touches(touches)
        }
    }
);

impl WinitView {
    fn handle_touches(&self, touches: id) {
        unsafe {
            let window: id = msg_send![self, window];
            assert!(!window.is_null());
            let uiscreen: id = msg_send![window, screen];
            let touches_enum: id = msg_send![touches, objectEnumerator];
            let mut touch_events = Vec::new();
            let os_supports_force = app_state::os_capabilities().force_touch;
            loop {
                let touch: id = msg_send![touches_enum, nextObject];
                if touch == nil {
                    break;
                }
                let logical_location: CGPoint = msg_send![touch, locationInView: nil];
                let touch_type: UITouchType = msg_send![touch, type];
                let force = if os_supports_force {
                    let trait_collection: id = msg_send![self, traitCollection];
                    let touch_capability: UIForceTouchCapability =
                        msg_send![trait_collection, forceTouchCapability];
                    // Both the OS _and_ the device need to be checked for force touch support.
                    if touch_capability == UIForceTouchCapability::Available {
                        let force: CGFloat = msg_send![touch, force];
                        let max_possible_force: CGFloat = msg_send![touch, maximumPossibleForce];
                        let altitude_angle: Option<f64> = if touch_type == UITouchType::Pencil {
                            let angle: CGFloat = msg_send![touch, altitudeAngle];
                            Some(angle as _)
                        } else {
                            None
                        };
                        Some(Force::Calibrated {
                            force: force as _,
                            max_possible_force: max_possible_force as _,
                            altitude_angle,
                        })
                    } else {
                        None
                    }
                } else {
                    None
                };
                let touch_id = touch as u64;
                let phase: UITouchPhase = msg_send![touch, phase];
                let phase = match phase {
                    UITouchPhase::Began => TouchPhase::Started,
                    UITouchPhase::Moved => TouchPhase::Moved,
                    // 2 is UITouchPhase::Stationary and is not expected here
                    UITouchPhase::Ended => TouchPhase::Ended,
                    UITouchPhase::Cancelled => TouchPhase::Cancelled,
                    _ => panic!("unexpected touch phase: {:?}", phase as i32),
                };

                let physical_location = {
                    let scale_factor: CGFloat = msg_send![self, contentScaleFactor];
                    PhysicalPosition::from_logical::<(f64, f64), f64>(
                        (logical_location.x as _, logical_location.y as _),
                        scale_factor as f64,
                    )
                };
                touch_events.push(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.into()),
                    event: WindowEvent::Touch(Touch {
                        device_id: RootDeviceId(DeviceId { uiscreen }),
                        id: touch_id,
                        location: physical_location,
                        force,
                        phase,
                    }),
                }));
            }
            app_state::handle_nonuser_events(touch_events);
        }
    }
}

declare_class!(
    pub(crate) struct WinitViewController {
        _prefers_status_bar_hidden: bool,
        _prefers_home_indicator_auto_hidden: bool,
        _supported_orientations: UIInterfaceOrientationMask,
        _preferred_screen_edges_deferring_system_gestures: UIRectEdge,
    }

    unsafe impl ClassType for WinitViewController {
        #[inherits(UIResponder, NSObject)]
        type Super = UIViewController;
        const NAME: &'static str = "WinitUIViewController";
    }

    unsafe impl WinitViewController {
        #[sel(shouldAutorotate)]
        fn should_autorotate(&self) -> bool {
            true
        }
    }

    unsafe impl WinitViewController {
        #[sel(prefersStatusBarHidden)]
        fn prefers_status_bar_hidden(&self) -> bool {
            *self._prefers_status_bar_hidden
        }

        #[sel(setPrefersStatusBarHidden:)]
        fn set_prefers_status_bar_hidden(&mut self, val: bool) {
            *self._prefers_status_bar_hidden = val;
            self.setNeedsStatusBarAppearanceUpdate();
        }

        #[sel(prefersHomeIndicatorAutoHidden)]
        fn prefers_home_indicator_auto_hidden(&self) -> bool {
            *self._prefers_home_indicator_auto_hidden
        }

        #[sel(setPrefersHomeIndicatorAutoHidden:)]
        fn set_prefers_home_indicator_auto_hidden(&mut self, val: bool) {
            *self._prefers_home_indicator_auto_hidden = val;
            let os_capabilities = app_state::os_capabilities();
            if os_capabilities.home_indicator_hidden {
                self.setNeedsUpdateOfHomeIndicatorAutoHidden();
            } else {
                os_capabilities.home_indicator_hidden_err_msg("ignoring")
            }
        }

        #[sel(supportedInterfaceOrientations)]
        fn supported_orientations(&self) -> UIInterfaceOrientationMask {
            *self._supported_orientations
        }

        #[sel(setSupportedInterfaceOrientations:)]
        fn set_supported_orientations(&mut self, val: UIInterfaceOrientationMask) {
            *self._supported_orientations = val;
            UIViewController::attemptRotationToDeviceOrientation();
        }

        #[sel(preferredScreenEdgesDeferringSystemGestures)]
        fn preferred_screen_edges_deferring_system_gestures(&self) -> UIRectEdge {
            *self._preferred_screen_edges_deferring_system_gestures
        }

        #[sel(setPreferredScreenEdgesDeferringSystemGestures:)]
        fn set_preferred_screen_edges_deferring_system_gestures(&mut self, val: UIRectEdge) {
            *self._preferred_screen_edges_deferring_system_gestures = val;
            let os_capabilities = app_state::os_capabilities();
            if os_capabilities.defer_system_gestures {
                self.setNeedsUpdateOfScreenEdgesDeferringSystemGestures();
            } else {
                os_capabilities.defer_system_gestures_err_msg("ignoring")
            }
        }
    }
);

extern_methods!(
    #[allow(non_snake_case)]
    unsafe impl WinitViewController {
        #[sel(setPrefersStatusBarHidden:)]
        pub(crate) fn setPrefersStatusBarHidden(&self, flag: bool);

        #[sel(setSupportedInterfaceOrientations:)]
        pub(crate) fn setSupportedInterfaceOrientations(&self, val: UIInterfaceOrientationMask);

        #[sel(setPrefersHomeIndicatorAutoHidden:)]
        pub(crate) fn setPrefersHomeIndicatorAutoHidden(&self, val: bool);

        #[sel(setPreferredScreenEdgesDeferringSystemGestures:)]
        pub(crate) fn setPreferredScreenEdgesDeferringSystemGestures(&self, val: UIRectEdge);
    }
);

impl WinitViewController {
    pub(crate) fn set_supported_interface_orientations(&self, mtm: MainThreadMarker, valid_orientations: ValidOrientations) {
        let mask = match (valid_orientations, UIDevice::current(mtm).userInterfaceIdiom()) {
            (ValidOrientations::LandscapeAndPortrait, UIUserInterfaceIdiom::Phone) => {
                UIInterfaceOrientationMask::AllButUpsideDown
            }
            (ValidOrientations::LandscapeAndPortrait, _) => UIInterfaceOrientationMask::All,
            (ValidOrientations::Landscape, _) => UIInterfaceOrientationMask::Landscape,
            (ValidOrientations::Portrait, UIUserInterfaceIdiom::Phone) => {
                UIInterfaceOrientationMask::Portrait
            }
            (ValidOrientations::Portrait, _) => {
                UIInterfaceOrientationMask::Portrait
                    | UIInterfaceOrientationMask::PortraitUpsideDown
            }
        };
        self.setSupportedInterfaceOrientations(mask);
    }

    pub(crate) fn new(
        mtm: MainThreadMarker,
        _window_attributes: &WindowAttributes,
        platform_attributes: &PlatformSpecificWindowBuilderAttributes,
        view: &UIView,
    ) -> Id<Self, Shared> {
        let this: Id<Self, Shared> = unsafe {
            msg_send_id![msg_send_id![Self::class(), alloc], init]
        };

        this.setPrefersStatusBarHidden(platform_attributes.prefers_status_bar_hidden);

        this.set_supported_interface_orientations(mtm, platform_attributes.valid_orientations);

        this.setPrefersHomeIndicatorAutoHidden(platform_attributes.prefers_home_indicator_hidden);

        this.setPreferredScreenEdgesDeferringSystemGestures(
            platform_attributes
                .preferred_screen_edges_deferring_system_gestures
                .into()
        );

        this.setView(Some(view));

        this
    }
}

declare_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct WinitUIWindow {}

    unsafe impl ClassType for WinitUIWindow {
        #[inherits(UIResponder, NSObject)]
        type Super = UIWindow;
    }

    unsafe impl WinitUIWindow {
        #[sel(becomeKeyWindow)]
        fn become_key_window(&self) {
            app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(self.id()),
                event: WindowEvent::Focused(true),
            }));
            let _: () = unsafe { msg_send![super(self), becomeKeyWindow] };
        }

        #[sel(resignKeyWindow)]
        fn resign_key_window(&self) {
            app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(self.id()),
                event: WindowEvent::Focused(false),
            }));
            let _: () = unsafe { msg_send![super(self), resignKeyWindow] };
        }
    }
);

// requires main thread
pub(crate) unsafe fn create_view(
    _window_attributes: &WindowAttributes,
    platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    frame: CGRect,
) -> id {
    let view: id = msg_send![WinitView::class(), alloc];
    assert!(!view.is_null(), "Failed to create `UIView` instance");
    let view: id = msg_send![view, initWithFrame: frame];
    assert!(!view.is_null(), "Failed to initialize `UIView` instance");
    let _: () = msg_send![view, setMultipleTouchEnabled: true];
    if let Some(scale_factor) = platform_attributes.scale_factor {
        let _: () = msg_send![view, setContentScaleFactor: scale_factor as CGFloat];
    }

    view
}

impl WinitUIWindow {
    pub(crate) fn new(
        _mtm: MainThreadMarker,
        window_attributes: &WindowAttributes,
        _platform_attributes: &PlatformSpecificWindowBuilderAttributes,
        frame: CGRect,
        view_controller: &UIViewController,
    ) -> Id<Self, Shared> {
        let this: Id<Self, Shared> =
            unsafe { msg_send_id![msg_send_id![Self::class(), alloc], initWithFrame: frame] };

        this.setRootViewController(Some(view_controller));

        match window_attributes.fullscreen {
            Some(Fullscreen::Exclusive(ref video_mode)) => {
                let monitor = video_mode.monitor();
                let screen = monitor.ui_screen();
                screen.setCurrentMode(Some(&video_mode.screen_mode.0));
                this.setScreen(&screen);
            }
            Some(Fullscreen::Borderless(ref monitor)) => {
                if let Some(monitor) = &monitor {
                    let screen = monitor.ui_screen();
                    this.setScreen(&screen);
                };
            }
            None => (),
        }

        this
    }

    pub(crate) fn id(&self) -> WindowId {
        (self as *const Self as usize as u64).into()
    }
}

declare_class!(
    pub struct WinitApplicationDelegate {}

    unsafe impl ClassType for WinitApplicationDelegate {
        type Super = NSObject;
    }

    // UIApplicationDelegate protocol
    unsafe impl WinitApplicationDelegate {
        #[sel(application:didFinishLaunchingWithOptions:)]
        fn did_finish_launching(&self, _application: &UIApplication, _: id) -> bool {
            unsafe {
                app_state::did_finish_launching();
            }
            true
        }

        #[sel(applicationDidBecomeActive:)]
        fn did_become_active(&self, _application: &UIApplication) {
            unsafe { app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::Resumed)) }
        }

        #[sel(applicationWillResignActive:)]
        fn will_resign_active(&self, _application: &UIApplication) {
            unsafe { app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::Suspended)) }
        }

        #[sel(applicationWillEnterForeground:)]
        fn will_enter_foreground(&self, _application: &UIApplication) {}
        #[sel(applicationDidEnterBackground:)]
        fn did_enter_background(&self, _application: &UIApplication) {}

        #[sel(applicationWillTerminate:)]
        fn will_terminate(&self, application: &UIApplication) {
            let mut events = Vec::new();
            for window in application.windows().iter() {
                if window.is_kind_of::<WinitUIWindow>() {
                    // SAFETY: We just checked that the window is a `winit` window
                    let window = unsafe {
                        let ptr: *const UIWindow = window;
                        let ptr: *const WinitUIWindow = ptr.cast();
                        &*ptr
                    };
                    events.push(EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id: RootWindowId(window.id()),
                        event: WindowEvent::Destroyed,
                    }));
                }
            }
            unsafe {
                app_state::handle_nonuser_events(events);
                app_state::terminated();
            }
        }
    }
);
