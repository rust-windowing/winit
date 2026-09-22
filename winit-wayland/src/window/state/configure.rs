use std::sync::Arc;

use dpi::LogicalSize;
use sctk::reexports::csd_frame::{DecorationsFrame, WindowState as XdgWindowState};
use sctk::shell::xdg::popup::{ConfigureKind, PopupConfigure};
use sctk::shell::xdg::window::{DecorationMode, WindowConfigure};
use sctk::shm::Shm;
use sctk::subcompositor::SubcompositorState;
use tracing::warn;

#[cfg(feature = "sctk-adwaita")]
use super::create_sctk_adwaita_config;
use super::{MIN_WINDOW_SIZE, WindowState, WindowType, WinitFrame};

impl WindowState {
    pub fn configure_popup(&mut self, configure: PopupConfigure) -> bool {
        // NOTE: when using fractional scaling or wl_compositor@v6 the scaling
        // should be delivered before the first configure, thus apply it to
        // properly scale the physical sizes provided by the users.
        if let Some(initial_size) = self.initial_size.take() {
            self.size = initial_size.to_logical(self.scale_factor());
        }

        // The popup was constrained to a different size by the compositor
        let constrained = self.size.width != configure.width as u32
            || self.size.height != configure.height as u32;
        let new_size =
            LogicalSize { width: configure.width as u32, height: configure.height as u32 };

        // NOTE: Set the configure before doing a resize, since we query it during it.
        if let WindowType::Popup { last_configure, .. } = &mut self.window {
            let kind = configure.kind.clone();
            *last_configure = Some(configure);

            // Always resize on the initial configure to properly initialize the viewport
            // destination and window geometry. This is required for fractional scaling
            // to work correctly: without calling resize(), viewport.set_destination()
            // is never called, and the compositor would interpret the buffer size as
            // logical pixels, making the popup appear at the wrong size. Also resize
            // when the compositor constrained us to a different size than requested.
            if matches!(kind, ConfigureKind::Initial) || constrained {
                self.resize(new_size);
                true
            } else {
                false
            }
        } else {
            tracing::error!(
                "configure_popup called for window type unequal of popup. This should never \
                 happen, because we start configuring with a popup"
            );
            false
        }
    }

