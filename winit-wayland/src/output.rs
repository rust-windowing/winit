use std::borrow::Cow;
use std::num::NonZeroU32;

use dpi::{LogicalPosition, PhysicalPosition};
use sctk::output::{Mode, OutputData};
use sctk::reexports::client::Proxy;
use sctk::reexports::client::protocol::wl_output::WlOutput;
use winit_core::monitor::{MonitorHandleProvider as CoreMonitorHandle, VideoMode};

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
    fn id(&self) -> u128 {
        self.native_id() as _
    }

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

    fn current_video_mode(&self) -> Option<winit_core::monitor::VideoMode> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| {
            let mode = info.modes.iter().find(|mode| mode.current).cloned();

            mode.map(|mode| wayland_mode_to_core_mode(mode, info.logical_size))
        })
    }

    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        let (size, modes) =
            output_data.with_output_info(|info| (info.logical_size, info.modes.clone()));

        Box::new(modes.into_iter().map(move |mode| wayland_mode_to_core_mode(mode, size)))
    }
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.native_id() == other.native_id()
    }
}

impl Eq for MonitorHandle {}

/// Convert the wayland's [`Mode`] to winit's [`VideoMode`].
fn wayland_mode_to_core_mode(mode: Mode, size: Option<(i32, i32)>) -> VideoMode {
    VideoMode::new(
        size.map(|(x, y)| (x as u32, y as u32))
            .unwrap_or_else(|| (mode.dimensions.0 as u32, mode.dimensions.1 as u32))
            .into(),
        None,
        NonZeroU32::new(mode.refresh_rate as u32),
    )
}
