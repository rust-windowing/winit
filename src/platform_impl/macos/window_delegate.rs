#![allow(clippy::unnecessary_cast)]
use std::cell::Cell;
use std::ptr;

use icrate::AppKit::{
    NSApplicationPresentationFullScreen, NSApplicationPresentationHideDock,
    NSApplicationPresentationHideMenuBar, NSApplicationPresentationOptions, NSDraggingDestination,
    NSFilenamesPboardType, NSPasteboard, NSWindowDelegate, NSWindowOcclusionStateVisible,
};
use icrate::Foundation::{
    MainThreadMarker, NSArray, NSObject, NSObjectProtocol, NSPoint, NSSize, NSString,
};
use objc2::rc::{autoreleasepool, Id};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{
    class, declare_class, msg_send, msg_send_id, mutability, sel, ClassType, DeclaredClass,
};

use super::app_delegate::ApplicationDelegate;
use super::monitor::flip_window_screen_coordinates;
use super::{
    window::{get_ns_theme, WinitWindow},
    Fullscreen,
};
use crate::{
    dpi::{LogicalPosition, LogicalSize},
    event::WindowEvent,
};

#[derive(Debug)]
pub(crate) struct State {
    window: Id<WinitWindow>,

    // This is set when WindowBuilder::with_fullscreen was set,
    // see comments of `window_did_fail_to_enter_fullscreen`
    initial_fullscreen: Cell<bool>,

    // During `windowDidResize`, we use this to only send Moved if the position changed.
    //
    // This is expressed in native screen coordinates.
    previous_position: Cell<Option<NSPoint>>,

    // Used to prevent redundant events.
    previous_scale_factor: Cell<f64>,
}

