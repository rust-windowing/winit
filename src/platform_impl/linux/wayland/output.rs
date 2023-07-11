use sctk::reexports::client::protocol::wl_output::WlOutput;
use sctk::reexports::client::Proxy;

use sctk::output::OutputData;

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::platform_impl::platform::{
    MonitorHandle as PlatformMonitorHandle, VideoMode as PlatformVideoMode,
};

use super::event_loop::EventLoopWindowTarget;

impl<T> EventLoopWindowTarget<T> {
    #[inline]
    pub fn available_monitors(&self) -> Vec<MonitorHandle> {
        self.state
            .borrow()
            .output_state
            .outputs()
            .map(MonitorHandle::new)
            .collect()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<PlatformMonitorHandle> {
        // There's no primary monitor on Wayland.
        None
    }
}

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
    pub fn size(&self) -> PhysicalSize<u32> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        let dimensions = output_data.with_output_info(|info| {
            info.modes
                .iter()
                .find_map(|mode| mode.current.then_some(mode.dimensions))
        });

        match dimensions {
            Some((width, height)) => (width as u32, height as u32),
            _ => (0, 0),
        }
        .into()
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| info.location).into()
    }

    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.with_output_info(|info| {
            info.modes
                .iter()
                .find_map(|mode| mode.current.then_some(mode.refresh_rate as u32))
        })
    }

    #[inline]
    pub fn scale_factor(&self) -> i32 {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        output_data.scale_factor()
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = PlatformVideoMode> {
        let output_data = self.proxy.data::<OutputData>().unwrap();
        let modes = output_data.with_output_info(|info| info.modes.clone());

        let monitor = self.clone();

        modes.into_iter().map(move |mode| {
            PlatformVideoMode::Wayland(VideoMode {
                size: (mode.dimensions.0 as u32, mode.dimensions.1 as u32).into(),
                refresh_rate_millihertz: mode.refresh_rate as u32,
                bit_depth: 32,
                monitor: monitor.clone(),
            })
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
pub struct VideoMode {
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) bit_depth: u16,
    pub(crate) refresh_rate_millihertz: u32,
    pub(crate) monitor: MonitorHandle,
}

impl VideoMode {
    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    #[inline]
    pub fn bit_depth(&self) -> u16 {
        self.bit_depth
    }

    #[inline]
    pub fn refresh_rate_millihertz(&self) -> u32 {
        self.refresh_rate_millihertz
    }

    pub fn monitor(&self) -> PlatformMonitorHandle {
        PlatformMonitorHandle::Wayland(self.monitor.clone())
    }
}
