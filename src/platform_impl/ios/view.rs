#![allow(clippy::unnecessary_cast)]
use std::cell::Cell;
use std::ptr::NonNull;

use icrate::Foundation::{CGFloat, CGRect, MainThreadMarker, NSObject, NSObjectProtocol, NSSet};
use objc2::declare::{Ivar, IvarDrop};
use objc2::rc::Id;
use objc2::runtime::AnyClass;
use objc2::{declare_class, extern_methods, msg_send, msg_send_id, mutability, ClassType};

use super::app_state::{self, EventWrapper};
use super::uikit::{
    UIApplication, UIDevice, UIEvent, UIForceTouchCapability, UIInterfaceOrientationMask,
    UIResponder, UIStatusBarStyle, UITouch, UITouchPhase, UITouchType, UITraitCollection, UIView,
    UIViewController, UIWindow,
};
use super::window::WindowId;
use crate::{
    dpi::PhysicalPosition,
    event::{DeviceId as RootDeviceId, Event, Force, Touch, TouchPhase, WindowEvent},
    platform::ios::ValidOrientations,
    platform_impl::platform::{
        ffi::{UIRectEdge, UIUserInterfaceIdiom},
        window::PlatformSpecificWindowBuilderAttributes,
        DeviceId, Fullscreen,
    },
    window::{WindowAttributes, WindowId as RootWindowId},
};

declare_class!(
    pub(crate) struct WinitView;

    unsafe impl ClassType for WinitView {
        #[inherits(UIResponder, NSObject)]
        type Super = UIView;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitUIView";
    }

    unsafe impl WinitView {
        #[method(drawRect:)]
        fn draw_rect(&self, rect: CGRect) {
            let mtm = MainThreadMarker::new().unwrap();
            let window = self.window().unwrap();
            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::RedrawRequested,
                }),
            );
            let _: () = unsafe { msg_send![super(self), drawRect: rect] };
        }

        #[method(layoutSubviews)]
        fn layout_subviews(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            let _: () = unsafe { msg_send![super(self), layoutSubviews] };

            let window = self.window().unwrap();
            let window_bounds = window.bounds();
            let screen = window.screen();
            let screen_space = screen.coordinateSpace();
            let screen_frame = self.convertRect_toCoordinateSpace(window_bounds, &screen_space);
            let scale_factor = screen.scale();
            let size = crate::dpi::LogicalSize {
                width: screen_frame.size.width as f64,
                height: screen_frame.size.height as f64,
            }
            .to_physical(scale_factor as f64);

            // If the app is started in landscape, the view frame and window bounds can be mismatched.
            // The view frame will be in portrait and the window bounds in landscape. So apply the
            // window bounds to the view frame to make it consistent.
            let view_frame = self.frame();
            if view_frame != window_bounds {
                self.setFrame(window_bounds);
            }

            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(window.id()),
                    event: WindowEvent::Resized(size),
                }),
            );
        }

        #[method(setContentScaleFactor:)]
        fn set_content_scale_factor(&self, untrusted_scale_factor: CGFloat) {
            let mtm = MainThreadMarker::new().unwrap();
            let _: () =
                unsafe { msg_send![super(self), setContentScaleFactor: untrusted_scale_factor] };

            // `window` is null when `setContentScaleFactor` is invoked prior to `[UIWindow
            // makeKeyAndVisible]` at window creation time (either manually or internally by
            // UIKit when the `UIView` is first created), in which case we send no events here
            let window = match self.window() {
                Some(window) => window,
                None => return,
            };
            // `setContentScaleFactor` may be called with a value of 0, which means "reset the
            // content scale factor to a device-specific default value", so we can't use the
            // parameter here. We can query the actual factor using the getter
            let scale_factor = self.contentScaleFactor();
            assert!(
                !scale_factor.is_nan()
                    && scale_factor.is_finite()
                    && scale_factor.is_sign_positive()
                    && scale_factor > 0.0,
                "invalid scale_factor set on UIView",
            );
            let scale_factor = scale_factor as f64;
            let bounds = self.bounds();
            let screen = window.screen();
            let screen_space = screen.coordinateSpace();
            let screen_frame = self.convertRect_toCoordinateSpace(bounds, &screen_space);
            let size = crate::dpi::LogicalSize {
                width: screen_frame.size.width as f64,
                height: screen_frame.size.height as f64,
            };
            let window_id = RootWindowId(window.id());
            app_state::handle_nonuser_events(
                mtm,
                std::iter::once(EventWrapper::ScaleFactorChanged(
                    app_state::ScaleFactorChanged {
                        window,
                        scale_factor,
                        suggested_size: size.to_physical(scale_factor),
                    },
                ))
                .chain(std::iter::once(EventWrapper::StaticEvent(
                    Event::WindowEvent {
                        window_id,
                        event: WindowEvent::Resized(size.to_physical(scale_factor)),
                    },
                ))),
            );
        }

        #[method(touchesBegan:withEvent:)]
        fn touches_began(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesMoved:withEvent:)]
        fn touches_moved(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesEnded:withEvent:)]
        fn touches_ended(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }

        #[method(touchesCancelled:withEvent:)]
        fn touches_cancelled(&self, touches: &NSSet<UITouch>, _event: Option<&UIEvent>) {
            self.handle_touches(touches)
        }
    }
);

