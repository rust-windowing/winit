use std::num::{NonZeroU16, NonZeroU32};

use sctk::output::{Mode, OutputData};
use sctk::reexports::client::protocol::wl_output::WlOutput;
use sctk::reexports::client::Proxy;

use crate::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize};
use crate::platform_impl::platform::VideoModeHandle as PlatformVideoModeHandle;

#[derive(Clone, Debug)]
pub struct MonitorHandle {
    pub(crate) proxy: WlOutput,
}

impl MonitorHandle {
    #[inline]
    pub(crate) fn new(proxy: WlOutput) -> Self {
        Self { proxy }
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| info.name.clone())
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| info.id)
    }

    #[inline]
    pub fn position(&self) -> Option<PhysicalPosition<i32>> {
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

    #[inline]
    pub fn scale_factor(&self) -> i32 {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.scale_factor()
    }

    #[inline]
    pub fn current_video_mode(&self) -> Option<PlatformVideoModeHandle> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| {
            let mode = info.modes.iter().find(|mode| mode.current).cloned();

            mode.map(|mode| {
                PlatformVideoModeHandle::Wayland(VideoModeHandle::new(self.clone(), mode))
            })
        })
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = PlatformVideoModeHandle> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        let modes = output_data.with_output_info(|info| info.modes.clone());

        let monitor = self.clone();

        modes.into_iter().map(move |mode| {
            PlatformVideoModeHandle::Wayland(VideoModeHandle::new(monitor.clone(), mode))
        })
    }
}

impl PartialEq for MonitorHandle {
    fn eq(&self, other: &Self) -> bool {
        self.native_identifier() == other.native_identifier()
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
        self.native_identifier().cmp(&other.native_identifier())
    }
}

impl std::hash::Hash for MonitorHandle {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.native_identifier().hash(state);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VideoModeHandle {
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) refresh_rate_millihertz: Option<NonZeroU32>,
    pub(crate) monitor: MonitorHandle,
}

impl VideoModeHandle {
    fn new(monitor: MonitorHandle, mode: Mode) -> Self {
        VideoModeHandle {
            size: (mode.dimensions.0 as u32, mode.dimensions.1 as u32).into(),
            refresh_rate_millihertz: NonZeroU32::new(mode.refresh_rate as u32),
            monitor: monitor.clone(),
        }
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    #[inline]
    pub fn bit_depth(&self) -> Option<NonZeroU16> {
        None
    }

    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Option<NonZeroU32> {
        self.refresh_rate_millihertz
    }

    pub fn monitor(&self) -> MonitorHandle {
        self.monitor.clone()
    }
}
