//! `CADisplayLink`-paced redraws.
//!
//! When a window has the display link enabled, a `CADisplayLink` created from the
//! window's view sends `RedrawRequested` on every tick, so frames are driven by the
//! display rather than by `request_redraw`. It's paused only while the window is
//! occluded (covered, minimized, hidden, on another Space), from
//! `windowDidChangeOcclusionState:`, so it flips once per visibility change.

use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::sel;
use objc2_foundation::{NSRunLoop, NSRunLoopCommonModes};
use objc2_quartz_core::{CACurrentMediaTime, CADisplayLink, CAFrameRateRange};

use super::view::WinitView;

/// A `CADisplayLink` scheduled on the main run loop in the common modes, so it keeps
/// ticking during live resize and menu tracking.
///
/// The link retains its target, the view, which owns this. That cycle is only broken
/// by [`DisplayLink::invalidate`], so it must be called when the window closes.
#[derive(Debug)]
pub(super) struct DisplayLink {
    link: Retained<CADisplayLink>,
}

impl DisplayLink {
    /// Creates a running display link that follows the display `view` is on and calls
    /// its `displayLinkTick:`.
    pub(super) fn new(view: &WinitView) -> Self {
        // SAFETY: `WinitView` implements `displayLinkTick:`, which takes the link.
        let link = unsafe { view.displayLinkWithTarget_selector(view, sel!(displayLinkTick:)) };
        // SAFETY: The main run loop and the common modes are valid for the link.
        unsafe { link.addToRunLoop_forMode(&NSRunLoop::mainRunLoop(), NSRunLoopCommonModes) };
        Self { link }
    }

    pub(super) fn set_frame_rate_range(&self, range: CAFrameRateRange) {
        self.link.setPreferredFrameRateRange(range);
    }

    pub(super) fn set_paused(&self, paused: bool) {
        self.link.setPaused(paused);
    }

    /// Stops the link and releases the view it retains.
    pub(super) fn invalidate(self) {
        self.link.invalidate();
    }

    /// The link's `targetTimestamp` (the latest tick's target), as an `Instant`.
    /// `None` before the first tick, while `targetTimestamp` is still 0.
    pub(super) fn target_timestamp(&self) -> Option<Instant> {
        let target = self.link.targetTimestamp();
        if target <= 0.0 {
            return None;
        }
        let (now, media_now) = (Instant::now(), CACurrentMediaTime());
        let delta = target - media_now;
        Some(if delta >= 0.0 {
            now + Duration::from_secs_f64(delta)
        } else {
            now.checked_sub(Duration::from_secs_f64(-delta)).unwrap_or(now)
        })
    }
}
