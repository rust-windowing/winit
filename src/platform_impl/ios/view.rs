use std::collections::HashMap;

use objc::{
    declare::ClassDecl,
    runtime::{Class, Object, Sel, BOOL, NO, YES},
};

use crate::{
    event::{DeviceId as RootDeviceId, Event, Force, Touch, TouchPhase, WindowEvent},
    platform::ios::MonitorHandleExtIOS,
    platform_impl::platform::{
        app_state::{self, OSCapabilities},
        event_loop::{self, EventProxy, EventWrapper},
        ffi::{
            id, nil, CGFloat, CGPoint, CGRect, UIForceTouchCapability, UIInterfaceOrientationMask,
            UIRectEdge, UITouchPhase, UITouchType,
        },
        window::PlatformSpecificWindowBuilderAttributes,
        DeviceId,
    },
    window::{Fullscreen, WindowAttributes, WindowId as RootWindowId},
};

macro_rules! add_property {
    (
        $decl:ident,
        $name:ident: $t:ty,
        $setter_name:ident: |$object:ident| $after_set:expr,
        $getter_name:ident,
    ) => {
        add_property!(
            $decl,
            $name: $t,
            $setter_name: true, |_, _|{}; |$object| $after_set,
            $getter_name,
        )
    };
    (
        $decl:ident,
        $name:ident: $t:ty,
        $setter_name:ident: $capability:expr, $err:expr; |$object:ident| $after_set:expr,
        $getter_name:ident,
    ) => {
        {
            const VAR_NAME: &'static str = concat!("_", stringify!($name));
            $decl.add_ivar::<$t>(VAR_NAME);
            let setter = if $capability {
                #[allow(non_snake_case)]
                extern "C" fn $setter_name($object: &mut Object, _: Sel, value: $t) {
                    unsafe {
                        $object.set_ivar::<$t>(VAR_NAME, value);
                    }
                    $after_set
                }
                $setter_name
            } else {
                #[allow(non_snake_case)]
                extern "C" fn $setter_name($object: &mut Object, _: Sel, value: $t) {
                    unsafe {
                        $object.set_ivar::<$t>(VAR_NAME, value);
                    }
                    $err(&app_state::os_capabilities(), "ignoring")
                }
                $setter_name
            };
            #[allow(non_snake_case)]
            extern "C" fn $getter_name($object: &Object, _: Sel) -> $t {
                unsafe { *$object.get_ivar::<$t>(VAR_NAME) }
            }
            $decl.add_method(
                sel!($setter_name:),
                setter as extern "C" fn(&mut Object, Sel, $t),
            );
            $decl.add_method(
                sel!($getter_name),
                $getter_name as extern "C" fn(&Object, Sel) -> $t,
            );
        }
    };
}

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
        let is_uiview: BOOL = msg_send![root_view_class, isSubclassOfClass: uiview_class];
        assert_eq!(
            is_uiview, YES,
            "`root_view_class` must inherit from `UIView`"
        );

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
                let () = msg_send![super(object, superclass), drawRect: rect];
            }
        }

        extern "C" fn layout_subviews(object: &Object, _: Sel) {
            unsafe {
                let superclass: &'static Class = msg_send![object, superclass];
                let () = msg_send![super(object, superclass), layoutSubviews];

                let window: id = msg_send![object, window];
                assert!(!window.is_null());
                let bounds: CGRect = msg_send![window, bounds];
                let screen: id = msg_send![window, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect =
                    msg_send![object, convertRect:bounds toCoordinateSpace:screen_space];
                let dpi_factor: CGFloat = msg_send![screen, scale];
                let size = crate::dpi::LogicalSize {
                    width: screen_frame.size.width as f64,
                    height: screen_frame.size.height as f64,
                }
                .to_physical(dpi_factor.into());
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
                let superclass: &'static Class = msg_send![object, superclass];
                let () = msg_send![
                    super(object, superclass),
                    setContentScaleFactor: untrusted_scale_factor
                ];

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
                let dpi_factor: CGFloat = msg_send![object, contentScaleFactor];
                assert!(
                    !dpi_factor.is_nan()
                        && dpi_factor.is_finite()
                        && dpi_factor.is_sign_positive()
                        && dpi_factor > 0.0,
                    "invalid scale_factor set on UIView",
                );
                let scale_factor: f64 = dpi_factor.into();
                let bounds: CGRect = msg_send![object, bounds];
                let screen: id = msg_send![window, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect =
                    msg_send![object, convertRect:bounds toCoordinateSpace:screen_space];
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
                    let location: CGPoint = msg_send![touch, locationInView: nil];
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

                    touch_events.push(EventWrapper::StaticEvent(Event::WindowEvent {
                        window_id: RootWindowId(window.into()),
                        event: WindowEvent::Touch(Touch {
                            device_id: RootDeviceId(DeviceId { uiscreen }),
                            id: touch_id,
                            location: (location.x as f64, location.y as f64).into(),
                            force,
                            phase,
                        }),
                    }));
                }
                app_state::handle_nonuser_events(touch_events);
            }
        }

        let mut decl = ClassDecl::new(&format!("WinitUIView{}", ID), root_view_class)
            .expect("Failed to declare class `WinitUIView`");
        ID += 1;
        decl.add_method(
            sel!(drawRect:),
            draw_rect as extern "C" fn(&Object, Sel, CGRect),
        );
        decl.add_method(
            sel!(layoutSubviews),
            layout_subviews as extern "C" fn(&Object, Sel),
        );
        decl.add_method(
            sel!(setContentScaleFactor:),
            set_content_scale_factor as extern "C" fn(&mut Object, Sel, CGFloat),
        );

        decl.add_method(
            sel!(touchesBegan:withEvent:),
            handle_touches as extern "C" fn(this: &Object, _: Sel, _: id, _: id),
        );
        decl.add_method(
            sel!(touchesMoved:withEvent:),
            handle_touches as extern "C" fn(this: &Object, _: Sel, _: id, _: id),
        );
        decl.add_method(
            sel!(touchesEnded:withEvent:),
            handle_touches as extern "C" fn(this: &Object, _: Sel, _: id, _: id),
        );
        decl.add_method(
            sel!(touchesCancelled:withEvent:),
            handle_touches as extern "C" fn(this: &Object, _: Sel, _: id, _: id),
        );

        decl.register()
    })
}