extern_methods!(
    #[allow(non_snake_case)]
    unsafe impl WinitView {
        fn window(&self) -> Option<Id<WinitUIWindow>> {
            unsafe { msg_send_id![self, window] }
        }

        unsafe fn traitCollection(&self) -> Id<UITraitCollection> {
            msg_send_id![self, traitCollection]
        }

        // TODO: Allow the user to customize this
        #[method(layerClass)]
        pub(crate) fn layerClass() -> &'static AnyClass;
    }
);

impl WinitView {
    pub(crate) fn new(
        _mtm: MainThreadMarker,
        _window_attributes: &WindowAttributes,
        platform_attributes: &PlatformSpecificWindowBuilderAttributes,
        frame: CGRect,
    ) -> Id<Self> {
        let this: Id<Self> = unsafe { msg_send_id![Self::alloc(), initWithFrame: frame] };

        this.setMultipleTouchEnabled(true);

        if let Some(scale_factor) = platform_attributes.scale_factor {
            this.setContentScaleFactor(scale_factor as _);
        }

        this
    }

    fn handle_touches(&self, touches: &NSSet<UITouch>) {
        let window = self.window().unwrap();
        let uiscreen = window.screen();
        let mut touch_events = Vec::new();
        let os_supports_force = app_state::os_capabilities().force_touch;
        for touch in touches {
            let logical_location = touch.locationInView(None);
            let touch_type = touch.type_();
            let force = if os_supports_force {
                let trait_collection = unsafe { self.traitCollection() };
                let touch_capability = trait_collection.forceTouchCapability();
                // Both the OS _and_ the device need to be checked for force touch support.
                if touch_capability == UIForceTouchCapability::Available
                    || touch_type == UITouchType::Pencil
                {
                    let force = touch.force();
                    let max_possible_force = touch.maximumPossibleForce();
                    let altitude_angle: Option<f64> = if touch_type == UITouchType::Pencil {
                        let angle = touch.altitudeAngle();
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
            let touch_id = touch as *const UITouch as u64;
            let phase = touch.phase();
            let phase = match phase {
                UITouchPhase::Began => TouchPhase::Started,
                UITouchPhase::Moved => TouchPhase::Moved,
                // 2 is UITouchPhase::Stationary and is not expected here
                UITouchPhase::Ended => TouchPhase::Ended,
                UITouchPhase::Cancelled => TouchPhase::Cancelled,
                _ => panic!("unexpected touch phase: {:?}", phase as i32),
            };

            let physical_location = {
                let scale_factor = self.contentScaleFactor();
                PhysicalPosition::from_logical::<(f64, f64), f64>(
                    (logical_location.x as _, logical_location.y as _),
                    scale_factor as f64,
                )
            };
            touch_events.push(EventWrapper::StaticEvent(Event::WindowEvent {
                window_id: RootWindowId(window.id()),
                event: WindowEvent::Touch(Touch {
                    device_id: RootDeviceId(DeviceId {
                        uiscreen: Id::as_ptr(&uiscreen),
                    }),
                    id: touch_id,
                    location: physical_location,
                    force,
                    phase,
                }),
            }));
        }
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(mtm, touch_events);
    }
}

pub struct ViewControllerState {
    prefers_status_bar_hidden: Cell<bool>,
    preferred_status_bar_style: Cell<UIStatusBarStyle>,
    prefers_home_indicator_auto_hidden: Cell<bool>,
    supported_orientations: Cell<UIInterfaceOrientationMask>,
    preferred_screen_edges_deferring_system_gestures: Cell<UIRectEdge>,
}

declare_class!(
    pub(crate) struct WinitViewController {
        state: IvarDrop<Box<ViewControllerState>, "_state">,
    }

    mod view_controller_ivars;

    unsafe impl ClassType for WinitViewController {
        #[inherits(UIResponder, NSObject)]
        type Super = UIViewController;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitUIViewController";
    }

    unsafe impl WinitViewController {
        #[method(init)]
        unsafe fn init(this: *mut Self) -> Option<NonNull<Self>> {
            let this: Option<&mut Self> = msg_send![super(this), init];
            this.map(|this| {
                // These are set in WinitViewController::new, it's just to set them
                // to _something_.
                Ivar::write(
                    &mut this.state,
                    Box::new(ViewControllerState {
                        prefers_status_bar_hidden: Cell::new(false),
                        preferred_status_bar_style: Cell::new(UIStatusBarStyle::Default),
                        prefers_home_indicator_auto_hidden: Cell::new(false),
                        supported_orientations: Cell::new(UIInterfaceOrientationMask::All),
                        preferred_screen_edges_deferring_system_gestures: Cell::new(
                            UIRectEdge::NONE,
                        ),
                    }),
                );
                NonNull::from(this)
            })
        }
    }

    unsafe impl WinitViewController {
        #[method(shouldAutorotate)]
        fn should_autorotate(&self) -> bool {
            true
        }

        #[method(prefersStatusBarHidden)]
        fn prefers_status_bar_hidden(&self) -> bool {
            self.state.prefers_status_bar_hidden.get()
        }

        #[method(preferredStatusBarStyle)]
        fn preferred_status_bar_style(&self) -> UIStatusBarStyle {
            self.state.preferred_status_bar_style.get()
        }

        #[method(prefersHomeIndicatorAutoHidden)]
        fn prefers_home_indicator_auto_hidden(&self) -> bool {
            self.state.prefers_home_indicator_auto_hidden.get()
        }

        #[method(supportedInterfaceOrientations)]
        fn supported_orientations(&self) -> UIInterfaceOrientationMask {
            self.state.supported_orientations.get()
        }

        #[method(preferredScreenEdgesDeferringSystemGestures)]
        fn preferred_screen_edges_deferring_system_gestures(&self) -> UIRectEdge {
            self.state
                .preferred_screen_edges_deferring_system_gestures
                .get()
        }
    }
);

impl WinitViewController {
    pub(crate) fn set_prefers_status_bar_hidden(&self, val: bool) {
        self.state.prefers_status_bar_hidden.set(val);
        self.setNeedsStatusBarAppearanceUpdate();
    }

    pub(crate) fn set_preferred_status_bar_style(&self, val: UIStatusBarStyle) {
        self.state.preferred_status_bar_style.set(val);
        self.setNeedsStatusBarAppearanceUpdate();
    }

    pub(crate) fn set_prefers_home_indicator_auto_hidden(&self, val: bool) {
        self.state.prefers_home_indicator_auto_hidden.set(val);
        let os_capabilities = app_state::os_capabilities();
        if os_capabilities.home_indicator_hidden {
            self.setNeedsUpdateOfHomeIndicatorAutoHidden();
        } else {
            os_capabilities.home_indicator_hidden_err_msg("ignoring")
        }
    }

    pub(crate) fn set_preferred_screen_edges_deferring_system_gestures(&self, val: UIRectEdge) {
        self.state
            .preferred_screen_edges_deferring_system_gestures
            .set(val);
        let os_capabilities = app_state::os_capabilities();
        if os_capabilities.defer_system_gestures {
            self.setNeedsUpdateOfScreenEdgesDeferringSystemGestures();
        } else {
            os_capabilities.defer_system_gestures_err_msg("ignoring")
        }
    }

    pub(crate) fn set_supported_interface_orientations(
        &self,
        mtm: MainThreadMarker,
        valid_orientations: ValidOrientations,
    ) {
        let mask = match (
            valid_orientations,
            UIDevice::current(mtm).userInterfaceIdiom(),
        ) {
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
        self.state.supported_orientations.set(mask);
        UIViewController::attemptRotationToDeviceOrientation();
    }

    pub(crate) fn new(
        mtm: MainThreadMarker,
        _window_attributes: &WindowAttributes,
        platform_attributes: &PlatformSpecificWindowBuilderAttributes,
        view: &UIView,
    ) -> Id<Self> {
        let this: Id<Self> = unsafe { msg_send_id![Self::alloc(), init] };

        this.set_prefers_status_bar_hidden(platform_attributes.prefers_status_bar_hidden);

        this.set_preferred_status_bar_style(platform_attributes.preferred_status_bar_style.into());

        this.set_supported_interface_orientations(mtm, platform_attributes.valid_orientations);

        this.set_prefers_home_indicator_auto_hidden(
            platform_attributes.prefers_home_indicator_hidden,
        );

        this.set_preferred_screen_edges_deferring_system_gestures(
            platform_attributes
                .preferred_screen_edges_deferring_system_gestures
                .into(),
        );

        this.setView(Some(view));

        this
    }
}

declare_class!(
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct WinitUIWindow;

    unsafe impl ClassType for WinitUIWindow {
        #[inherits(UIResponder, NSObject)]
        type Super = UIWindow;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitUIWindow";
    }

    unsafe impl WinitUIWindow {
        #[method(becomeKeyWindow)]
        fn become_key_window(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(self.id()),
                    event: WindowEvent::Focused(true),
                }),
            );
            let _: () = unsafe { msg_send![super(self), becomeKeyWindow] };
        }

        #[method(resignKeyWindow)]
        fn resign_key_window(&self) {
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(
                mtm,
                EventWrapper::StaticEvent(Event::WindowEvent {
                    window_id: RootWindowId(self.id()),
                    event: WindowEvent::Focused(false),
                }),
            );
            let _: () = unsafe { msg_send![super(self), resignKeyWindow] };
        }
    }
);

impl WinitUIWindow {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        window_attributes: &WindowAttributes,
        _platform_attributes: &PlatformSpecificWindowBuilderAttributes,
        frame: CGRect,
        view_controller: &UIViewController,
    ) -> Id<Self> {
        let this: Id<Self> = unsafe { msg_send_id![Self::alloc(), initWithFrame: frame] };

        this.setRootViewController(Some(view_controller));

        match window_attributes.fullscreen.0.clone().map(Into::into) {
            Some(Fullscreen::Exclusive(ref video_mode)) => {
                let monitor = video_mode.monitor();
                let screen = monitor.ui_screen(mtm);
                screen.setCurrentMode(Some(video_mode.screen_mode(mtm)));
                this.setScreen(screen);
            }
            Some(Fullscreen::Borderless(Some(ref monitor))) => {
                let screen = monitor.ui_screen(mtm);
                this.setScreen(screen);
            }
            _ => (),
        }

        this
    }

