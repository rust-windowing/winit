use std::os::raw::c_void;
use std::ptr;

use objc2::foundation::{NSArray, NSObject, NSString};
use objc2::rc::{autoreleasepool, Id, Shared};
use objc2::runtime::Object;
use objc2::{declare_class, ClassType};

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
        window::WinitWindow,
    },
    window::{Fullscreen, WindowId},
};

struct WindowDelegateState {
    window: Id<WinitWindow, Shared>,

    // TODO: It's possible for delegate methods to be called asynchronously,
    // causing data races / `RefCell` panics.

    // This is set when WindowBuilder::with_fullscreen was set,
    // see comments of `window_did_fail_to_enter_fullscreen`
    initial_fullscreen: bool,

    // During `windowDidResize`, we use this to only send Moved if the position changed.
    previous_position: Option<(f64, f64)>,

    // Used to prevent redundant events.
    previous_scale_factor: f64,
}

impl WindowDelegateState {
    fn new(window: Id<WinitWindow, Shared>, initial_fullscreen: bool) -> Self {
        let scale_factor = window.scale_factor();
        let mut delegate_state = WindowDelegateState {
            window,
            initial_fullscreen,
            previous_position: None,
            previous_scale_factor: scale_factor,
        };

        if scale_factor != 1.0 {
            delegate_state.emit_static_scale_factor_changed_event();
        }

        delegate_state
    }

    // TODO: Remove this
    fn with_window<F, T>(&mut self, callback: F) -> Option<T>
    where
        F: FnOnce(&WinitWindow) -> T,
    {
        Some(callback(&self.window.clone()))
    }

    fn emit_event(&mut self, event: WindowEvent<'static>) {
        let event = Event::WindowEvent {
            window_id: WindowId(self.window.id()),
            event,
        };
        AppState::queue_event(EventWrapper::StaticEvent(event));
    }

    fn emit_static_scale_factor_changed_event(&mut self) {
        let scale_factor = self.get_scale_factor();
        if scale_factor == self.previous_scale_factor {
            return;
        };

        self.previous_scale_factor = scale_factor;
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
        let moved = self.previous_position != Some((x, y));
        if moved {
            self.previous_position = Some((x, y));
            let scale_factor = self.get_scale_factor();
            let physical_pos = LogicalPosition::<f64>::from((x, y)).to_physical(scale_factor);
            self.emit_event(WindowEvent::Moved(physical_pos));
        }
    }

    fn get_scale_factor(&self) -> f64 {
        self.window.scale_factor()
    }

    fn view_size(&self) -> LogicalSize<f64> {
        let size = self.window.contentView().frame().size;
        LogicalSize::new(size.width as f64, size.height as f64)
    }
}