declare_class!(
    pub(crate) struct WinitWindowDelegate;

    unsafe impl ClassType for WinitWindowDelegate {
        type Super = NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "WinitWindowDelegate";
    }

    impl DeclaredClass for WinitWindowDelegate {
        type Ivars = State;
    }

    unsafe impl NSObjectProtocol for WinitWindowDelegate {}

    unsafe impl NSWindowDelegate for WinitWindowDelegate {
        #[method(windowShouldClose:)]
        fn window_should_close(&self, _: Option<&AnyObject>) -> bool {
            trace_scope!("windowShouldClose:");
            self.queue_event(WindowEvent::CloseRequested);
            false
        }

        #[method(windowWillClose:)]
        fn window_will_close(&self, _: Option<&AnyObject>) {
            trace_scope!("windowWillClose:");
            // `setDelegate:` retains the previous value and then autoreleases it
            autoreleasepool(|_| {
                // Since El Capitan, we need to be careful that delegate methods can't
                // be called after the window closes.
                self.ivars().window.setDelegate(None);
            });
            self.queue_event(WindowEvent::Destroyed);
        }

        #[method(windowDidResize:)]
        fn window_did_resize(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidResize:");
            // NOTE: WindowEvent::Resized is reported in frameDidChange.
            self.emit_move_event();
        }

        #[method(windowWillStartLiveResize:)]
        fn window_will_start_live_resize(&self, _: Option<&AnyObject>) {
            trace_scope!("windowWillStartLiveResize:");

            let increments = self
                .ivars()
                .window
                .lock_shared_state("window_will_enter_fullscreen")
                .resize_increments;
            self.ivars().window.set_resize_increments_inner(increments);
        }

        #[method(windowDidEndLiveResize:)]
        fn window_did_end_live_resize(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidEndLiveResize:");
            self.ivars()
                .window
                .set_resize_increments_inner(NSSize::new(1., 1.));
        }

        // This won't be triggered if the move was part of a resize.
        #[method(windowDidMove:)]
        fn window_did_move(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidMove:");
            self.emit_move_event();
        }

        #[method(windowDidChangeBackingProperties:)]
        fn window_did_change_backing_properties(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidChangeBackingProperties:");
            self.queue_static_scale_factor_changed_event();
        }

        #[method(windowDidBecomeKey:)]
        fn window_did_become_key(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidBecomeKey:");
            // TODO: center the cursor if the window had mouse grab when it
            // lost focus
            self.queue_event(WindowEvent::Focused(true));
        }

        #[method(windowDidResignKey:)]
        fn window_did_resign_key(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidResignKey:");
            // It happens rather often, e.g. when the user is Cmd+Tabbing, that the
            // NSWindowDelegate will receive a didResignKey event despite no event
            // being received when the modifiers are released.  This is because
            // flagsChanged events are received by the NSView instead of the
            // NSWindowDelegate, and as a result a tracked modifiers state can quite
            // easily fall out of synchrony with reality.  This requires us to emit
            // a synthetic ModifiersChanged event when we lose focus.
            self.ivars().window.view().reset_modifiers();

            self.queue_event(WindowEvent::Focused(false));
        }

        /// Invoked when before enter fullscreen
        #[method(windowWillEnterFullScreen:)]
        fn window_will_enter_fullscreen(&self, _: Option<&AnyObject>) {
            trace_scope!("windowWillEnterFullScreen:");

            let mut shared_state = self
                .ivars()
                .window
                .lock_shared_state("window_will_enter_fullscreen");
            shared_state.maximized = self.ivars().window.is_zoomed();
            let fullscreen = shared_state.fullscreen.as_ref();
            match fullscreen {
                // Exclusive mode sets the state in `set_fullscreen` as the user
                // can't enter exclusive mode by other means (like the
                // fullscreen button on the window decorations)
                Some(Fullscreen::Exclusive(_)) => (),
                // `window_will_enter_fullscreen` was triggered and we're already
                // in fullscreen, so we must've reached here by `set_fullscreen`
                // as it updates the state
                Some(Fullscreen::Borderless(_)) => (),
                // Otherwise, we must've reached fullscreen by the user clicking
                // on the green fullscreen button. Update state!
                None => {
                    let current_monitor = self.ivars().window.current_monitor_inner();
                    shared_state.fullscreen = Some(Fullscreen::Borderless(current_monitor))
                }
            }
            shared_state.in_fullscreen_transition = true;
        }

        /// Invoked when before exit fullscreen
        #[method(windowWillExitFullScreen:)]
        fn window_will_exit_fullscreen(&self, _: Option<&AnyObject>) {
            trace_scope!("windowWillExitFullScreen:");

            let mut shared_state = self
                .ivars()
                .window
                .lock_shared_state("window_will_exit_fullscreen");
            shared_state.in_fullscreen_transition = true;
        }

        #[method(window:willUseFullScreenPresentationOptions:)]
        fn window_will_use_fullscreen_presentation_options(
            &self,
            _: Option<&AnyObject>,
            proposed_options: NSApplicationPresentationOptions,
        ) -> NSApplicationPresentationOptions {
            trace_scope!("window:willUseFullScreenPresentationOptions:");
            // Generally, games will want to disable the menu bar and the dock. Ideally,
            // this would be configurable by the user. Unfortunately because of our
            // `CGShieldingWindowLevel() + 1` hack (see `set_fullscreen`), our window is
            // placed on top of the menu bar in exclusive fullscreen mode. This looks
            // broken so we always disable the menu bar in exclusive fullscreen. We may
            // still want to make this configurable for borderless fullscreen. Right now
            // we don't, for consistency. If we do, it should be documented that the
            // user-provided options are ignored in exclusive fullscreen.
            let mut options = proposed_options;
            let shared_state = self
                .ivars()
                .window
                .lock_shared_state("window_will_use_fullscreen_presentation_options");
            if let Some(Fullscreen::Exclusive(_)) = shared_state.fullscreen {
                options = NSApplicationPresentationFullScreen
                    | NSApplicationPresentationHideDock
                    | NSApplicationPresentationHideMenuBar;
            }

            options
        }

        /// Invoked when entered fullscreen
        #[method(windowDidEnterFullScreen:)]
        fn window_did_enter_fullscreen(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidEnterFullScreen:");
            self.ivars().initial_fullscreen.set(false);
            let mut shared_state = self
                .ivars()
                .window
                .lock_shared_state("window_did_enter_fullscreen");
            shared_state.in_fullscreen_transition = false;
            let target_fullscreen = shared_state.target_fullscreen.take();
            drop(shared_state);
            if let Some(target_fullscreen) = target_fullscreen {
                self.ivars().window.set_fullscreen(target_fullscreen);
            }
        }

        /// Invoked when exited fullscreen
        #[method(windowDidExitFullScreen:)]
        fn window_did_exit_fullscreen(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidExitFullScreen:");

            self.ivars().window.restore_state_from_fullscreen();
            let mut shared_state = self
                .ivars()
                .window
                .lock_shared_state("window_did_exit_fullscreen");
            shared_state.in_fullscreen_transition = false;
            let target_fullscreen = shared_state.target_fullscreen.take();
            drop(shared_state);
            if let Some(target_fullscreen) = target_fullscreen {
                self.ivars().window.set_fullscreen(target_fullscreen);
            }
        }

        /// Invoked when fail to enter fullscreen
        ///
        /// When this window launch from a fullscreen app (e.g. launch from VS Code
        /// terminal), it creates a new virtual destkop and a transition animation.
        /// This animation takes one second and cannot be disable without
        /// elevated privileges. In this animation time, all toggleFullscreen events
        /// will be failed. In this implementation, we will try again by using
        /// performSelector:withObject:afterDelay: until window_did_enter_fullscreen.
        /// It should be fine as we only do this at initialzation (i.e with_fullscreen
        /// was set).
        ///
        /// From Apple doc:
        /// In some cases, the transition to enter full-screen mode can fail,
        /// due to being in the midst of handling some other animation or user gesture.
        /// This method indicates that there was an error, and you should clean up any
        /// work you may have done to prepare to enter full-screen mode.
        #[method(windowDidFailToEnterFullScreen:)]
        fn window_did_fail_to_enter_fullscreen(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidFailToEnterFullScreen:");
            let mut shared_state = self
                .ivars()
                .window
                .lock_shared_state("window_did_fail_to_enter_fullscreen");
            shared_state.in_fullscreen_transition = false;
            shared_state.target_fullscreen = None;
            if self.ivars().initial_fullscreen.get() {
                #[allow(clippy::let_unit_value)]
                unsafe {
                    let _: () = msg_send![
                        &*self.ivars().window,
                        performSelector: sel!(toggleFullScreen:),
                        withObject: ptr::null::<AnyObject>(),
                        afterDelay: 0.5,
                    ];
                };
            } else {
                self.ivars().window.restore_state_from_fullscreen();
            }
        }

        // Invoked when the occlusion state of the window changes
        #[method(windowDidChangeOcclusionState:)]
        fn window_did_change_occlusion_state(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidChangeOcclusionState:");
            let visible = self.ivars().window.occlusionState() & NSWindowOcclusionStateVisible
                == NSWindowOcclusionStateVisible;
            self.queue_event(WindowEvent::Occluded(!visible));
        }

        #[method(windowDidChangeScreen:)]
        fn window_did_change_screen(&self, _: Option<&AnyObject>) {
            trace_scope!("windowDidChangeScreen:");
            let is_simple_fullscreen = self
                .ivars()
                .window
                .lock_shared_state("window_did_change_screen")
                .is_simple_fullscreen;
            if is_simple_fullscreen {
                if let Some(screen) = self.ivars().window.screen() {
                    self.ivars().window.setFrame_display(screen.frame(), true);
                }
            }
        }
    }

    unsafe impl NSDraggingDestination for WinitWindowDelegate {
        /// Invoked when the dragged image enters destination bounds or frame
        #[method(draggingEntered:)]
        fn dragging_entered(&self, sender: &NSObject) -> bool {
            trace_scope!("draggingEntered:");

            use std::path::PathBuf;

            let pb: Id<NSPasteboard> = unsafe { msg_send_id![sender, draggingPasteboard] };
            let filenames = pb
                .propertyListForType(unsafe { NSFilenamesPboardType })
                .unwrap();
            let filenames: Id<NSArray<NSString>> = unsafe { Id::cast(filenames) };

            filenames.into_iter().for_each(|file| {
                let path = PathBuf::from(file.to_string());
                self.queue_event(WindowEvent::HoveredFile(path));
            });

            true
        }

        /// Invoked when the image is released
        #[method(prepareForDragOperation:)]
        fn prepare_for_drag_operation(&self, _sender: &NSObject) -> bool {
            trace_scope!("prepareForDragOperation:");
            true
        }

        /// Invoked after the released image has been removed from the screen
        #[method(performDragOperation:)]
        fn perform_drag_operation(&self, sender: &NSObject) -> bool {
            trace_scope!("performDragOperation:");

            use std::path::PathBuf;

            let pb: Id<NSPasteboard> = unsafe { msg_send_id![sender, draggingPasteboard] };
            let filenames = pb
                .propertyListForType(unsafe { NSFilenamesPboardType })
                .unwrap();
            let filenames: Id<NSArray<NSString>> = unsafe { Id::cast(filenames) };

            filenames.into_iter().for_each(|file| {
                let path = PathBuf::from(file.to_string());
                self.queue_event(WindowEvent::DroppedFile(path));
            });

            true
        }

        /// Invoked when the dragging operation is complete
        #[method(concludeDragOperation:)]
        fn conclude_drag_operation(&self, _sender: Option<&NSObject>) {
            trace_scope!("concludeDragOperation:");
        }

        /// Invoked when the dragging operation is cancelled
        #[method(draggingExited:)]
        fn dragging_exited(&self, _sender: Option<&NSObject>) {
            trace_scope!("draggingExited:");
            self.queue_event(WindowEvent::HoveredFileCancelled);
        }
    }

    unsafe impl WinitWindowDelegate {
        // Observe theme change
        #[method(effectiveAppearanceDidChange:)]
        fn effective_appearance_did_change(&self, sender: Option<&AnyObject>) {
            trace_scope!("Triggered `effectiveAppearanceDidChange:`");
            unsafe {
                msg_send![
                    self,
                    performSelectorOnMainThread: sel!(effectiveAppearanceDidChangedOnMainThread:),
                    withObject: sender,
                    waitUntilDone: false,
                ]
            }
        }

        #[method(effectiveAppearanceDidChangedOnMainThread:)]
        fn effective_appearance_did_changed_on_main_thread(&self, _: Option<&AnyObject>) {
            let mtm = MainThreadMarker::from(self);
            let theme = get_ns_theme(mtm);
            let mut shared_state = self
                .ivars()
                .window
                .lock_shared_state("effective_appearance_did_change");
            let current_theme = shared_state.current_theme;
            shared_state.current_theme = Some(theme);
            drop(shared_state);
            if current_theme != Some(theme) {
                self.queue_event(WindowEvent::ThemeChanged(theme));
            }
        }
    }
);

