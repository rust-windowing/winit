#![allow(clippy::unnecessary_cast)]

use objc2::foundation::{CGFloat, CGPoint, CGRect, NSObject};
use objc2::{class, declare_class, msg_send, ClassType};

use super::uikit::{UIApplication, UIResponder, UIView, UIViewController, UIWindow};
use super::window::WindowId;
use crate::{
    dpi::PhysicalPosition,
    event::{DeviceId as RootDeviceId, Event, Force, Touch, TouchPhase, WindowEvent},
    platform_impl::platform::{
        app_state,
        event_loop::{self, EventProxy, EventWrapper},
        ffi::{
            id, nil, UIForceTouchCapability, UIInterfaceOrientationMask, UIRectEdge, UITouchPhase,
            UITouchType,
        },
        window::PlatformSpecificWindowBuilderAttributes,
        DeviceId, Fullscreen,
    },
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

                let window: id = msg_send![self, window];
                // `window` is null when `setContentScaleFactor` is invoked prior to `[UIWindow
                // makeKeyAndVisible]` at window creation time (either manually or internally by
                // UIKit when the `UIView` is first created), in which case we send no events here
                if window.is_null() {
                    return;
                }
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
                let screen: id = msg_send![window, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect =
                    msg_send![self, convertRect: bounds, toCoordinateSpace: screen_space];
                let size = crate::dpi::LogicalSize {
                    width: screen_frame.size.width as _,
                    height: screen_frame.size.height as _,
                };
                app_state::handle_nonuser_events(
                    std::iter::once(EventWrapper::EventProxy(EventProxy::DpiChangedProxy {
                        window_id: window,
                        scale_factor,
                        suggested_size: size,
                    }))
                    .chain(std::iter::once(EventWrapper::StaticEvent(
                        Event::WindowEvent {
                            window_id: RootWindowId(window.into()),
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
    struct WinitViewController {
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
            unsafe {
                let _: () = msg_send![self, setNeedsStatusBarAppearanceUpdate];
            }
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
                unsafe {
                    let _: () = msg_send![self, setNeedsUpdateOfHomeIndicatorAutoHidden];
                }
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
            unsafe {
                let _: () = msg_send![
                    UIViewController::class(),
                    attemptRotationToDeviceOrientation
                ];
            }
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
                unsafe {
                    let _: () = msg_send![self, setNeedsUpdateOfScreenEdgesDeferringSystemGestures];
                }
            } else {
                os_capabilities.defer_system_gestures_err_msg("ignoring")
            }
        }
    }
);

declare_class!(
    struct WinitUIWindow {}

    unsafe impl ClassType for WinitUIWindow {
        #[inherits(UIResponder, NSObject)]
        type Super = UIWindow;
    }

    unsafe impl WinitUIWindow {
        #[sel(becomeKeyWindow)]
        fn become_key_window(&self) {
            unsafe {
                app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId((&*****self).into()),
                    event: WindowEvent::Focused(true),
                }));
                let _: () = msg_send![super(self), becomeKeyWindow];
            }
        }

        #[sel(resignKeyWindow)]
        fn resign_key_window(&self) {
            unsafe {
                app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId((&*****self).into()),
                    event: WindowEvent::Focused(false),
                }));
                let _: () = msg_send![super(self), resignKeyWindow];
            }
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

// requires main thread
pub(crate) unsafe fn create_view_controller(
    _window_attributes: &WindowAttributes,
    platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    view: id,
) -> id {
    let class = WinitViewController::class();

    let view_controller: id = msg_send![class, alloc];
    assert!(
        !view_controller.is_null(),
        "Failed to create `UIViewController` instance"
    );
    let view_controller: id = msg_send![view_controller, init];
    assert!(
        !view_controller.is_null(),
        "Failed to initialize `UIViewController` instance"
    );
    let status_bar_hidden = platform_attributes.prefers_status_bar_hidden;
    let idiom = event_loop::get_idiom();
    let supported_orientations = UIInterfaceOrientationMask::from_valid_orientations_idiom(
        platform_attributes.valid_orientations,
        idiom,
    );
    let prefers_home_indicator_hidden = platform_attributes.prefers_home_indicator_hidden;
    let edges: UIRectEdge = platform_attributes
        .preferred_screen_edges_deferring_system_gestures
        .into();
    let _: () = msg_send![
        view_controller,
        setPrefersStatusBarHidden: status_bar_hidden
    ];
    let _: () = msg_send![
        view_controller,
        setSupportedInterfaceOrientations: supported_orientations
    ];
    let _: () = msg_send![
        view_controller,
        setPrefersHomeIndicatorAutoHidden: prefers_home_indicator_hidden
    ];
    let _: () = msg_send![
        view_controller,
        setPreferredScreenEdgesDeferringSystemGestures: edges
    ];
    let _: () = msg_send![view_controller, setView: view];
    view_controller
}

// requires main thread
pub(crate) unsafe fn create_window(
    window_attributes: &WindowAttributes,
    _platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    frame: CGRect,
    view_controller: id,
) -> id {
    let window: id = msg_send![WinitUIWindow::class(), alloc];
    assert!(!window.is_null(), "Failed to create `UIWindow` instance");
    let window: id = msg_send![window, initWithFrame: frame];
    assert!(
        !window.is_null(),
        "Failed to initialize `UIWindow` instance"
    );
    let _: () = msg_send![window, setRootViewController: view_controller];
    match window_attributes.fullscreen {
        Some(Fullscreen::Exclusive(ref video_mode)) => {
            let uiscreen = video_mode.monitor().ui_screen() as id;
            let _: () = msg_send![uiscreen, setCurrentMode: video_mode.screen_mode.0];
            msg_send![window, setScreen:video_mode.monitor().ui_screen()]
        }
        Some(Fullscreen::Borderless(ref monitor)) => {
            let uiscreen: id = match &monitor {
                Some(monitor) => monitor.ui_screen() as id,
                None => {
                    let uiscreen: id = msg_send![window, screen];
                    uiscreen
                }
            };

            msg_send![window, setScreen: uiscreen]
        }
        None => (),
    }

    window
}

impl WinitUIWindow {
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
