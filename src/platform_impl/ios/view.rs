use std::collections::HashMap;

use objc2::declare::ClassBuilder;
use objc2::foundation::NSObject;
use objc2::runtime::{Bool, Class, Object, Sel, NO, YES};
use objc2::{class, declare_class, msg_send, sel, ClassType};

use super::uikit::{UIResponder, UIViewController, UIWindow};
use crate::{
    dpi::PhysicalPosition,
    event::{DeviceId as RootDeviceId, Event, Force, Touch, TouchPhase, WindowEvent},
    platform_impl::platform::{
        app_state,
        event_loop::{self, EventProxy, EventWrapper},
        ffi::{
            id, nil, CGFloat, CGPoint, CGRect, UIForceTouchCapability, UIInterfaceOrientationMask,
            UIRectEdge, UITouchPhase, UITouchType,
        },
        window::PlatformSpecificWindowBuilderAttributes,
        DeviceId, Fullscreen,
    },
    window::{WindowAttributes, WindowId as RootWindowId},
};

// requires main thread
unsafe fn get_view_class(root_view_class: &'static Class) -> &'static Class {
    static mut CLASSES: Option<HashMap<*const Class, &'static Class>> = None;
    static mut ID: usize = 0;

    if CLASSES.is_none() {
        CLASSES = Some(HashMap::default());
    }

    let classes = CLASSES.as_mut().unwrap();

    classes.entry(root_view_class).or_insert_with(move || {
        let uiview_class = class!(UIView);
        let is_uiview: bool = msg_send![root_view_class, isSubclassOfClass: uiview_class];
        assert!(is_uiview, "`root_view_class` must inherit from `UIView`");

        extern "C" fn draw_rect(object: &Object, _: Sel, rect: CGRect) {
            unsafe {
                let window: id = msg_send![object, window];
                assert!(!window.is_null());
                app_state::handle_nonuser_events(
                    std::iter::once(EventWrapper::StaticEvent(Event::RedrawRequested(
                        RootWindowId(window.into()),
                    )))
                    .chain(std::iter::once(EventWrapper::StaticEvent(
                        Event::RedrawEventsCleared,
                    ))),
                );
                let superclass: &'static Class = msg_send![object, superclass];
                let _: () = msg_send![super(object, superclass), drawRect: rect];
            }
        }

        extern "C" fn layout_subviews(object: &Object, _: Sel) {
            unsafe {
                let superclass: &'static Class = msg_send![object, superclass];
                let _: () = msg_send![super(object, superclass), layoutSubviews];

                let window: id = msg_send![object, window];
                assert!(!window.is_null());
                let window_bounds: CGRect = msg_send![window, bounds];
                let screen: id = msg_send![window, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect = msg_send![
                    object,
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
                let view_frame: CGRect = msg_send![object, frame];
                if view_frame != window_bounds {
                    let _: () = msg_send![object, setFrame: window_bounds];
                }

                app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.into()),
                    event: WindowEvent::Resized(size),
                }));
            }
        }

        extern "C" fn set_content_scale_factor(
            object: &mut Object,
            _: Sel,
            untrusted_scale_factor: CGFloat,
        ) {
            unsafe {
                let superclass: &'static Class = msg_send![&*object, superclass];
                let _: () = msg_send![
                    super(&mut *object, superclass),
                    setContentScaleFactor: untrusted_scale_factor
                ];
                let object = &*object; // Immutable for rest of method

                let window: id = msg_send![object, window];
                // `window` is null when `setContentScaleFactor` is invoked prior to `[UIWindow
                // makeKeyAndVisible]` at window creation time (either manually or internally by
                // UIKit when the `UIView` is first created), in which case we send no events here
                if window.is_null() {
                    return;
                }
                // `setContentScaleFactor` may be called with a value of 0, which means "reset the
                // content scale factor to a device-specific default value", so we can't use the
                // parameter here. We can query the actual factor using the getter
                let scale_factor: CGFloat = msg_send![object, contentScaleFactor];
                assert!(
                    !scale_factor.is_nan()
                        && scale_factor.is_finite()
                        && scale_factor.is_sign_positive()
                        && scale_factor > 0.0,
                    "invalid scale_factor set on UIView",
                );
                let scale_factor = scale_factor as f64;
                let bounds: CGRect = msg_send![object, bounds];
                let screen: id = msg_send![window, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect =
                    msg_send![object, convertRect: bounds, toCoordinateSpace: screen_space];
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

        extern "C" fn handle_touches(object: &Object, _: Sel, touches: id, _: id) {
            unsafe {
                let window: id = msg_send![object, window];
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
                        let trait_collection: id = msg_send![object, traitCollection];
                        let touch_capability: UIForceTouchCapability =
                            msg_send![trait_collection, forceTouchCapability];
                        // Both the OS _and_ the device need to be checked for force touch support.
                        if touch_capability == UIForceTouchCapability::Available {
                            let force: CGFloat = msg_send![touch, force];
                            let max_possible_force: CGFloat =
                                msg_send![touch, maximumPossibleForce];
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
                        let scale_factor: CGFloat = msg_send![uiscreen, nativeScale];
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

        let mut decl = ClassBuilder::new(&format!("WinitUIView{}", ID), root_view_class)
            .expect("Failed to declare class `WinitUIView`");
        ID += 1;
        decl.add_method(sel!(drawRect:), draw_rect as extern "C" fn(_, _, _));
        decl.add_method(sel!(layoutSubviews), layout_subviews as extern "C" fn(_, _));
        decl.add_method(
            sel!(setContentScaleFactor:),
            set_content_scale_factor as extern "C" fn(_, _, _),
        );

        decl.add_method(
            sel!(touchesBegan:withEvent:),
            handle_touches as extern "C" fn(_, _, _, _),
        );
        decl.add_method(
            sel!(touchesMoved:withEvent:),
            handle_touches as extern "C" fn(_, _, _, _),
        );
        decl.add_method(
            sel!(touchesEnded:withEvent:),
            handle_touches as extern "C" fn(_, _, _, _),
        );
        decl.add_method(
            sel!(touchesCancelled:withEvent:),
            handle_touches as extern "C" fn(_, _, _, _),
        );

        decl.register()
    })
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
    window_attributes: &WindowAttributes,
    platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    frame: CGRect,
) -> id {
    let class = get_view_class(platform_attributes.root_view_class);

    let view: id = msg_send![class, alloc];
    assert!(!view.is_null(), "Failed to create `UIView` instance");
    let view: id = msg_send![view, initWithFrame: frame];
    assert!(!view.is_null(), "Failed to initialize `UIView` instance");
    if window_attributes.multitouch_enabled {
        let _: () = msg_send![view, setMultipleTouchEnabled: YES];
    } else {
        let _: () = msg_send![view, setMultipleTouchEnabled: NO];
    }
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
    let status_bar_hidden = Bool::new(platform_attributes.prefers_status_bar_hidden);
    let idiom = event_loop::get_idiom();
    let supported_orientations = UIInterfaceOrientationMask::from_valid_orientations_idiom(
        platform_attributes.valid_orientations,
        idiom,
    );
    let prefers_home_indicator_hidden =
        Bool::new(platform_attributes.prefers_home_indicator_hidden);
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

declare_class!(
    pub struct WinitApplicationDelegate {}

    unsafe impl ClassType for WinitApplicationDelegate {
        type Super = NSObject;
    }

    // UIApplicationDelegate protocol
    unsafe impl WinitApplicationDelegate {
        #[sel(application:didFinishLaunchingWithOptions:)]
        fn did_finish_launching(&self, _: id, _: id) -> bool {
            unsafe {
                app_state::did_finish_launching();
            }
            true
        }

        #[sel(applicationDidBecomeActive:)]
        fn did_become_active(&self, _: id) {
            unsafe { app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::Resumed)) }
        }

        #[sel(applicationWillResignActive:)]
        fn will_resign_active(&self, _: id) {
            unsafe { app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::Suspended)) }
        }

        #[sel(applicationWillEnterForeground:)]
        fn will_enter_foreground(&self, _: id) {}
        #[sel(applicationDidEnterBackground:)]
        fn did_enter_background(&self, _: id) {}

        #[sel(applicationWillTerminate:)]
        fn will_terminate(&self, _: id) {
            unsafe {
                let app: id = msg_send![class!(UIApplication), sharedApplication];
                let windows: id = msg_send![app, windows];
                let windows_enum: id = msg_send![windows, objectEnumerator];
                let mut events = Vec::new();
                loop {
                    let window: id = msg_send![windows_enum, nextObject];
                    if window == nil {
                        break;
                    }
                    let is_winit_window = msg_send![window, isKindOfClass: WinitUIWindow::class()];
                    if is_winit_window {
                        events.push(EventWrapper::StaticEvent(Event::WindowEvent {
                            window_id: RootWindowId(window.into()),
                            event: WindowEvent::Destroyed,
                        }));
                    }
                }
                app_state::handle_nonuser_events(events);
                app_state::terminated();
            }
        }
    }

    #[sel(application:openURL:options:)]
    fn open_url(_: &mut Object, _: Sel, _application: id, url: id, _options: id) -> BOOL {
        let url = unsafe {
            let absolute_string: id = msg_send![url, absoluteString];
            let ptr: *const std::os::raw::c_char = msg_send!(absolute_string, UTF8String);
            std::ffi::CStr::from_ptr(ptr)
                .to_str()
                .unwrap_or_default()
                .to_string()
        };
        unsafe {
            app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::OpenURL { url }))
        }
        YES
    }

    #[sel(application:continueUserActivity:restorationHandler:)]
    fn continue_user_activity(
        _: &mut Object,
        _: Sel,
        _application: id,
        user_activity: id,
        restoration_handler: id,
    ) -> BOOL {
        unsafe {
            app_state::handle_nonuser_event(EventWrapper::StaticEvent(
                Event::ContinueUserActivity {
                    user_activity,
                    restoration_handler,
                },
            ))
        }
        YES
    }

    #[sel(applicationDidReceiveMemoryWarning:)]
    fn did_receive_memory_warning(_: &Object, _: Sel, _: id) {
        unsafe { app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::MemoryWarning)) }
    }
);