// requires main thread
unsafe fn get_view_controller_class() -> &'static Class {
    static mut CLASS: Option<&'static Class> = None;
    if CLASS.is_none() {
        let os_capabilities = app_state::os_capabilities();

        let uiviewcontroller_class = class!(UIViewController);

        extern "C" fn should_autorotate(_: &Object, _: Sel) -> BOOL {
            YES
        }

        let mut decl = ClassDecl::new("WinitUIViewController", uiviewcontroller_class)
            .expect("Failed to declare class `WinitUIViewController`");
        decl.add_method(
            sel!(shouldAutorotate),
            should_autorotate as extern "C" fn(&Object, Sel) -> BOOL,
        );
        add_property! {
            decl,
            prefers_status_bar_hidden: BOOL,
            setPrefersStatusBarHidden: |object| {
                unsafe {
                    let () = msg_send![object, setNeedsStatusBarAppearanceUpdate];
                }
            },
            prefersStatusBarHidden,
        }
        add_property! {
            decl,
            prefers_home_indicator_auto_hidden: BOOL,
            setPrefersHomeIndicatorAutoHidden:
                os_capabilities.home_indicator_hidden,
                OSCapabilities::home_indicator_hidden_err_msg;
                |object| {
                    unsafe {
                        let () = msg_send![object, setNeedsUpdateOfHomeIndicatorAutoHidden];
                    }
                },
            prefersHomeIndicatorAutoHidden,
        }
        add_property! {
            decl,
            supported_orientations: UIInterfaceOrientationMask,
            setSupportedInterfaceOrientations: |object| {
                unsafe {
                    let () = msg_send![class!(UIViewController), attemptRotationToDeviceOrientation];
                }
            },
            supportedInterfaceOrientations,
        }
        add_property! {
            decl,
            preferred_screen_edges_deferring_system_gestures: UIRectEdge,
            setPreferredScreenEdgesDeferringSystemGestures:
                os_capabilities.defer_system_gestures,
                OSCapabilities::defer_system_gestures_err_msg;
                |object| {
                    unsafe {
                        let () = msg_send![object, setNeedsUpdateOfScreenEdgesDeferringSystemGestures];
                    }
                },
            preferredScreenEdgesDeferringSystemGestures,
        }
        CLASS = Some(decl.register());
    }
    CLASS.unwrap()
}

// requires main thread
unsafe fn get_window_class() -> &'static Class {
    static mut CLASS: Option<&'static Class> = None;
    if CLASS.is_none() {
        let uiwindow_class = class!(UIWindow);

        extern "C" fn become_key_window(object: &Object, _: Sel) {
            unsafe {
                app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(object.into()),
                    event: WindowEvent::Focused(true),
                }));
                let () = msg_send![super(object, class!(UIWindow)), becomeKeyWindow];
            }
        }

        extern "C" fn resign_key_window(object: &Object, _: Sel) {
            unsafe {
                app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(object.into()),
                    event: WindowEvent::Focused(false),
                }));
                let () = msg_send![super(object, class!(UIWindow)), resignKeyWindow];
            }
        }

        let mut decl = ClassDecl::new("WinitUIWindow", uiwindow_class)
            .expect("Failed to declare class `WinitUIWindow`");
        decl.add_method(
            sel!(becomeKeyWindow),
            become_key_window as extern "C" fn(&Object, Sel),
        );
        decl.add_method(
            sel!(resignKeyWindow),
            resign_key_window as extern "C" fn(&Object, Sel),
        );

        CLASS = Some(decl.register());
    }
    CLASS.unwrap()
}

