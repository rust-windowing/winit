#![allow(clippy::unnecessary_cast)]

use std::ptr;

use objc2::declare::{Ivar, IvarDrop};
use objc2::foundation::{NSArray, NSObject, NSSize, NSString};
use objc2::rc::{autoreleasepool, Id, Shared};
use objc2::runtime::Object;
use objc2::{class, declare_class, msg_send, msg_send_id, sel, ClassType};

use super::appkit::{
    NSApplicationPresentationOptions, NSFilenamesPboardType, NSPasteboard, NSWindowOcclusionState,
};
use crate::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, ModifiersState, WindowEvent},
    platform_impl::platform::{
        app_state::AppState,
        event::{EventProxy, EventWrapper},
        util,
        window::{get_ns_theme, WinitWindow},
        Fullscreen,
    },
    window::WindowId,
};

declare_class!(
    #[derive(Debug)]
    pub(crate) struct WinitWindowDelegate {
        window: IvarDrop<Id<WinitWindow, Shared>>,

        // TODO: It's possible for delegate methods to be called asynchronously,
        // causing data races / `RefCell` panics.

        // This is set when WindowBuilder::with_fullscreen was set,
        // see comments of `window_did_fail_to_enter_fullscreen`
        initial_fullscreen: bool,

        // During `windowDidResize`, we use this to only send Moved if the position changed.
        // TODO: Remove unnecessary boxing here
        previous_position: IvarDrop<Option<Box<(f64, f64)>>>,

        // Used to prevent redundant events.
        previous_scale_factor: f64,
    }

    unsafe impl ClassType for WinitWindowDelegate {
        type Super = NSObject;
    }

    unsafe impl WinitWindowDelegate {
        #[sel(initWithWindow:initialFullscreen:)]
        fn init_with_winit(
            &mut self,
            window: &WinitWindow,
            initial_fullscreen: bool,
        ) -> Option<&mut Self> {
            let this: Option<&mut Self> = unsafe { msg_send![self, init] };
            this.map(|this| {
                let scale_factor = window.scale_factor();

                Ivar::write(&mut this.window, window.retain());
                Ivar::write(&mut this.initial_fullscreen, initial_fullscreen);
                Ivar::write(&mut this.previous_position, None);
                Ivar::write(&mut this.previous_scale_factor, scale_factor);

                if scale_factor != 1.0 {
                    this.queue_static_scale_factor_changed_event();
                }
                this.window.setDelegate(Some(this));

                // Enable theme change event
                let notification_center: Id<Object, Shared> =
                    unsafe { msg_send_id![class!(NSDistributedNotificationCenter), defaultCenter] };
                let notification_name =
                    NSString::from_str("AppleInterfaceThemeChangedNotification");
                let _: () = unsafe {
                    msg_send![
                        &notification_center,
                        addObserver: &*this
                        selector: sel!(effectiveAppearanceDidChange:)
                        name: &*notification_name
                        object: ptr::null::<Object>()
                    ]
                };

                this
            })
        }
    }

    // NSWindowDelegate + NSDraggingDestination protocols
    unsafe impl WinitWindowDelegate {
        #[sel(windowShouldClose:)]
        fn window_should_close(&self, _: Option<&Object>) -> bool {
            trace_scope!("windowShouldClose:");
            self.queue_event(WindowEvent::CloseRequested);
            false
        }

        #[sel(windowWillClose:)]
        fn window_will_close(&self, _: Option<&Object>) {
            trace_scope!("windowWillClose:");
            // `setDelegate:` retains the previous value and then autoreleases it
            autoreleasepool(|_| {
                // Since El Capitan, we need to be careful that delegate methods can't
                // be called after the window closes.
                self.window.setDelegate(None);
            });
            self.queue_event(WindowEvent::Destroyed);
        }

        #[sel(windowDidResize:)]
        fn window_did_resize(&mut self, _: Option<&Object>) {
            trace_scope!("windowDidResize:");
            // NOTE: WindowEvent::Resized is reported in frameDidChange.
            self.emit_move_event();
        }

        #[sel(windowWillStartLiveResize:)]
        fn window_will_start_live_resize(&mut self, _: Option<&Object>) {
            trace_scope!("windowWillStartLiveResize:");

            let increments = self
                .window
                .lock_shared_state("window_will_enter_fullscreen")
                .resize_increments;
            self.window.set_resize_increments_inner(increments);
        }

        #[sel(windowDidEndLiveResize:)]
        fn window_did_end_live_resize(&mut self, _: Option<&Object>) {
            trace_scope!("windowDidEndLiveResize:");
            self.window.set_resize_increments_inner(NSSize::new(1., 1.));
        }

        // This won't be triggered if the move was part of a resize.
        #[sel(windowDidMove:)]
        fn window_did_move(&mut self, _: Option<&Object>) {
            trace_scope!("windowDidMove:");
            self.emit_move_event();
        }

        #[sel(windowDidChangeBackingProperties:)]
        fn window_did_change_backing_properties(&mut self, _: Option<&Object>) {
            trace_scope!("windowDidChangeBackingProperties:");
            self.queue_static_scale_factor_changed_event();
        }

        #[sel(windowDidBecomeKey:)]
        fn window_did_become_key(&self, _: Option<&Object>) {
            trace_scope!("windowDidBecomeKey:");
            // TODO: center the cursor if the window had mouse grab when it
            // lost focus
            self.queue_event(WindowEvent::Focused(true));
        }

        #[sel(windowDidResignKey:)]
        fn window_did_resign_key(&self, _: Option<&Object>) {
            trace_scope!("windowDidResignKey:");
            // It happens rather often, e.g. when the user is Cmd+Tabbing, that the
            // NSWindowDelegate will receive a didResignKey event despite no event
            // being received when the modifiers are released.  This is because
            // flagsChanged events are received by the NSView instead of the
            // NSWindowDelegate, and as a result a tracked modifiers state can quite
            // easily fall out of synchrony with reality.  This requires us to emit
            // a synthetic ModifiersChanged event when we lose focus.

            // TODO(madsmtm): Remove the need for this unsafety
            let mut view = unsafe { Id::from_shared(self.window.view()) };

            // Both update the state and emit a ModifiersChanged event.
            if !view.state.modifiers.is_empty() {
                view.state.modifiers = ModifiersState::empty();
                self.queue_event(WindowEvent::ModifiersChanged(view.state.modifiers));
            }

            self.queue_event(WindowEvent::Focused(false));
        }

        /// Invoked when the dragged image enters destination bounds or frame
        #[sel(draggingEntered:)]
        fn dragging_entered(&self, sender: *mut Object) -> bool {
            trace_scope!("draggingEntered:");

            use std::path::PathBuf;

            let pb: Id<NSPasteboard, Shared> = unsafe { msg_send_id![sender, draggingPasteboard] };
            let filenames = pb.propertyListForType(unsafe { NSFilenamesPboardType });
            let filenames: Id<NSArray<NSString>, Shared> = unsafe { Id::cast(filenames) };

            filenames.into_iter().for_each(|file| {
                let path = PathBuf::from(file.to_string());
                self.queue_event(WindowEvent::HoveredFile(path));
            });

            true
        }

        /// Invoked when the image is released
        #[sel(prepareForDragOperation:)]
        fn prepare_for_drag_operation(&self, _: Option<&Object>) -> bool {
            trace_scope!("prepareForDragOperation:");
            true
        }

        /// Invoked after the released image has been removed from the screen
        #[sel(performDragOperation:)]
        fn perform_drag_operation(&self, sender: *mut Object) -> bool {
            trace_scope!("performDragOperation:");

            use std::path::PathBuf;

            let pb: Id<NSPasteboard, Shared> = unsafe { msg_send_id![sender, draggingPasteboard] };
            let filenames = pb.propertyListForType(unsafe { NSFilenamesPboardType });
            let filenames: Id<NSArray<NSString>, Shared> = unsafe { Id::cast(filenames) };

            filenames.into_iter().for_each(|file| {
                let path = PathBuf::from(file.to_string());
                self.queue_event(WindowEvent::DroppedFile(path));
            });

            true
        }

        /// Invoked when the dragging operation is complete
        #[sel(concludeDragOperation:)]
        fn conclude_drag_operation(&self, _: Option<&Object>) {
            trace_scope!("concludeDragOperation:");
        }

        /// Invoked when the dragging operation is cancelled
        #[sel(draggingExited:)]
        fn dragging_exited(&self, _: Option<&Object>) {
            trace_scope!("draggingExited:");
            self.queue_event(WindowEvent::HoveredFileCancelled);
        }

        /// Invoked when before enter fullscreen
        #[sel(windowWillEnterFullScreen:)]
        fn window_will_enter_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowWillEnterFullScreen:");

            let mut shared_state = self
                .window
                .lock_shared_state("window_will_enter_fullscreen");
            shared_state.maximized = self.window.is_zoomed();
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
                    let current_monitor = self.window.current_monitor_inner();
                    shared_state.fullscreen = Some(Fullscreen::Borderless(current_monitor))
                }
            }
            shared_state.in_fullscreen_transition = true;
        }

        /// Invoked when before exit fullscreen
        #[sel(windowWillExitFullScreen:)]
        fn window_will_exit_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowWillExitFullScreen:");

            let mut shared_state = self.window.lock_shared_state("window_will_exit_fullscreen");
            shared_state.in_fullscreen_transition = true;
        }

        #[sel(window:willUseFullScreenPresentationOptions:)]
        fn window_will_use_fullscreen_presentation_options(
            &self,
            _: Option<&Object>,
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
                .window
                .lock_shared_state("window_will_use_fullscreen_presentation_options");
            if let Some(Fullscreen::Exclusive(_)) = shared_state.fullscreen {
                options = NSApplicationPresentationOptions::NSApplicationPresentationFullScreen
                    | NSApplicationPresentationOptions::NSApplicationPresentationHideDock
                    | NSApplicationPresentationOptions::NSApplicationPresentationHideMenuBar;
            }

            options
        }

        /// Invoked when entered fullscreen
        #[sel(windowDidEnterFullScreen:)]
        fn window_did_enter_fullscreen(&mut self, _: Option<&Object>) {
            trace_scope!("windowDidEnterFullScreen:");
            *self.initial_fullscreen = false;
            let mut shared_state = self.window.lock_shared_state("window_did_enter_fullscreen");
            shared_state.in_fullscreen_transition = false;
            let target_fullscreen = shared_state.target_fullscreen.take();
            drop(shared_state);
            if let Some(target_fullscreen) = target_fullscreen {
                self.window.set_fullscreen(target_fullscreen);
            }
        }

        /// Invoked when exited fullscreen
        #[sel(windowDidExitFullScreen:)]
        fn window_did_exit_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowDidExitFullScreen:");

            self.window.restore_state_from_fullscreen();
            let mut shared_state = self.window.lock_shared_state("window_did_exit_fullscreen");
            shared_state.in_fullscreen_transition = false;
            let target_fullscreen = shared_state.target_fullscreen.take();
            drop(shared_state);
            if let Some(target_fullscreen) = target_fullscreen {
                self.window.set_fullscreen(target_fullscreen);
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
        #[sel(windowDidFailToEnterFullScreen:)]
        fn window_did_fail_to_enter_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowDidFailToEnterFullScreen:");
            let mut shared_state = self
                .window
                .lock_shared_state("window_did_fail_to_enter_fullscreen");
            shared_state.in_fullscreen_transition = false;
            shared_state.target_fullscreen = None;
            if *self.initial_fullscreen {
                #[allow(clippy::let_unit_value)]
                unsafe {
                    let _: () = msg_send![
                        &*self.window,
                        performSelector: sel!(toggleFullScreen:),
                        withObject: ptr::null::<Object>(),
                        afterDelay: 0.5,
                    ];
                };
            } else {
                self.window.restore_state_from_fullscreen();
            }
        }

        // Invoked when the occlusion state of the window changes
        #[sel(windowDidChangeOcclusionState:)]
        fn window_did_change_occlusion_state(&self, _: Option<&Object>) {
            trace_scope!("windowDidChangeOcclusionState:");
            self.queue_event(WindowEvent::Occluded(
                !self
                    .window
                    .occlusionState()
                    .contains(NSWindowOcclusionState::NSWindowOcclusionStateVisible),
            ))
        }

        // Observe theme change
        #[sel(effectiveAppearanceDidChange:)]
        fn effective_appearance_did_change(&self, sender: Option<&Object>) {
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

        #[sel(effectiveAppearanceDidChangedOnMainThread:)]
        fn effective_appearance_did_changed_on_main_thread(&self, _: Option<&Object>) {
            let theme = get_ns_theme();
            let mut shared_state = self
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
    pub fn new(window: &WinitWindow, initial_fullscreen: bool) -> Id<Self, Shared> {
        unsafe {
            msg_send_id![
                msg_send_id![Self::class(), alloc],
                initWithWindow: window,
                initialFullscreen: initial_fullscreen,
            ]
        }
    }

    pub(crate) fn queue_event(&self, event: WindowEvent<'static>) {
        let event = Event::WindowEvent {
            window_id: WindowId(self.window.id()),
            event,
        };
        AppState::queue_event(EventWrapper::StaticEvent(event));
    }

    fn queue_static_scale_factor_changed_event(&mut self) {
        let scale_factor = self.window.scale_factor();
        if scale_factor == *self.previous_scale_factor {
            return;
        };

        *self.previous_scale_factor = scale_factor;
        let wrapper = EventWrapper::EventProxy(EventProxy::DpiChangedProxy {
            window: self.window.clone(),
            suggested_size: self.view_size(),
            scale_factor,
        });
        AppState::queue_event(wrapper);
    }

    fn emit_move_event(&mut self) {
        let rect = self.window.frame();
        let x = rect.origin.x as f64;
        let y = util::bottom_left_to_top_left(rect);
        if self.previous_position.as_deref() != Some(&(x, y)) {
            *self.previous_position = Some(Box::new((x, y)));
            let scale_factor = self.window.scale_factor();
            let physical_pos = LogicalPosition::<f64>::from((x, y)).to_physical(scale_factor);
            self.queue_event(WindowEvent::Moved(physical_pos));
        }
    }

    fn view_size(&self) -> LogicalSize<f64> {
        let size = self.window.contentView().frame().size;
        LogicalSize::new(size.width as f64, size.height as f64)
    }
}
