use std::collections::HashMap;

use objc::declare::ClassDecl;
use objc::runtime::{BOOL, Class, NO, Object, Sel, YES};

use crate::event::{
    DeviceId as RootDeviceId,
    Event,
    Touch,
    TouchPhase,
    WindowEvent
};
use crate::platform::ios::MonitorHandleExtIOS;
use crate::window::{WindowAttributes, WindowId as RootWindowId};

use crate::platform_impl::platform::app_state::AppState;
use crate::platform_impl::platform::DeviceId;
use crate::platform_impl::platform::event_loop;
use crate::platform_impl::platform::ffi::{
    id,
    nil,
    CGFloat,
    CGPoint,
    CGRect,
    UIInterfaceOrientationMask,
    UITouchPhase,
};
use crate::platform_impl::platform::window::{PlatformSpecificWindowBuilderAttributes};

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
        let is_uiview: BOOL = msg_send![root_view_class, isSubclassOfClass:uiview_class];
        assert_eq!(is_uiview, YES, "`root_view_class` must inherit from `UIView`");

        extern fn draw_rect(object: &Object, _: Sel, rect: CGRect) {
            unsafe {
                let window: id = msg_send![object, window];
                AppState::handle_nonuser_event(Event::WindowEvent {
                    window_id: RootWindowId(window.into()),
                    event: WindowEvent::RedrawRequested,
                });
                let superclass: &'static Class = msg_send![object, superclass];
                let () = msg_send![super(object, superclass), drawRect: rect];
            }
        }

        extern fn layout_subviews(object: &Object, _: Sel) {
            unsafe {
                let window: id = msg_send![object, window];
                let bounds: CGRect = msg_send![window, bounds];
                let screen: id = msg_send![window, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect = msg_send![object, convertRect:bounds toCoordinateSpace:screen_space];
                let size = crate::dpi::LogicalSize {
                    width: screen_frame.size.width,
                    height: screen_frame.size.height,
                };
                AppState::handle_nonuser_event(Event::WindowEvent {
                    window_id: RootWindowId(window.into()),
                    event: WindowEvent::Resized(size),
                });
                let superclass: &'static Class = msg_send![object, superclass];
                let () = msg_send![super(object, superclass), layoutSubviews];
            }
        }

        let mut decl = ClassDecl::new(&format!("WinitUIView{}", ID), root_view_class)
            .expect("Failed to declare class `WinitUIView`");
        ID += 1;
        decl.add_method(sel!(drawRect:),
                        draw_rect as extern fn(&Object, Sel, CGRect));
        decl.add_method(sel!(layoutSubviews),
                        layout_subviews as extern fn(&Object, Sel));
        decl.register()
    })
}

// requires main thread
unsafe fn get_view_controller_class() -> &'static Class {
    static mut CLASS: Option<&'static Class> = None;
    if CLASS.is_none() {
        let uiviewcontroller_class = class!(UIViewController);

        extern fn set_prefers_status_bar_hidden(object: &mut Object, _: Sel, hidden: BOOL) {
            unsafe {
                object.set_ivar::<BOOL>("_prefers_status_bar_hidden", hidden);
                let () = msg_send![object, setNeedsStatusBarAppearanceUpdate];
            }
        }

        extern fn prefers_status_bar_hidden(object: &Object, _: Sel) -> BOOL {
            unsafe {
                *object.get_ivar::<BOOL>("_prefers_status_bar_hidden")
            }
        }

        extern fn set_supported_orientations(object: &mut Object, _: Sel, orientations: UIInterfaceOrientationMask) {
            unsafe {
                object.set_ivar::<UIInterfaceOrientationMask>("_supported_orientations", orientations);
                let () = msg_send![class!(UIViewController), attemptRotationToDeviceOrientation];
            }
        }

        extern fn supported_orientations(object: &Object, _: Sel) -> UIInterfaceOrientationMask {
            unsafe {
                *object.get_ivar::<UIInterfaceOrientationMask>("_supported_orientations")
            }
        }

        extern fn should_autorotate(_: &Object, _: Sel) -> BOOL {
            YES
        }

        let mut decl = ClassDecl::new("WinitUIViewController", uiviewcontroller_class)
            .expect("Failed to declare class `WinitUIViewController`");
        decl.add_ivar::<BOOL>("_prefers_status_bar_hidden");
        decl.add_ivar::<UIInterfaceOrientationMask>("_supported_orientations");
        decl.add_method(sel!(setPrefersStatusBarHidden:),
                        set_prefers_status_bar_hidden as extern fn(&mut Object, Sel, BOOL));
        decl.add_method(sel!(prefersStatusBarHidden),
                        prefers_status_bar_hidden as extern fn(&Object, Sel) -> BOOL);
        decl.add_method(sel!(setSupportedInterfaceOrientations:),
                        set_supported_orientations as extern fn(&mut Object, Sel, UIInterfaceOrientationMask));
        decl.add_method(sel!(supportedInterfaceOrientations),
                        supported_orientations as extern fn(&Object, Sel) -> UIInterfaceOrientationMask);
        decl.add_method(sel!(shouldAutorotate),
                        should_autorotate as extern fn(&Object, Sel) -> BOOL);
        CLASS = Some(decl.register());
    }
    CLASS.unwrap()
}

