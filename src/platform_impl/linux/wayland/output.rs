use std::borrow::Cow;
use std::num::NonZeroU32;

use sctk::output::OutputData;
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

    fn current_video_mode(&self) -> Option<crate::monitor::VideoMode> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| {
            let mode = info.modes.iter().find(|mode| mode.current).cloned();

            mode.map(|mode| VideoMode {
                size: info
                    .logical_size
                    .map(|(x, y)| (x as u32, y as u32))
                    .unwrap_or_else(|| (mode.dimensions.0 as u32, mode.dimensions.1 as u32))
                    .into(),
                bit_depth: None,
                refresh_rate_millihertz: NonZeroU32::new(mode.refresh_rate as u32),
            })
        })
    }

    fn video_modes(&self) -> Box<dyn Iterator<Item = VideoMode>> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        let (size, modes) =
            output_data.with_output_info(|info| (info.logical_size, info.modes.clone()));

        Box::new(modes.into_iter().map(move |mode| {
            VideoMode {
                size: size
                    .map(|(x, y)| (x as u32, y as u32))
                    .unwrap_or_else(|| (mode.dimensions.0 as u32, mode.dimensions.1 as u32))
                    .into(),
                bit_depth: None,
                refresh_rate_millihertz: NonZeroU32::new(mode.refresh_rate as u32),
            }
        }))
    }
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.native_id() == other.native_id()
    }
}

impl Eq for MonitorHandle {}

impl PartialOrd for MonitorHandle {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MonitorHandle {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.native_id().cmp(&other.native_id())
    }
}

impl std::hash::Hash for MonitorHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.native_id().hash(state);
    }
}
