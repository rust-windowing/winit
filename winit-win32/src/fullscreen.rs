use std::ptr;

use tracing::warn;
use windows_sys::Win32::Graphics::Gdi::{
    CDS_FULLSCREEN, ChangeDisplaySettingsExW, DISP_CHANGE_SUCCESSFUL,
};
use winit_core::monitor::{Fullscreen, MonitorHandle as CoreMonitorHandle};

use crate::monitor::{self, MonitorHandle, VideoModeHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DisplayTransition {
    Applied,
    Unchanged,
    Windowed,
}

struct ExclusiveMode {
    monitor: CoreMonitorHandle,
    mode: Option<VideoModeHandle>,
}

impl ExclusiveMode {
    fn prepare(fullscreen: &Option<Fullscreen>, require_mode: bool) -> Result<Option<Self>, ()> {
        let Some(Fullscreen::Exclusive(monitor, mode)) = fullscreen else {
            return Ok(None);
        };
        let Some(native_monitor) = monitor.cast_ref::<MonitorHandle>() else {
            warn!("Exclusive fullscreen requires a Windows monitor");
            return Err(());
        };
        let native_mode = if require_mode {
            match native_monitor.video_mode_handles().find(|candidate| &candidate.mode == mode) {
                Some(mode) => Some(mode),
                None => {
                    warn!("Exclusive fullscreen video mode is no longer available");
                    return Err(());
                },
            }
        } else {
            None
        };
        Ok(Some(Self { monitor: monitor.clone(), mode: native_mode }))
    }

    fn change(&self, activate: bool) -> bool {
        let monitor_info = match monitor::get_monitor_info(self.monitor.native_id() as _) {
            Ok(info) => info,
            Err(error) => {
                // A disconnected monitor has no desktop mode left to restore. Do not fall back
                // to a null device name, which would change the primary monitor instead.
                if !activate
                    && monitor::is_monitor_connected(self.monitor.native_id() as _)
                        .is_ok_and(|connected| !connected)
                {
                    return true;
                }
                warn!("Failed to obtain exclusive fullscreen monitor information: {error}");
                return false;
            },
        };
        let mode = if activate {
            let Some(mode) = self.mode.as_ref() else {
                return false;
            };
            &*mode.native_video_mode as *const _
        } else {
            ptr::null()
        };
        let flags = if activate { CDS_FULLSCREEN } else { 0 };
        let status = unsafe {
            ChangeDisplaySettingsExW(
                monitor_info.szDevice.as_ptr(),
                mode,
                ptr::null_mut(),
                flags,
                ptr::null(),
            )
        };
        if status != DISP_CHANGE_SUCCESSFUL {
            warn!(status, activate, "Failed to change exclusive fullscreen display mode");
            return false;
        }
        true
    }
}

pub(crate) fn change_display_mode(
    old: &Option<Fullscreen>,
    new: &Option<Fullscreen>,
) -> DisplayTransition {
    let changing_monitor = match (old, new) {
        (Some(Fullscreen::Exclusive(old, _)), Some(Fullscreen::Exclusive(new, _))) => {
            old.native_id() != new.native_id()
        },
        _ => false,
    };
    let old = match ExclusiveMode::prepare(old, changing_monitor) {
        Ok(mode) => mode,
        Err(()) => return DisplayTransition::Unchanged,
    };
    let new = match ExclusiveMode::prepare(new, true) {
        Ok(mode) => mode,
        Err(()) => return DisplayTransition::Unchanged,
    };
    transition(old.as_ref(), new.as_ref(), changing_monitor, ExclusiveMode::change)
}

fn transition<T>(
    old: Option<&T>,
    new: Option<&T>,
    changing_monitor: bool,
    mut change: impl FnMut(&T, bool) -> bool,
) -> DisplayTransition {
    if let Some(new) = new {
        if let Some(old) = old.filter(|_| changing_monitor) {
            if !change(old, false) {
                return DisplayTransition::Unchanged;
            }
            if change(new, true) {
                return DisplayTransition::Applied;
            }
            if change(old, true) {
                return DisplayTransition::Unchanged;
            }
            warn!("Failed to restore previous exclusive fullscreen mode; returning to windowed");
            return DisplayTransition::Windowed;
        }
        if !change(new, true) {
            return DisplayTransition::Unchanged;
        }
    } else if let Some(old) = old {
        if !change(old, false) {
            return DisplayTransition::Unchanged;
        }
    }
    DisplayTransition::Applied
}

#[cfg(test)]
#[path = "fullscreen/test.rs"]
mod tests;
