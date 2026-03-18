use std::cell::Cell;

use block2::RcBlock;
use objc2::rc::{Retained, Weak};
use objc2::runtime::ProtocolObject;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, Message, available, define_class, msg_send,
};
use objc2_core_foundation::{CFTimeInterval, CGRect};
use objc2_core_graphics::CGRectIntersection;
use objc2_foundation::{
    NSNotification, NSNotificationCenter, NSNumber, NSObject, NSObjectProtocol, NSValue,
};
use objc2_ui_kit::{
    UICoordinateSpace, UIDevice, UIEdgeInsets, UIInterfaceOrientationMask,
    UIKeyboardAnimationCurveUserInfoKey, UIKeyboardAnimationDurationUserInfoKey,
    UIKeyboardFrameBeginUserInfoKey, UIKeyboardFrameEndUserInfoKey,
    UIKeyboardWillChangeFrameNotification, UIRectEdge, UIResponder, UIScreen, UIStatusBarStyle,
    UIUserInterfaceIdiom, UIView, UIViewAnimating, UIViewAnimationCurve, UIViewAnimationOptions,
    UIViewController, UIViewPropertyAnimator,
};

use crate::notification_center::create_observer;
use crate::{ScreenEdge, StatusBarStyle, ValidOrientations, WindowAttributesIos};

pub struct ViewControllerState {
    prefers_status_bar_hidden: Cell<bool>,
    preferred_status_bar_style: Cell<UIStatusBarStyle>,
    prefers_home_indicator_auto_hidden: Cell<bool>,
    supported_orientations: Cell<UIInterfaceOrientationMask>,
    preferred_screen_edges_deferring_system_gestures: Cell<UIRectEdge>,
    // Keep observer around (deallocating it stops notifications being posted).
    keyboard_will_change_frame_observer:
        Cell<Option<Retained<ProtocolObject<dyn NSObjectProtocol>>>>,
}

define_class!(
    #[unsafe(super(UIViewController, UIResponder, NSObject))]
    #[name = "WinitUIViewController"]
    #[ivars = ViewControllerState]
    pub(crate) struct WinitViewController;

    /// This documentation attribute makes rustfmt work for some reason?
    impl WinitViewController {
        #[unsafe(method(shouldAutorotate))]
        fn should_autorotate(&self) -> bool {
            true
        }

        #[unsafe(method(prefersStatusBarHidden))]
        fn prefers_status_bar_hidden(&self) -> bool {
            self.ivars().prefers_status_bar_hidden.get()
        }

        #[unsafe(method(preferredStatusBarStyle))]
        fn preferred_status_bar_style(&self) -> UIStatusBarStyle {
            self.ivars().preferred_status_bar_style.get()
        }

        #[unsafe(method(prefersHomeIndicatorAutoHidden))]
        fn prefers_home_indicator_auto_hidden(&self) -> bool {
            self.ivars().prefers_home_indicator_auto_hidden.get()
        }

        #[unsafe(method(supportedInterfaceOrientations))]
        fn supported_orientations(&self) -> UIInterfaceOrientationMask {
            self.ivars().supported_orientations.get()
        }

        #[unsafe(method(preferredScreenEdgesDeferringSystemGestures))]
        fn preferred_screen_edges_deferring_system_gestures(&self) -> UIRectEdge {
            self.ivars().preferred_screen_edges_deferring_system_gestures.get()
        }
    }
);

impl WinitViewController {
    pub(crate) fn set_prefers_status_bar_hidden(&self, val: bool) {
        self.ivars().prefers_status_bar_hidden.set(val);
        self.setNeedsStatusBarAppearanceUpdate();
    }

    pub(crate) fn set_preferred_status_bar_style(&self, val: StatusBarStyle) {
        let val = match val {
            StatusBarStyle::Default => UIStatusBarStyle::Default,
            StatusBarStyle::LightContent => UIStatusBarStyle::LightContent,
            StatusBarStyle::DarkContent => UIStatusBarStyle::DarkContent,
        };
        self.ivars().preferred_status_bar_style.set(val);
        self.setNeedsStatusBarAppearanceUpdate();
    }

    pub(crate) fn set_prefers_home_indicator_auto_hidden(&self, val: bool) {
        self.ivars().prefers_home_indicator_auto_hidden.set(val);
        if available!(ios = 11.0, visionos = 1.0) {
            self.setNeedsUpdateOfHomeIndicatorAutoHidden();
        } else {
            tracing::warn!(
                "`setNeedsUpdateOfHomeIndicatorAutoHidden` requires iOS 11.0+ or visionOS. \
                 Ignoring"
            );
        }
    }

    pub(crate) fn set_preferred_screen_edges_deferring_system_gestures(&self, val: ScreenEdge) {
        let val = {
            assert_eq!(val.bits() & !ScreenEdge::ALL.bits(), 0, "invalid `ScreenEdge`");
            UIRectEdge(val.bits().into())
        };
        self.ivars().preferred_screen_edges_deferring_system_gestures.set(val);
        if available!(ios = 11.0, visionos = 1.0) {
            self.setNeedsUpdateOfScreenEdgesDeferringSystemGestures();
        } else {
            tracing::warn!(
                "`setNeedsUpdateOfScreenEdgesDeferringSystemGestures` requires iOS 11.0+ or \
                 visionOS. Ignoring"
            );
        }
    }