declare_class!(
    #[derive(Debug)]
    pub(crate) struct WinitWindowDelegate {
        state: *mut c_void,
    }

    unsafe impl ClassType for WinitWindowDelegate {
        type Super = NSObject;
    }

    unsafe impl WinitWindowDelegate {
        #[sel(dealloc)]
        fn dealloc(&mut self) {
            self.with_state(|state| unsafe {
                drop(Box::from_raw(state as *mut WindowDelegateState));
            });
        }

        #[sel(initWithWinit:)]
        fn init_with_winit(&mut self, state: *mut c_void) -> Option<&mut Self> {
            let this: Option<&mut Self> = unsafe { msg_send![self, init] };
            this.map(|this| {
                *this.state = state;
                this.with_state(|state| {
                    state.window.setDelegate(Some(&*this));
                });
                this
            })
        }
    }

    // NSWindowDelegate + NSDraggingDestination protocols
    unsafe impl WinitWindowDelegate {
        #[sel(windowShouldClose:)]
        fn window_should_close(&self, _: Option<&Object>) -> bool {
            trace_scope!("windowShouldClose:");
            self.with_state(|state| state.emit_event(WindowEvent::CloseRequested));
            false
        }

        #[sel(windowWillClose:)]
        fn window_will_close(&self, _: Option<&Object>) {
            trace_scope!("windowWillClose:");
            self.with_state(|state| {
                // `setDelegate:` retains the previous value and then autoreleases it
                autoreleasepool(|_| {
                    // Since El Capitan, we need to be careful that delegate methods can't
                    // be called after the window closes.
                    state.window.setDelegate(None);
                });
                state.emit_event(WindowEvent::Destroyed);
            });
        }

        #[sel(windowDidResize:)]
        fn window_did_resize(&self, _: Option<&Object>) {
            trace_scope!("windowDidResize:");
            self.with_state(|state| {
                // NOTE: WindowEvent::Resized is reported in frameDidChange.
                state.emit_move_event();
            });
        }

        // This won't be triggered if the move was part of a resize.
        #[sel(windowDidMove:)]
        fn window_did_move(&self, _: Option<&Object>) {
            trace_scope!("windowDidMove:");
            self.with_state(|state| {
                state.emit_move_event();
            });
        }

        #[sel(windowDidChangeBackingProperties:)]
        fn window_did_change_backing_properties(&self, _: Option<&Object>) {
            trace_scope!("windowDidChangeBackingProperties:");
            self.with_state(|state| {
                state.emit_static_scale_factor_changed_event();
            });
        }

        #[sel(windowDidBecomeKey:)]
        fn window_did_become_key(&self, _: Option<&Object>) {
            trace_scope!("windowDidBecomeKey:");
            self.with_state(|state| {
                // TODO: center the cursor if the window had mouse grab when it
                // lost focus
                state.emit_event(WindowEvent::Focused(true));
            });
        }

        #[sel(windowDidResignKey:)]
        fn window_did_resign_key(&self, _: Option<&Object>) {
            trace_scope!("windowDidResignKey:");
            self.with_state(|state| {
                // It happens rather often, e.g. when the user is Cmd+Tabbing, that the
                // NSWindowDelegate will receive a didResignKey event despite no event
                // being received when the modifiers are released.  This is because
                // flagsChanged events are received by the NSView instead of the
                // NSWindowDelegate, and as a result a tracked modifiers state can quite
                // easily fall out of synchrony with reality.  This requires us to emit
                // a synthetic ModifiersChanged event when we lose focus.

                // TODO(madsmtm): Remove the need for this unsafety
                let mut view = unsafe { Id::from_shared(state.window.view()) };

                // Both update the state and emit a ModifiersChanged event.
                if !view.state.modifiers.is_empty() {
                    view.state.modifiers = ModifiersState::empty();
                    state.emit_event(WindowEvent::ModifiersChanged(view.state.modifiers));
                }

                state.emit_event(WindowEvent::Focused(false));
            });
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

                self.with_state(|state| {
                    state.emit_event(WindowEvent::HoveredFile(path));
                });
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

                self.with_state(|state| {
                    state.emit_event(WindowEvent::DroppedFile(path));
                });
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
            self.with_state(|state| state.emit_event(WindowEvent::HoveredFileCancelled));
        }

        /// Invoked when before enter fullscreen
        #[sel(windowWillEnterFullscreen:)]
        fn window_will_enter_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowWillEnterFullscreen:");

            self.with_state(|state| {
                state.with_window(|window| {
                    let mut shared_state = window.lock_shared_state("window_will_enter_fullscreen");
                    shared_state.maximized = window.is_zoomed();
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
                            let current_monitor = Some(window.current_monitor_inner());
                            shared_state.fullscreen = Some(Fullscreen::Borderless(current_monitor))
                        }
                    }
                    shared_state.in_fullscreen_transition = true;
                })
            });
        }

        /// Invoked when before exit fullscreen
        #[sel(windowWillExitFullScreen:)]
        fn window_will_exit_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowWillExitFullScreen:");

            self.with_state(|state| {
                state.with_window(|window| {
                    let mut shared_state = window.lock_shared_state("window_will_exit_fullscreen");
                    shared_state.in_fullscreen_transition = true;
                });
            });
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
            self.with_state(|state| {
                state.with_window(|window| {
                    let shared_state =
                        window.lock_shared_state("window_will_use_fullscreen_presentation_options");
                    if let Some(Fullscreen::Exclusive(_)) = shared_state.fullscreen {
                        options = NSApplicationPresentationOptions::NSApplicationPresentationFullScreen
                            | NSApplicationPresentationOptions::NSApplicationPresentationHideDock
                            | NSApplicationPresentationOptions::NSApplicationPresentationHideMenuBar;
                    }
                })
            });

            options
        }

        /// Invoked when entered fullscreen
        #[sel(windowDidEnterFullscreen:)]
        fn window_did_enter_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowDidEnterFullscreen:");
            self.with_state(|state| {
                state.initial_fullscreen = false;
                state.with_window(|window| {
                    let mut shared_state = window.lock_shared_state("window_did_enter_fullscreen");
                    shared_state.in_fullscreen_transition = false;
                    let target_fullscreen = shared_state.target_fullscreen.take();
                    drop(shared_state);
                    if let Some(target_fullscreen) = target_fullscreen {
                        window.set_fullscreen(target_fullscreen);
                    }
                });
            });
        }

        /// Invoked when exited fullscreen
        #[sel(windowDidExitFullscreen:)]
        fn window_did_exit_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowDidExitFullscreen:");

            self.with_state(|state| {
                state.with_window(|window| {
                    window.restore_state_from_fullscreen();
                    let mut shared_state = window.lock_shared_state("window_did_exit_fullscreen");
                    shared_state.in_fullscreen_transition = false;
                    let target_fullscreen = shared_state.target_fullscreen.take();
                    drop(shared_state);
                    if let Some(target_fullscreen) = target_fullscreen {
                        window.set_fullscreen(target_fullscreen);
                    }
                })
            });
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
        #[sel(windowDidFailToEnterFullscreen:)]
        fn window_did_fail_to_enter_fullscreen(&self, _: Option<&Object>) {
            trace_scope!("windowDidFailToEnterFullscreen:");
            self.with_state(|state| {
                state.with_window(|window| {
                    let mut shared_state =
                        window.lock_shared_state("window_did_fail_to_enter_fullscreen");
                    shared_state.in_fullscreen_transition = false;
                    shared_state.target_fullscreen = None;
                });
                if state.initial_fullscreen {
                    #[allow(clippy::let_unit_value)]
                    unsafe {
                        let _: () = msg_send![
                            &*state.window,
                            performSelector: sel!(toggleFullScreen:),
                            withObject: ptr::null::<Object>(),
                            afterDelay: 0.5,
                        ];
                    };
                } else {
                    state.with_window(|window| window.restore_state_from_fullscreen());
                }
            });
        }

        // Invoked when the occlusion state of the window changes
        #[sel(windowDidChangeOcclusionState:)]
        fn window_did_change_occlusion_state(&self, _: Option<&Object>) {
            trace_scope!("windowDidChangeOcclusionState:");
            self.with_state(|state| {
                state.emit_event(WindowEvent::Occluded(
                    !state
                        .window
                        .occlusionState()
                        .contains(NSWindowOcclusionState::NSWindowOcclusionStateVisible),
                ))
            });
        }
    }
);

impl WinitWindowDelegate {
    pub fn new(window: Id<WinitWindow, Shared>, initial_fullscreen: bool) -> Id<Self, Shared> {
        let state = WindowDelegateState::new(window, initial_fullscreen);
        unsafe {
            // This is free'd in `dealloc`
            let state_ptr = Box::into_raw(Box::new(state)) as *mut c_void;
            msg_send_id![msg_send_id![Self::class(), alloc], initWithWinit: state_ptr]
        }
    }

    // This function is definitely unsafe (&self -> &mut state), but labeling that
    // would increase boilerplate and wouldn't really clarify anything...
    fn with_state<F: FnOnce(&mut WindowDelegateState) -> T, T>(&self, callback: F) {
        let state_ptr = unsafe {
            let state_ptr: *mut c_void = *self.state;
            &mut *(state_ptr as *mut WindowDelegateState)
        };
        callback(state_ptr);
    }
}
