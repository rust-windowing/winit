use std::{collections::VecDeque, os::unix::prelude::AsRawFd, sync::Arc};

use drm::control::{atomic, property, AtomicCommitFlags, Device};
use parking_lot::Mutex;

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError},
    platform::unix::Card,
    window::{CursorIcon, Fullscreen},
};

use super::event_loop::find_prop_id;

pub struct Window {
    mode: drm::control::Mode,
    connector: drm::control::connector::Info,
    ping: calloop::ping::Ping,
    plane: drm::control::plane::Handle,
    cursor: Arc<Mutex<PhysicalPosition<f64>>>,
    card: Card,
}

impl Window {
    pub fn new<T>(
        event_loop_window_target: &super::event_loop::EventLoopWindowTarget<T>,
        _attributes: crate::window::WindowAttributes,
        _platform_attributes: crate::platform_impl::PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, crate::error::OsError> {
        let mut atomic_req = atomic::AtomicModeReq::new();
        atomic_req.add_property(
            event_loop_window_target.connector.handle(),
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.connector.handle(),
                "CRTC_ID",
            )
            .ok_or_else(|| {
                crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::KmsMisc("Could not get CRTC_ID"),
                )
            })?,
            property::Value::CRTC(Some(event_loop_window_target.crtc.handle())),
        );
        let blob = event_loop_window_target
            .device
            .create_property_blob(&event_loop_window_target.mode)
            .map_err(|_| {
                crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::KmsMisc("Failed to create blob"),
                )
            })?;
        atomic_req.add_property(
            event_loop_window_target.crtc.handle(),
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.crtc.handle(),
                "MODE_ID",
            )
            .ok_or_else(|| {
                crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::KmsMisc("Could not get MODE_ID"),
                )
            })?,
            blob,
        );
        atomic_req.add_property(
            event_loop_window_target.crtc.handle(),
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.crtc.handle(),
                "ACTIVE",
            )
            .ok_or_else(|| {
                crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::KmsMisc("Could not get ACTIVE"),
                )
            })?,
            property::Value::Boolean(true),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "CRTC_ID",
            )
            .expect("Could not get CRTC_ID"),
            property::Value::CRTC(Some(event_loop_window_target.crtc.handle())),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "SRC_X",
            )
            .expect("Could not get SRC_X"),
            property::Value::UnsignedRange(0),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "SRC_Y",
            )
            .expect("Could not get SRC_Y"),
            property::Value::UnsignedRange(0),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "SRC_W",
            )
            .expect("Could not get SRC_W"),
            property::Value::UnsignedRange((event_loop_window_target.mode.size().0 as u64) << 16),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "SRC_H",
            )
            .expect("Could not get SRC_H"),
            property::Value::UnsignedRange((event_loop_window_target.mode.size().1 as u64) << 16),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "CRTC_X",
            )
            .expect("Could not get CRTC_X"),
            property::Value::SignedRange(0),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "CRTC_Y",
            )
            .expect("Could not get CRTC_Y"),
            property::Value::SignedRange(0),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "CRTC_W",
            )
            .expect("Could not get CRTC_W"),
            property::Value::UnsignedRange(event_loop_window_target.mode.size().0 as u64),
        );
        atomic_req.add_property(
            event_loop_window_target.plane,
            find_prop_id(
                &event_loop_window_target.device,
                event_loop_window_target.plane,
                "CRTC_H",
            )
            .expect("Could not get CRTC_H"),
            property::Value::UnsignedRange(event_loop_window_target.mode.size().1 as u64),
        );

        event_loop_window_target
            .device
            .atomic_commit(AtomicCommitFlags::ALLOW_MODESET, atomic_req)
            .map_err(|_| {
                crate::error::OsError::new(
                    line!(),
                    file!(),
                    crate::platform_impl::OsError::KmsMisc("Failed to set mode"),
                )
            })?;

        Ok(Self {
            mode: event_loop_window_target.mode.clone(),
            connector: event_loop_window_target.connector.clone(),
            plane: event_loop_window_target.plane.clone(),
            cursor: event_loop_window_target.cursor_arc.clone(),
            ping: event_loop_window_target.event_loop_awakener.clone(),
            card: event_loop_window_target.device.clone(),
        })
    }
    #[inline]
    pub fn id(&self) -> super::WindowId {
        super::WindowId
    }

    #[inline]
    pub fn set_title(&self, _title: &str) {}

    #[inline]
    pub fn set_visible(&self, _visible: bool) {}

    #[inline]
    pub fn is_visible(&self) -> Option<bool> {
        Some(true)
    }

    #[inline]
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Ok(PhysicalPosition::new(0, 0))
    }

    #[inline]
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
        Ok(PhysicalPosition::new(0, 0))
    }

    #[inline]
    pub fn set_outer_position(&self, _position: Position) {}

    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        let size = self.mode.size();
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
    }

    #[inline]
    pub fn set_min_inner_size(&self, _dimensions: Option<Size>) {}

    #[inline]
    pub fn set_max_inner_size(&self, _dimensions: Option<Size>) {}

    #[inline]
    pub fn set_resizable(&self, _resizable: bool) {}

    #[inline]
    pub fn is_resizable(&self) -> bool {
        false
    }

    #[inline]
    pub fn set_cursor_icon(&self, _cursor: CursorIcon) {}

    #[inline]
    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        Ok(())
    }

    #[inline]
    pub fn set_cursor_visible(&self, _visible: bool) {}

    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        Ok(())
    }

    #[inline]
    pub fn set_cursor_hittest(&self, _hittest: bool) -> Result<(), ExternalError> {
        Ok(())
    }

    #[inline]
    pub fn scale_factor(&self) -> f64 {
        1.0
    }

    #[inline]
    pub fn set_cursor_position(&self, position: Position) -> Result<(), ExternalError> {
        *self.cursor.lock() = position.to_physical(1.0);
        Ok(())
    }

    #[inline]
    pub fn set_maximized(&self, _maximized: bool) {}

    #[inline]
    pub fn is_maximized(&self) -> bool {
        true
    }

    #[inline]
    pub fn set_minimized(&self, _minimized: bool) {}

    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        Some(Fullscreen::Exclusive(crate::monitor::VideoMode {
            video_mode: crate::platform_impl::VideoMode::Kms(super::VideoMode(
                self.mode,
                self.connector.clone(),
            )),
        }))
    }

    #[inline]
    pub fn set_fullscreen(&self, _monitor: Option<Fullscreen>) {}

    #[inline]
    pub fn set_decorations(&self, _decorations: bool) {}

    pub fn is_decorated(&self) -> bool {
        false
    }

    #[inline]
    pub fn set_ime_position(&self, _position: Position) {}

    #[inline]
    pub fn set_ime_allowed(&self, _allowed: bool) {}

    #[inline]
    pub fn request_redraw(&self) {
        self.ping.ping();
    }

    #[inline]
    pub fn current_monitor(&self) -> Option<super::MonitorHandle> {
        Some(super::MonitorHandle(self.connector.clone()))
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<super::MonitorHandle> {
        self.card
            .resource_handles()
            .unwrap()
            .connectors()
            .iter()
            .map(|f| super::MonitorHandle(self.card.get_connector(*f).unwrap()))
            .collect()
    }

    #[inline]
    pub fn raw_window_handle(&self) -> raw_window_handle::DrmHandle {
        let mut rwh = raw_window_handle::DrmHandle::empty();
        rwh.fd = self.card.as_raw_fd();
        rwh.plane = self.plane.into();
        rwh
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<crate::monitor::MonitorHandle> {
        Some(crate::monitor::MonitorHandle {
            inner: crate::platform_impl::MonitorHandle::Kms(super::MonitorHandle(
                self.connector.clone(),
            )),
        })
    }
}