    pub(crate) fn set_supported_interface_orientations(
        &self,
        mtm: MainThreadMarker,
        valid_orientations: ValidOrientations,
    ) {
        let mask = match (valid_orientations, UIDevice::currentDevice(mtm).userInterfaceIdiom()) {
            (ValidOrientations::LandscapeAndPortrait, UIUserInterfaceIdiom::Phone) => {
                UIInterfaceOrientationMask::AllButUpsideDown
            },
            (ValidOrientations::LandscapeAndPortrait, _) => UIInterfaceOrientationMask::All,
            (ValidOrientations::Landscape, _) => UIInterfaceOrientationMask::Landscape,
            (ValidOrientations::Portrait, UIUserInterfaceIdiom::Phone) => {
                UIInterfaceOrientationMask::Portrait
            },
            (ValidOrientations::Portrait, _) => {
                UIInterfaceOrientationMask::Portrait
                    | UIInterfaceOrientationMask::PortraitUpsideDown
            },
        };
        self.ivars().supported_orientations.set(mask);
        #[allow(deprecated)]
        UIViewController::attemptRotationToDeviceOrientation(mtm);
    }

    pub(crate) fn new(
        mtm: MainThreadMarker,
        ios_attributes: &WindowAttributesIos,
        view: &UIView,
    ) -> Retained<Self> {
        // These are set properly below, we just to set them to something in the meantime.
        let this = mtm.alloc().set_ivars(ViewControllerState {
            prefers_status_bar_hidden: Cell::new(false),
            preferred_status_bar_style: Cell::new(UIStatusBarStyle::Default),
            prefers_home_indicator_auto_hidden: Cell::new(false),
            supported_orientations: Cell::new(UIInterfaceOrientationMask::All),
            preferred_screen_edges_deferring_system_gestures: Cell::new(UIRectEdge::empty()),
            keyboard_will_change_frame_observer: Cell::new(None),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };

        this.set_prefers_status_bar_hidden(ios_attributes.prefers_status_bar_hidden);

        this.set_preferred_status_bar_style(ios_attributes.preferred_status_bar_style);

        this.set_supported_interface_orientations(mtm, ios_attributes.valid_orientations);

        this.set_prefers_home_indicator_auto_hidden(ios_attributes.prefers_home_indicator_hidden);

        this.set_preferred_screen_edges_deferring_system_gestures(
            ios_attributes.preferred_screen_edges_deferring_system_gestures,
        );

        let center = NSNotificationCenter::defaultCenter();

        this.setView(Some(view));

        // Set up an observer that will make the `safeAreaRect` of the view update based on the soft
        // keyboard's presence (in addition to everything else that the safe area depends on).
        let controller_weak = Weak::from_retained(&this);
        this.ivars().keyboard_will_change_frame_observer.set(Some(create_observer(
            &center,
            unsafe { UIKeyboardWillChangeFrameNotification },
            move |notification| {
                eprintln!("UIKeyboardWillChangeFrameNotification");
                if let Some(controller) = controller_weak.load() {
                    keyboard_will_change_frame(&controller, notification);
                }
            },
        )));

        this
    }

    /// The current keyboard frame, in the view's coordinate space.
    pub(crate) fn current_keyboard_frame(&self) -> CGRect {
        // TODO: Combine start_frame and end_frame with `animator.fractionComplete()` to produce
        // current frame

        // Convert keyboard frame to view coordinates.
        let keyboard_frame = self
            .view()
            .unwrap()
            .convertRect_fromCoordinateSpace(frame, &keyboard_screen.coordinateSpace());
        todo!()
    }
}

fn keyboard_will_change_frame(controller: &WinitViewController, notification: &NSNotification) {
    let mtm = controller.mtm();
    let controller = controller.retain();
    let view = controller.view().unwrap();

    // The notification's object is the screen the keyboard appears on (since iOS 16).
    let keyboard_screen = notification
        .object()
        .map(|s| s.downcast::<UIScreen>().unwrap())
        .unwrap_or_else(|| view.window().unwrap().screen());

    let user_info = notification.userInfo().unwrap();
    let begin_frame = user_info
        .objectForKey(unsafe { UIKeyboardFrameBeginUserInfoKey })
        .unwrap()
        .downcast::<NSValue>()
        .unwrap()
        .get_rect()
        .unwrap();
    let end_frame = user_info
        .objectForKey(unsafe { UIKeyboardFrameEndUserInfoKey })
        .unwrap()
        .downcast::<NSValue>()
        .unwrap()
        .get_rect()
        .unwrap();
    let duration: CFTimeInterval = user_info
        .objectForKey(unsafe { UIKeyboardAnimationDurationUserInfoKey })
        .unwrap()
        .downcast::<NSNumber>()
        .unwrap()
        .doubleValue();
    let curve_raw = user_info
        .objectForKey(unsafe { UIKeyboardAnimationCurveUserInfoKey })
        .unwrap()
        .downcast::<NSNumber>()
        .unwrap()
        .integerValue();
    let curve = UIViewAnimationCurve(curve_raw);

    // If OS version is high enough, set up a `UIViewPropertyAnimator` to track the position of the
    // keyboard.
    if available!(ios = 10.0, tvos = 10.0, visionos = 1.0) {
        let animator = UIViewPropertyAnimator::initWithDuration_curve_animations(
            UIViewPropertyAnimator::alloc(mtm),
            duration,
            curve,
            None,
        );

        animator.addCompletion(&RcBlock::new(move |_| {
            controller.setAdditionalSafeAreaInsets(todo!());
            // TODO: Might need to do further work to update the safe area when we
            // move the view?

            view.layoutIfNeeded();
            // Safe area changed -> request redraw.
            view.setNeedsDisplay();
        }));

        animator.startAnimation();
    } else {
        // Update immediately.
        todo!()
    }

    // Not sufficient, `setAdditionalSafeAreaInsets` only updates at the start, it doesn't change
    // `safeAreaInsets` continously during the keyboard open animation.
    //
    // UIView::animateWithDuration_delay_options_animations_completion(
    //     duration,
    //     0.0,
    //     options,
    //     None,
    //     mtm,
    // );
}
