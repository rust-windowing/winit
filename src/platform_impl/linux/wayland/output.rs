use std::borrow::Cow;
use std::num::NonZeroU16;

use sctk::output::{Mode, OutputData};
use sctk::reexports::client::protocol::wl_output::WlOutput;
use sctk::reexports::client::Proxy;

use crate::dpi::{LogicalPosition, PhysicalPosition};
use crate::monitor::{MonitorHandleProvider as CoreMonitorHandle, VideoMode};

#[derive(Clone, Debug)]
pub struct MonitorHandle {
    pub(crate) proxy: WlOutput,
}

impl MonitorHandle {
    #[inline]
    pub(crate) fn new(proxy: WlOutput) -> Self {
        Self { proxy }
    }
}

impl CoreMonitorHandle for MonitorHandle {
    fn native_id(&self) -> u64 {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| info.id as u64)
    }

    fn name(&self) -> Option<Cow<'_, str>> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| info.name.clone().map(Cow::Owned))
    }

    fn position(&self) -> Option<PhysicalPosition<i32>> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        Some(output_data.with_output_info(|info| {
            info.logical_position.map_or_else(
                || {
                    LogicalPosition::<i32>::from(info.location)
                        .to_physical(info.scale_factor as f64)
                },
                |logical_position| {
                    LogicalPosition::<i32>::from(logical_position)
                        .to_physical(info.scale_factor as f64)
                },
            )
        }))
    }

    fn scale_factor(&self) -> f64 {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.scale_factor() as f64
    }

    fn current_video_mode(&self) -> Option<crate::monitor::VideoMode> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| {
            let mode = info.modes.iter().find(|mode| mode.current).cloned();

            mode.map(wayland_mode_to_core_mode)
        })
    }

    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        let modes = output_data.with_output_info(|info| info.modes.clone());

        Box::new(modes.into_iter().map(wayland_mode_to_core_mode))
    }
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.native_id() == other.native_id()
    }
}

impl Eq for MonitorHandle {}

/// Convert the wayland's [`Mode`] to winit's [`VideoMode`].
fn wayland_mode_to_core_mode(mode: Mode) -> VideoMode {
    VideoMode {
        size: (mode.dimensions.0, mode.dimensions.1).into(),
        bit_depth: None,
        refresh_rate_millihertz: NonZeroU16::new(mode.refresh_rate as u16),
    }
}