// requires main thread
unsafe fn get_window_class() -> &'static Class {
    static mut CLASS: Option<&'static Class> = None;
    if CLASS.is_none() {
        let uiwindow_class = class!(UIWindow);

        extern fn become_key_window(object: &Object, _: Sel) {
            unsafe {
                AppState::handle_nonuser_event(Event::WindowEvent {
                    window_id: RootWindowId(object.into()),
                    event: WindowEvent::Focused(true),
                });
                let () = msg_send![super(object, class!(UIWindow)), becomeKeyWindow];
            }
        }

        extern fn resign_key_window(object: &Object, _: Sel) {
            unsafe {
                AppState::handle_nonuser_event(Event::WindowEvent {
                    window_id: RootWindowId(object.into()),
                    event: WindowEvent::Focused(false),
                });
                let () = msg_send![super(object, class!(UIWindow)), resignKeyWindow];
            }
        }

        extern fn handle_touches(object: &Object, _: Sel, touches: id, _:id) {
            unsafe {
                let uiscreen = msg_send![object, screen];
                let touches_enum: id = msg_send![touches, objectEnumerator];
                let mut touch_events = Vec::new();
                loop {
                    let touch: id = msg_send![touches_enum, nextObject];
                    if touch == nil {
                        break
                    }
                    let location: CGPoint = msg_send![touch, locationInView:nil];
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

                    touch_events.push(Event::WindowEvent {
                        window_id: RootWindowId(object.into()),
                        event: WindowEvent::Touch(Touch {
                            device_id: RootDeviceId(DeviceId { uiscreen }),
                            id: touch_id,
                            location: (location.x as f64, location.y as f64).into(),
                            phase,
                        }),
                    });
                }
                AppState::handle_nonuser_events(touch_events);
            }
        }

        extern fn set_content_scale_factor(object: &mut Object, _: Sel, hidpi_factor: CGFloat) {
            unsafe {
                let () = msg_send![super(object, class!(UIWindow)), setContentScaleFactor:hidpi_factor];
                let view_controller: id = msg_send![object, rootViewController];
                let view: id = msg_send![view_controller, view];
                let () = msg_send![view, setContentScaleFactor:hidpi_factor];
                let bounds: CGRect = msg_send![object, bounds];
                let screen: id = msg_send![object, screen];
                let screen_space: id = msg_send![screen, coordinateSpace];
                let screen_frame: CGRect = msg_send![object, convertRect:bounds toCoordinateSpace:screen_space];
                let size = crate::dpi::LogicalSize {
                    width: screen_frame.size.width,
                    height: screen_frame.size.height,
                };
                AppState::handle_nonuser_events(
                    std::iter::once(Event::WindowEvent {
                        window_id: RootWindowId(object.into()),
                        event: WindowEvent::HiDpiFactorChanged(hidpi_factor as _),
                    }).chain(std::iter::once(Event::WindowEvent {
                        window_id: RootWindowId(object.into()),
                        event: WindowEvent::Resized(size),
                    }))
                );
            }
        }

        let mut decl = ClassDecl::new("WinitUIWindow", uiwindow_class)
            .expect("Failed to declare class `WinitUIWindow`");
        decl.add_method(sel!(becomeKeyWindow),
                        become_key_window as extern fn(&Object, Sel));
        decl.add_method(sel!(resignKeyWindow),
                        resign_key_window as extern fn(&Object, Sel));

        decl.add_method(sel!(touchesBegan:withEvent:),
                        handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));
        decl.add_method(sel!(touchesMoved:withEvent:),
                        handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));
        decl.add_method(sel!(touchesEnded:withEvent:),
                        handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));
        decl.add_method(sel!(touchesCancelled:withEvent:),
                        handle_touches as extern fn(this: &Object, _: Sel, _: id, _:id));

        decl.add_method(sel!(setContentScaleFactor:),
                        set_content_scale_factor as extern fn(&mut Object, Sel, CGFloat));

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
    let view: id = msg_send![view, initWithFrame:frame];
    assert!(!view.is_null(), "Failed to initialize `UIView` instance");
    let () = msg_send![view, setMultipleTouchEnabled:YES];

    view
}