    pub(crate) fn id(&self) -> WindowId {
        (self as *const Self as usize as u64).into()
    }
}

declare_class!(
    pub struct WinitApplicationDelegate;

    unsafe impl ClassType for WinitApplicationDelegate {
        type Super = NSObject;
        type Mutability = mutability::InteriorMutable;
        const NAME: &'static str = "WinitApplicationDelegate";
    }

    // UIApplicationDelegate protocol
    unsafe impl WinitApplicationDelegate {
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
            self.send_occluded_event_for_all_windows(application, false);
        }

        #[method(applicationDidEnterBackground:)]
        fn did_enter_background(&self, application: &UIApplication) {
            self.send_occluded_event_for_all_windows(application, true);
        }

        #[method(applicationWillTerminate:)]
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
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_events(mtm, events);
            app_state::terminated(mtm);
        }

        #[method(applicationDidReceiveMemoryWarning:)]
        fn did_receive_memory_warning(&self, _application: &UIApplication) {
            let mtm = MainThreadMarker::new().unwrap();
            app_state::handle_nonuser_event(mtm, EventWrapper::StaticEvent(Event::MemoryWarning))
        }
    }
);

impl WinitApplicationDelegate {
    fn send_occluded_event_for_all_windows(&self, application: &UIApplication, occluded: bool) {
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
                    event: WindowEvent::Occluded(occluded),
                }));
            }
        }
        let mtm = MainThreadMarker::new().unwrap();
        app_state::handle_nonuser_events(mtm, events);
    }
}