impl WinitWindowDelegate {
    pub fn new(window: &WinitWindow, initial_fullscreen: bool) -> Id<Self> {
        let mtm = MainThreadMarker::from(window);
        let scale_factor = window.scale_factor();
        let this = mtm.alloc().set_ivars(State {
            window: window.retain(),
            initial_fullscreen: Cell::new(initial_fullscreen),
            previous_position: Cell::new(None),
            previous_scale_factor: Cell::new(scale_factor),
        });
        let this: Id<Self> = unsafe { msg_send_id![super(this), init] };

        if scale_factor != 1.0 {
            this.queue_static_scale_factor_changed_event();
        }
        window.setDelegate(Some(ProtocolObject::from_ref(&*this)));

        // Enable theme change event
        let notification_center: Id<AnyObject> =
            unsafe { msg_send_id![class!(NSDistributedNotificationCenter), defaultCenter] };
        let notification_name = NSString::from_str("AppleInterfaceThemeChangedNotification");
        let _: () = unsafe {
            msg_send![
                &notification_center,
                addObserver: &*this
                selector: sel!(effectiveAppearanceDidChange:)
                name: &*notification_name
                object: ptr::null::<AnyObject>()
            ]
        };

        this
    }

    pub(crate) fn queue_event(&self, event: WindowEvent) {
        let app_delegate = ApplicationDelegate::get(MainThreadMarker::from(self));
        app_delegate.queue_window_event(self.ivars().window.id(), event);
    }

    fn queue_static_scale_factor_changed_event(&self) {
        let window = &self.ivars().window;
        let scale_factor = window.scale_factor();
        if scale_factor == self.ivars().previous_scale_factor.get() {
            return;
        };

        self.ivars().previous_scale_factor.set(scale_factor);
        let content_size = window.contentRectForFrameRect(window.frame()).size;
        let content_size = LogicalSize::new(content_size.width, content_size.height);

        let app_delegate = ApplicationDelegate::get(MainThreadMarker::from(self));
        app_delegate.queue_static_scale_factor_changed_event(
            window.clone(),
            content_size.to_physical(scale_factor),
            scale_factor,
        );
    }

    fn emit_move_event(&self) {
        let window = &self.ivars().window;
        let frame = window.frame();
        if self.ivars().previous_position.get() == Some(frame.origin) {
            return;
        }
        self.ivars().previous_position.set(Some(frame.origin));

        let position = flip_window_screen_coordinates(frame);
        let position =
            LogicalPosition::new(position.x, position.y).to_physical(window.scale_factor());
        self.queue_event(WindowEvent::Moved(position));
    }
}
