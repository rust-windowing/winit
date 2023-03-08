use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use sctk::reexports::client::protocol::wl_output::WlOutput;
use sctk::reexports::client::Display;

use sctk::environment::Environment;
use sctk::output::OutputStatusListener;

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::platform_impl::platform::{
    MonitorHandle as PlatformMonitorHandle, VideoMode as PlatformVideoMode,
};

use super::env::WinitEnv;
use super::event_loop::EventLoopWindowTarget;

/// Output manager.
pub struct OutputManager {
    /// A handle that actually performs all operations on outputs.
    handle: OutputManagerHandle,

    _output_listener: OutputStatusListener,
}

impl OutputManager {
    pub fn new(env: &Environment<WinitEnv>) -> Self {
        let handle = OutputManagerHandle::new();

        // Handle existing outputs.
        for output in env.get_all_outputs() {
            match sctk::output::with_output_info(&output, |info| info.obsolete) {
                Some(false) => (),
                // The output is obsolete or we've failed to access its data, skipping.
                _ => continue,
            }

            // The output is present and unusable, add it to the output manager manager.
            handle.add_output(output);
        }

        let handle_for_listener = handle.clone();

        let output_listener = env.listen_for_outputs(move |output, info, _| {
            if info.obsolete {
                handle_for_listener.remove_output(output)
            } else {
                handle_for_listener.add_output(output)
            }
        });

        Self {
            handle,
            _output_listener: output_listener,
        }
    }

    pub fn handle(&self) -> OutputManagerHandle {
        self.handle.clone()
    }
}

/// A handle to output manager.
#[derive(Debug, Clone)]
pub struct OutputManagerHandle {
    outputs: Arc<Mutex<VecDeque<MonitorHandle>>>,
}

impl OutputManagerHandle {
    fn new() -> Self {
        let outputs = Arc::new(Mutex::new(VecDeque::new()));
        Self { outputs }
    }

    /// Handle addition of the output.
    fn add_output(&self, output: WlOutput) {
        let mut outputs = self.outputs.lock().unwrap();
        let position = outputs.iter().position(|handle| handle.proxy == output);
        if position.is_none() {
            outputs.push_back(MonitorHandle::new(output));
        }
    }

    /// Handle removal of the output.
    fn remove_output(&self, output: WlOutput) {
        let mut outputs = self.outputs.lock().unwrap();
        let position = outputs.iter().position(|handle| handle.proxy == output);
        if let Some(position) = position {
            outputs.remove(position);
        }
    }

    /// Get all observed outputs.
    pub fn available_outputs(&self) -> VecDeque<MonitorHandle> {
        self.outputs.lock().unwrap().clone()
    }
}

#[derive(Clone, Debug)]
pub struct MonitorHandle {
    pub(crate) proxy: WlOutput,
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

impl MonitorHandle {
    #[inline]
    pub(crate) fn new(proxy: WlOutput) -> Self {
        Self { proxy }
    }

    #[inline]
    pub fn name(&self) -> Option<String> {
        sctk::output::with_output_info(&self.proxy, |info| {
            format!("{} ({})", info.model, info.make)
        })
    }

    #[inline]
    pub fn native_identifier(&self) -> u32 {
        sctk::output::with_output_info(&self.proxy, |info| info.id).unwrap_or(0)
    }

    #[inline]
    pub fn size(&self) -> PhysicalSize<u32> {
        match sctk::output::with_output_info(&self.proxy, |info| {
            info.modes
                .iter()
                .find(|mode| mode.is_current)
                .map(|mode| mode.dimensions)
        }) {
            Some(Some((w, h))) => (w as u32, h as u32),
            _ => (0, 0),
        }
        .into()
    }

    #[inline]
    pub fn position(&self) -> PhysicalPosition<i32> {
        sctk::output::with_output_info(&self.proxy, |info| info.location)
            .unwrap_or((0, 0))
            .into()
    }

    #[inline]
    pub fn refresh_rate_millihertz(&self) -> Option<u32> {
        sctk::output::with_output_info(&self.proxy, |info| {
            info.modes
                .iter()
                .find_map(|mode| mode.is_current.then_some(mode.refresh_rate as u32))
        })
        .flatten()
    }

    #[inline]
    pub fn scale_factor(&self) -> i32 {
        sctk::output::with_output_info(&self.proxy, |info| info.scale_factor).unwrap_or(1)
    }

    #[inline]
    pub fn video_modes(&self) -> impl Iterator<Item = PlatformVideoMode> {
        let modes = sctk::output::with_output_info(&self.proxy, |info| info.modes.clone())
            .unwrap_or_default();

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

impl<T> EventLoopWindowTarget<T> {
    #[inline]
    pub fn display(&self) -> &Display {
        &self.display
    }

    #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        self.output_manager.handle.available_outputs()
    }

    #[inline]
    pub fn primary_monitor(&self) -> Option<PlatformMonitorHandle> {
        // There's no primary monitor on Wayland.
        None
    }
}