// requires main thread
pub unsafe fn create_view_controller(
    window_attributes: &WindowAttributes,
    platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    view: id,
) -> id {
    let class = get_view_controller_class();

    let view_controller: id = msg_send![class, alloc];
    assert!(!view_controller.is_null(), "Failed to create `UIViewController` instance");
    let view_controller: id = msg_send![view_controller, init];
    assert!(!view_controller.is_null(), "Failed to initialize `UIViewController` instance");
    let status_bar_hidden = if window_attributes.decorations {
        NO
    } else {
        YES
    };
    let idiom = event_loop::get_idiom();
    let supported_orientations = UIInterfaceOrientationMask::from_valid_orientations_idiom(
        platform_attributes.valid_orientations,
        idiom,
    );
    let () = msg_send![view_controller, setPrefersStatusBarHidden:status_bar_hidden];
    let () = msg_send![view_controller, setSupportedInterfaceOrientations:supported_orientations];
    let () = msg_send![view_controller, setView:view];
    view_controller
}

// requires main thread
pub unsafe fn create_window(
    window_attributes: &WindowAttributes,
    platform_attributes: &PlatformSpecificWindowBuilderAttributes,
    frame: CGRect,
    view_controller: id,
) -> id {
    let class = get_window_class();

    let window: id = msg_send![class, alloc];
    assert!(!window.is_null(), "Failed to create `UIWindow` instance");
    let window: id = msg_send![window, initWithFrame:frame];
    assert!(!window.is_null(), "Failed to initialize `UIWindow` instance");
    let () = msg_send![window, setRootViewController:view_controller];
    if let Some(hidpi_factor) = platform_attributes.hidpi_factor {
        let () = msg_send![window, setContentScaleFactor:hidpi_factor as CGFloat];
    }
    if let &Some(ref monitor) = &window_attributes.fullscreen {
        let () = msg_send![window, setScreen:monitor.ui_screen()];
    }

    window
}

pub fn create_delegate_class() {
    extern fn did_finish_launching(_: &mut Object, _: Sel, _: id, _: id) -> BOOL {
        unsafe {
            AppState::did_finish_launching();
        }
        YES
    }

    extern fn did_become_active(_: &Object, _: Sel, _: id) {
        unsafe {
            AppState::handle_nonuser_event(Event::Suspended(false))
        }
    }

    extern fn will_resign_active(_: &Object, _: Sel, _: id) {
        unsafe {
            AppState::handle_nonuser_event(Event::Suspended(true))
        }
    }

    extern fn will_enter_foreground(_: &Object, _: Sel, _: id) {}
    extern fn did_enter_background(_: &Object, _: Sel, _: id) {}

    extern fn will_terminate(_: &Object, _: Sel, _: id) {
        unsafe {
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
            AppState::handle_nonuser_events(events);
            AppState::terminated();
        }
    }

    let ui_responder = class!(UIResponder);
    let mut decl = ClassDecl::new("AppDelegate", ui_responder).expect("Failed to declare class `AppDelegate`");

    unsafe {
        decl.add_method(sel!(application:didFinishLaunchingWithOptions:),
                        did_finish_launching as extern fn(&mut Object, Sel, id, id) -> BOOL);

        decl.add_method(sel!(applicationDidBecomeActive:),
                        did_become_active as extern fn(&Object, Sel, id));
        decl.add_method(sel!(applicationWillResignActive:),
                        will_resign_active as extern fn(&Object, Sel, id));
        decl.add_method(sel!(applicationWillEnterForeground:),
                        will_enter_foreground as extern fn(&Object, Sel, id));
        decl.add_method(sel!(applicationDidEnterBackground:),
                        did_enter_background as extern fn(&Object, Sel, id));

        decl.add_method(sel!(applicationWillTerminate:),
                        will_terminate as extern fn(&Object, Sel, id));

        decl.register();
    }
}
