use std::{collections::VecDeque, os::unix::prelude::AsRawFd, sync::Arc};

use super::MODE;
use drm::control::*;
use parking_lot::Mutex;

#[cfg(feature = "wayland")]
use sctk::reexports::calloop;

use crate::{
    dpi::{PhysicalPosition, PhysicalSize, Position, Size},
    error::{ExternalError, NotSupportedError, OsError},
    monitor::{MonitorHandle, VideoMode},
    platform::unix::Card,
    platform_impl,
    window::{CursorIcon, Fullscreen, WindowAttributes},
};

pub struct Window {
    connector: connector::Info,
    crtc: crtc::Info,
    ping: calloop::ping::Ping,
    plane: plane::Handle,
    cursor: Arc<Mutex<PhysicalPosition<f64>>>,
    card: Card,
}

fn find_prop_id<T: ResourceHandle>(
    card: &Card,
    handle: T,
    name: &'static str,
) -> Option<property::Handle> {
    let props = card.get_properties(handle).ok()?;
    let (ids, _vals) = props.as_props_and_values();
    ids.iter()
        .find(|&id| {
            let info = card.get_property(*id).unwrap();
            info.name().to_str().map(|x| x == name).unwrap_or(false)
        })
        .cloned()
}

macro_rules! find_prop_id {
    ($id_handle_1:expr,$handle:expr,$prop_name:literal) => {
        find_prop_id(&$id_handle_1, $handle, $prop_name).ok_or_else(|| {
            OsError::new(
                line!(),
                file!(),
                platform_impl::OsError::KmsMisc(concat!("could not get ", $prop_name)),
            )
        })
    };
}

macro_rules! add_property {
    ($atomic_req:expr,$handle:expr,$id_handle_1:expr,$prop_name:literal,$property:expr,) => {
        $atomic_req.add_property(
            $handle,
            find_prop_id!(&$id_handle_1, $handle, $prop_name)?,
            $property,
        );
    };
}

impl Window {
    pub fn new<T>(
        event_loop_window_target: &super::event_loop::EventLoopWindowTarget<T>,
        _attributes: WindowAttributes,
        _platform_attributes: platform_impl::PlatformSpecificWindowBuilderAttributes,
    ) -> Result<Self, OsError> {
        let mut atomic_req = atomic::AtomicModeReq::new();

        add_property!(
            atomic_req,
            event_loop_window_target.connector.handle(),
            event_loop_window_target.device,
            "CRTC_ID",
            property::Value::CRTC(Some(event_loop_window_target.crtc.handle())),
        );

        let ref mode = MODE.lock().ok_or_else(|| {
            OsError::new(
                line!(),
                file!(),
                platform_impl::OsError::KmsMisc("mode is not initialized"),
            )
        })?;

        let blob = event_loop_window_target
            .device
            .create_property_blob(mode)
            .map_err(|_| {
                OsError::new(
                    line!(),
                    file!(),
                    platform_impl::OsError::KmsMisc("failed to create blob"),
                )
            })?;

        add_property!(
            atomic_req,
            event_loop_window_target.crtc.handle(),
            event_loop_window_target.device,
            "MODE_ID",
            blob,
        );

        add_property!(
            atomic_req,
            event_loop_window_target.crtc.handle(),
            event_loop_window_target.device,
            "ACTIVE",
            property::Value::Boolean(true),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "CRTC_ID",
            property::Value::CRTC(Some(event_loop_window_target.crtc.handle())),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "SRC_X",
            property::Value::UnsignedRange(0),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "SRC_Y",
            property::Value::UnsignedRange(0),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "SRC_W",
            property::Value::UnsignedRange((mode.size().0 as u64) << 16),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "SRC_H",
            property::Value::UnsignedRange((mode.size().1 as u64) << 16),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "CRTC_X",
            property::Value::SignedRange(0),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "CRTC_Y",
            property::Value::SignedRange(0),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "CRTC_W",
            property::Value::UnsignedRange(mode.size().0 as u64),
        );

        add_property!(
            atomic_req,
            event_loop_window_target.plane,
            event_loop_window_target.device,
            "CRTC_H",
            property::Value::UnsignedRange(mode.size().1 as u64),
        );

        event_loop_window_target
            .device
            .atomic_commit(AtomicCommitFlags::ALLOW_MODESET, atomic_req)
            .map_err(|e| {
                OsError::new(
                    line!(),
                    file!(),
                    platform_impl::OsError::KmsError(format!("failed to set mode: {}", e)),
                )
            })?;

        Ok(Self {
            connector: event_loop_window_target.connector.clone(),
            crtc: event_loop_window_target.crtc.clone(),
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
        let size = MODE.lock().expect("mode is not initialized").size();
        PhysicalSize::new(size.0 as u32, size.1 as u32)
    }

    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        self.inner_size()
    }

    #[inline]
    pub fn set_inner_size(&self, _size: Size) {}

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
        Some(Fullscreen::Exclusive(VideoMode {
            video_mode: platform_impl::VideoMode::Kms(super::VideoMode {
                mode: MODE.lock().expect("mode is not initialized"),
                connector: self.connector.clone(),
            }),
        }))
    }

