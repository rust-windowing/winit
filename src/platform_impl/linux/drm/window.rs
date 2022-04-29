use std::collections::VecDeque;

use drm::control::{atomic, property, AtomicCommitFlags, Device};

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError},
    platform_impl::GBM_DEVICE,
    window::{CursorIcon, Fullscreen},
};

use super::event_loop::find_prop_id;

pub struct Window(
    drm::control::Mode,
    drm::control::connector::Info,
    calloop::ping::Ping,
);

impl Window {
    pub fn new<T>(
        event_loop_window_target: &super::event_loop::EventLoopWindowTarget<T>,
        _attributes: crate::window::WindowAttributes,
        _platform_attributes: crate::platform_impl::PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, crate::error::OsError> {
        let gbm = GBM_DEVICE.lock();
        let gbm = gbm.as_ref().map_err(|_| {
            crate::error::OsError::new(
                line!(),
                file!(),
                crate::platform_impl::OsError::DrmMisc("Failed to acquire gbm lock"),
            )
        })?;
        let &mode = event_loop_window_target
            .connector
            .modes()
            .get(0)
            .ok_or_else(|| {
                crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::DrmMisc("No modes found on connector"),
                )
            })?;

        let mut atomic_req = atomic::AtomicModeReq::new();
        atomic_req.add_property(
            event_loop_window_target.connector.handle(),
            find_prop_id(&gbm, event_loop_window_target.connector.handle(), "CRTC_ID").ok_or_else(
                || {
                    crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::DrmMisc("Could not get CRTC_ID"),
                    )
                },
            )?,
            property::Value::CRTC(Some(event_loop_window_target.crtc.handle())),
        );
        let blob = gbm.create_property_blob(&mode).map_err(|_| {
            crate::error::OsError::new(
                line!(),
                file!(),
                crate::platform_impl::OsError::DrmMisc("Failed to create blob"),
            )
        })?;
        atomic_req.add_property(
            event_loop_window_target.crtc.handle(),
            find_prop_id(&gbm, event_loop_window_target.crtc.handle(), "MODE_ID").ok_or_else(
                || {
                    crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::DrmMisc("Could not get MODE_ID"),
                    )
                },
            )?,
            blob,
        );
        atomic_req.add_property(
            event_loop_window_target.crtc.handle(),
            find_prop_id(&gbm, event_loop_window_target.crtc.handle(), "ACTIVE").ok_or_else(
                || {
                    crate::error::OsError::new(
                        line!(),
                        file!(),
                        crate::platform_impl::OsError::DrmMisc("Could not get ACTIVE"),
                    )
                },
            )?,
            property::Value::Boolean(true),
        );

        gbm.atomic_commit(AtomicCommitFlags::ALLOW_MODESET, atomic_req)
            .map_err(|_| {
                crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::DrmMisc("Failed to set mode"),
                )
            })?;

        Ok(Self(
            mode,
            event_loop_window_target.connector.clone(),
            event_loop_window_target.event_loop_awakener.clone(),
        ))
    }
    #[inline]
    pub fn id(&self) -> super::WindowId {
        super::WindowId
    }

    #[inline]
    pub fn set_title(&self, _title: &str) {}

    #[inline]
    pub fn set_visible(&self, visible: bool) {
        if !visible {
            eprintln!("It is not possible to make a window not visible in kmsdrm mode");
        }
    }

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        Some(true)
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Err(NotSupportedError::new())
    }

    #[inline]
    pub fn set_outer_position(&self, _position: Position) {
        eprintln!("The window cannot be moved in kmsdrm mode");
    }

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let size = self.0.size();
        PhysicalSize::new(size.0 as u32, size.1 as u32)
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        self.inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, _size: Size) {
        // It's technically possible to do this by changing video modes but that seems a little
        // restrictive
        eprintln!("The window cannot be resized in kmsdrm mode");
    }

    #[inline]
    pub fn set_min_inner_size(&self, _dimensions: Option<Size>) {
        eprintln!("The window cannot be resized in kmsdrm mode");
    }

    #[inline]
    pub fn set_max_inner_size(&self, _dimensions: Option<Size>) {
        // It's technically possible to do this by changing video modes but that seems a little
        // restrictive
        eprintln!("The window cannot be resized in kmsdrm mode");
    }

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {
        eprintln!("The window cannot be resized in kmsdrm mode");
    }

    #[inline]
    pub fn is_resizable(&self) -> bool {
        false
    }

    #[inline]
    pub fn set_cursor_icon(&self, _cursor: CursorIcon) {
        unimplemented!()
    }

    #[inline]
    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        eprintln!("The cursor is always grabbed in kmsdrm mode");
        Ok(())
    }

    #[inline]
    pub fn set_cursor_visible(&self, _visible: bool) {
        unimplemented!()
    }

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        eprintln!("The window cannot be dragged in kmsdrm mode");
        Ok(())
    }

    #[inline]
    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), ExternalError> {
        unimplemented!()
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, _position: Position) -> Result<(), ExternalError> {
        unimplemented!()
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {
        eprintln!("The window is always maximized in kmsdrm mode");
    }

    #[inline]
    pub fn is_maximized(&self) -> bool {
        true
    }

    #[inline]
    pub fn set_minimized(&self, _minimized: bool) {
        // By switching the crtc to the tty you can technically hide the "window".
        eprintln!("The window cannot be minimized in kmsdrm mode");
    }

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        Some(Fullscreen::Exclusive(crate::monitor::VideoMode {
            video_mode: crate::platform_impl::VideoMode::Drm(super::VideoMode(
                self.0,
                self.1.clone(),
            )),
        }))
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<Fullscreen>) {
        eprintln!("The window is always in fullscreen in kmsdrm mode");
    }

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {
        eprintln!("The window cannot be decorated in kmsdrm mode");
    }

    pub fn is_decorated(&self) -> bool {
        false
    }

    #[inline]
    pub fn set_ime_position(&self, _position: Position) {
        eprintln!("The window cannot be moved in kmsdrm mode");
    }

    #[inline]
    pub fn request_redraw(&self) {
        self.2.ping();
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<super::MonitorHandle> {
        Some(super::MonitorHandle(self.1.clone()))
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<super::MonitorHandle> {
        if let Ok(gbm) = &**GBM_DEVICE.lock() {
            gbm.resource_handles()
                .unwrap()
                .connectors()
                .iter()
                .map(|f| super::MonitorHandle(gbm.get_connector(*f).unwrap()))
                .collect()
        } else {
            VecDeque::new()
        }
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<crate::monitor::MonitorHandle> {
        Some(crate::monitor::MonitorHandle {
            inner: crate::platform_impl::MonitorHandle::Drm(super::MonitorHandle(self.1.clone())),
        })
    }
}