// requires main thread
pub unsafe fn create_view(
    _window_attributes: &WindowAttributes,
    platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    frame: CGRect,
) -> id {
    let class = get_view_class(platform_attributes.root_view_class);

    let view: id = msg_send![class, alloc];
    assert!(!view.is_null(), "Failed to create `UIView` instance");
    let view: id = msg_send![view, initWithFrame: frame];
    assert!(!view.is_null(), "Failed to initialize `UIView` instance");
    let () = msg_send![view, setMultipleTouchEnabled: YES];
    if let Some(scale_factor) = platform_attributes.scale_factor {
        let () = msg_send![view, setContentScaleFactor: scale_factor as CGFloat];
    }

    view
}

// requires main thread
pub unsafe fn create_view_controller(
    _window_attributes: &WindowAttributes,
    platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    view: id,
) -> id {
    let class = get_view_controller_class();

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
    let status_bar_hidden = if platform_attributes.prefers_status_bar_hidden {
        YES
    } else {
        NO
    };
    let idiom = event_loop::get_idiom();
    let supported_orientations = UIInterfaceOrientationMask::from_valid_orientations_idiom(
        platform_attributes.valid_orientations,
        idiom,
    );
    let prefers_home_indicator_hidden = if platform_attributes.prefers_home_indicator_hidden {
        YES
    } else {
        NO
    };
    let edges: UIRectEdge = platform_attributes
        .preferred_screen_edges_deferring_system_gestures
        .into();
    let () = msg_send![
        view_controller,
        setPrefersStatusBarHidden: status_bar_hidden
    ];
    let () = msg_send![
        view_controller,
        setSupportedInterfaceOrientations: supported_orientations
    ];
    let () = msg_send![
        view_controller,
        setPrefersHomeIndicatorAutoHidden: prefers_home_indicator_hidden
    ];
    let () = msg_send![
        view_controller,
        setPreferredScreenEdgesDeferringSystemGestures: edges
    ];
    let () = msg_send![view_controller, setView: view];
    view_controller
}

// requires main thread
pub unsafe fn create_window(
    window_attributes: &WindowAttributes,
    _platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    frame: CGRect,
    view_controller: id,
) -> id {
    let class = get_window_class();

    let window: id = msg_send![class, alloc];
    assert!(!window.is_null(), "Failed to create `UIWindow` instance");
    let window: id = msg_send![window, initWithFrame: frame];
    assert!(
        !window.is_null(),
        "Failed to initialize `UIWindow` instance"
    );
    let () = msg_send![window, setRootViewController: view_controller];
    match window_attributes.fullscreen {
        Some(Fullscreen::Exclusive(ref video_mode)) => {
            let uiscreen = video_mode.monitor().ui_screen() as id;
            let () = msg_send![uiscreen, setCurrentMode: video_mode.video_mode.screen_mode.0];
            msg_send![window, setScreen:video_mode.monitor().ui_screen()]
        }
        Some(Fullscreen::Borderless(ref monitor)) => {
            msg_send![window, setScreen:monitor.ui_screen()]
        }
        None => (),
    }

    window
}

pub fn create_delegate_class() {
    extern "C" fn did_finish_launching(_: &mut Object, _: Sel, _: id, _: id) -> BOOL {
        unsafe {
            app_state::did_finish_launching();
        }
        YES
    }

    extern "C" fn did_become_active(_: &Object, _: Sel, _: id) {
        unsafe { app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::Resumed)) }
    }

    extern "C" fn will_resign_active(_: &Object, _: Sel, _: id) {
        unsafe { app_state::handle_nonuser_event(EventWrapper::StaticEvent(Event::Suspended)) }
    }

    extern "C" fn will_enter_foreground(_: &Object, _: Sel, _: id) {}
    extern "C" fn did_enter_background(_: &Object, _: Sel, _: id) {}

    extern "C" fn will_terminate(_: &Object, _: Sel, _: id) {
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
                let is_winit_window: BOOL = msg_send![window, isKindOfClass: class!(WinitUIWindow)];
                if is_winit_window == YES {
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

    let ui_responder = class!(UIResponder);
    let mut decl =
        ClassDecl::new("AppDelegate", ui_responder).expect("Failed to declare class `AppDelegate`");

    unsafe {
        decl.add_method(
            sel!(application:didFinishLaunchingWithOptions:),
            did_finish_launching as extern "C" fn(&mut Object, Sel, id, id) -> BOOL,
        );

        decl.add_method(
            sel!(applicationDidBecomeActive:),
            did_become_active as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationWillResignActive:),
            will_resign_active as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationWillEnterForeground:),
            will_enter_foreground as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applicationDidEnterBackground:),
            did_enter_background as extern "C" fn(&Object, Sel, id),
        );

        decl.add_method(
            sel!(applicationWillTerminate:),
            will_terminate as extern "C" fn(&Object, Sel, id),
        );

        decl.register();
    }
}