    #[inline]
    pub fn set_fullscreen(&self, monitor: Option<Fullscreen>) {
        match monitor {
            Some(Fullscreen::Exclusive(fullscreen)) => {
                let modes = self.connector.modes().to_vec();
                let connector = self.connector.clone();
                if let Some(mo) = modes.into_iter().find(move |&f| {
                    VideoMode {
                        video_mode: platform_impl::VideoMode::Kms(super::VideoMode {
                            mode: f,
                            connector: connector.clone(),
                        }),
                    } == fullscreen
                }) {
                    let mut atomic_req = atomic::AtomicModeReq::new();
                    let blob = self.card.create_property_blob(&mo).unwrap();

                    atomic_req.add_property(
                        self.crtc.handle(),
                        find_prop_id!(&self.card, self.crtc.handle(), "MODE_ID").unwrap(),
                        blob,
                    );

                    atomic_req.add_property(
                        self.plane,
                        find_prop_id!(&self.card, self.plane, "SRC_W").unwrap(),
                        property::Value::UnsignedRange((mo.size().0 as u64) << 16),
                    );

                    atomic_req.add_property(
                        self.plane,
                        find_prop_id!(&self.card, self.plane, "SRC_H").unwrap(),
                        property::Value::UnsignedRange((mo.size().1 as u64) << 16),
                    );

                    atomic_req.add_property(
                        self.plane,
                        find_prop_id!(&self.card, self.plane, "CRTC_W").unwrap(),
                        property::Value::UnsignedRange(mo.size().0 as u64),
                    );

                    atomic_req.add_property(
                        self.plane,
                        find_prop_id!(&self.card, self.plane, "CRTC_H").unwrap(),
                        property::Value::UnsignedRange(mo.size().1 as u64),
                    );

                    self.card
                        .atomic_commit(AtomicCommitFlags::ALLOW_MODESET, atomic_req)
                        .unwrap();
                }
            }
            _ => {}
        }
    }

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
        Some(super::MonitorHandle {
            connector: self.connector.clone(),
            name: (*MODE.lock())?.name().to_string_lossy().into_owned(),
        })
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<super::MonitorHandle> {
        if let Some(mode) = *MODE.lock() {
            self.card
                .resource_handles()
                .unwrap()
                .connectors()
                .iter()
                .map(|f| super::MonitorHandle {
                    connector: self.card.get_connector(*f).unwrap(),
                    name: mode.name().to_string_lossy().into_owned(),
                })
                .collect()
        } else {
            VecDeque::new()
        }
    }

    #[inline]
    pub fn raw_window_handle(&self) -> raw_window_handle::DrmHandle {
        let mut rwh = raw_window_handle::DrmHandle::empty();
        rwh.fd = self.card.as_raw_fd();
        rwh.plane = self.plane.into();
        rwh
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<MonitorHandle> {
        Some(MonitorHandle {
            inner: platform_impl::MonitorHandle::Kms(super::MonitorHandle {
                connector: self.connector.clone(),
                name: (*MODE.lock())?.name().to_string_lossy().into_owned(),
            }),
        })
    }
}
