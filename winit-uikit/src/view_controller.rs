use std::cell::Cell;

use objc2::rc::Retained;
use objc2::{available, define_class, msg_send, DefinedClass, MainThreadMarker};
use objc2_foundation::NSObject;
use objc2_ui_kit::{
    UIDevice, UIInterfaceOrientationMask, UIRectEdge, UIResponder, UIStatusBarStyle,
    UIUserInterfaceIdiom, UIView, UIViewController,
};

use crate::{ScreenEdge, StatusBarStyle, ValidOrientations, WindowAttributesIos};

pub struct ViewControllerState {
    prefers_status_bar_hidden: Cell<bool>,
    preferred_status_bar_style: Cell<UIStatusBarStyle>,
    prefers_home_indicator_auto_hidden: Cell<bool>,
    supported_orientations: Cell<UIInterfaceOrientationMask>,
    preferred_screen_edges_deferring_system_gestures: Cell<UIRectEdge>,
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
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };

        this.set_prefers_status_bar_hidden(ios_attributes.prefers_status_bar_hidden);

        this.set_preferred_status_bar_style(ios_attributes.preferred_status_bar_style);

        this.set_supported_interface_orientations(mtm, ios_attributes.valid_orientations);

        this.set_prefers_home_indicator_auto_hidden(ios_attributes.prefers_home_indicator_hidden);

        this.set_preferred_screen_edges_deferring_system_gestures(
            ios_attributes.preferred_screen_edges_deferring_system_gestures,
        );

        this.setView(Some(view));

        this
    }
}