    /// Creates (or drops) the CSD frame per `configure`'s decoration mode, and computes the
    /// surface size to apply, accounting for borders, configure bounds, and resize increments.
    ///
    /// Shared between `configure_window` and `configure_dialog`, since both configure an
    /// `xdg_toplevel`-based surface from the same [`WindowConfigure`] event shape.
    fn configure_frame_and_size(
        &mut self,
        configure: &WindowConfigure,
        shm: &Shm,
        subcompositor: &Option<Arc<SubcompositorState>>,
    ) -> LogicalSize<u32> {
        // NOTE: when using fractional scaling or wl_compositor@v6 the scaling
        // should be delivered before the first configure, thus apply it to
        // properly scale the physical sizes provided by the users.
        if let Some(initial_size) = self.initial_size.take() {
            self.size = initial_size.to_logical(self.scale_factor());
            self.stateless_size = self.size;
        }

        if let Some(subcompositor) = subcompositor.as_ref().filter(|_| {
            configure.decoration_mode == DecorationMode::Client
                && self.frame.is_none()
                && !self.csd_fails
        }) {
            match WinitFrame::new(
                &self.window,
                shm,
                #[cfg(feature = "sctk-adwaita")]
                self.compositor.clone(),
                subcompositor.clone(),
                self.queue_handle.clone(),
                #[cfg(feature = "sctk-adwaita")]
                create_sctk_adwaita_config(self.theme),
            ) {
                Ok(mut frame) => {
                    frame.set_title(&self.title);
                    frame.set_scaling_factor(self.scale_factor);
                    // Hide the frame if we were asked to not decorate.
                    frame.set_hidden(!self.decorate);
                    self.frame = Some(frame);
                },
                Err(err) => {
                    warn!("Failed to create client side decorations frame: {err}");
                    self.csd_fails = true;
                },
            }
        } else if configure.decoration_mode == DecorationMode::Server {
            // Drop the frame for server side decorations to save resources.
            self.frame = None;
        }

        let stateless = Self::is_stateless(configure);

        let (mut new_size, constrain) = if let Some(frame) = self.frame.as_mut() {
            // Configure the window states.
            frame.update_state(configure.state);

            match configure.new_size {
                (Some(width), Some(height)) => {
                    let (width, height) = frame.subtract_borders(width, height);
                    let width = width.map(|w| w.get()).unwrap_or(1);
                    let height = height.map(|h| h.get()).unwrap_or(1);
                    ((width, height).into(), false)
                },
                (..) if stateless => (self.stateless_size, true),
                _ => (self.size, true),
            }
        } else {
            match configure.new_size {
                (Some(width), Some(height)) => ((width.get(), height.get()).into(), false),
                _ if stateless => (self.stateless_size, true),
                _ => (self.size, true),
            }
        };

        // Apply configure bounds only when compositor let the user decide what size to pick.
        if constrain {
            let bounds = self.surface_size_bounds(configure);
            new_size.width =
                bounds.0.map(|bound_w| new_size.width.min(bound_w.get())).unwrap_or(new_size.width);
            new_size.height = bounds
                .1
                .map(|bound_h| new_size.height.min(bound_h.get()))
                .unwrap_or(new_size.height);
        }

        // Apply size increments.
        //
        // We conditionally apply increments to avoid conflicts with the compositor's layout rules:
        // 1. If the window is floating (constrain == true), we snap to increments to ensure the
        //    app's grid alignment.
        // 2. If the user is interactively resizing (is_resizing), we snap the size to provide
        //    feedback.
        //
        // However, we MUST NOT snap if the compositor enforces a specific size (constrain == false,
        // or states like Maximized/Tiled). Snapping in these cases (e.g. corner tiling) would
        // shrink the window below the allocated area, creating visible gaps between valid
        // windows or screen edges.
        let was_resizing = matches!(
            &self.window,
            WindowType::Window { last_configure: Some(last), .. } if last.is_resizing()
        );
        if (constrain || configure.is_resizing() || was_resizing)
            && !configure.is_maximized()
            && !configure.is_fullscreen()
            && !configure.is_tiled()
        {
            if let Some(increments) = self.resize_increments {
                // We use min size as a base size for the increments, similar to how X11 does it.
                //
                // This ensures that we can always reach the min size and the increments are
                // calculated from it.
                let snap = |size: u32, min: u32, increment: u32, floor: u32| {
                    let steps = size.saturating_sub(min) / increment;
                    let floor_steps = floor.saturating_sub(min).div_ceil(increment);
                    min + steps.max(floor_steps) * increment
                };
                let min = self.min_surface_size;
                let (width, height) = (
                    snap(new_size.width, min.width, increments.width, MIN_WINDOW_SIZE.width),
                    snap(new_size.height, min.height, increments.height, MIN_WINDOW_SIZE.height),
                );

                new_size = (width, height).into();
            }
        }

        new_size
    }

    pub fn configure_window(
        &mut self,
        configure: WindowConfigure,
        shm: &Shm,
        subcompositor: &Option<Arc<SubcompositorState>>,
    ) -> bool {
        let new_size = self.configure_frame_and_size(&configure, shm, subcompositor);

        let new_state = configure.state;
        if let WindowType::Window { last_configure, .. } = &mut self.window {
            let old_state = last_configure.as_ref().map(|configure| configure.state);
            let decoration_mode_changed = last_configure
                .as_ref()
                .is_some_and(|last| last.decoration_mode != configure.decoration_mode);

            let state_change_requires_resize = old_state
                .map(|old_state| {
                    !old_state
                        .symmetric_difference(new_state)
                        .difference(XdgWindowState::ACTIVATED | XdgWindowState::SUSPENDED)
                        .is_empty()
                })
                // NOTE: `None` is present for the initial configure, thus we must always resize.
                .unwrap_or(true);

            // NOTE: Set the configure before doing a resize, since we query it during it.
            *last_configure = Some(configure);

            if state_change_requires_resize
                || new_size != self.surface_size()
                || decoration_mode_changed
            {
                self.resize(new_size);
                true
            } else {
                false
            }
        } else {
            tracing::error!(
                "configure_window called for window type unequal of `Window`. This should never \
                 happen, because we start configuring with a `Window`"
            );
            false
        }
    }
}
