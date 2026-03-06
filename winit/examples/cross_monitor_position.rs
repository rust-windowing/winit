//! Demonstrates the `set_outer_position` cross-monitor scale factor bug.
//!
//! # Requirements
//!
//! Two monitors with **different** scale factors (e.g. 1x + 2x, 1.5x + 2x).
//!
//! # The bug
//!
//! `set_outer_position` converts coordinates using the *window's current*
//! scale factor instead of the *target monitor's* scale factor. When the
//! window is on a 1x monitor and you move it to a 2x monitor (or vice
//! versa), the wrong scale factor corrupts the position and the window
//! lands at the wrong spot.
//!
//! The platform determines which coordinate type is affected:
//!
//! - **macOS** converts Physical → Logical, so the bug shows with Physical input
//! - **Windows** converts Logical → Physical, so the bug shows with Logical input
//!
//! # How to use
//!
//! ```sh
//! cargo run --example cross_monitor_position
//! ```
//!
//! The example detects your monitors automatically. Press a key to move the
//! window to a different-scale monitor and check if it lands correctly:
//!
//! - **Space** — move using Physical coordinates (shows bug on macOS)
//! - **L** — move using Logical coordinates (shows bug on Windows)
//! - **Q** — quit
//!
//! Each press logs the target position vs the actual position. With the bug,
//! there is a large delta and the window visibly jumps to the wrong spot.
//! Press the same key again to move back — the test naturally alternates
//! between monitors.
//!
//! # Confirming the fix
//!
//! With the fix applied, target and actual match on every keypress:
//!
//! ```text
//! --- Test #1 (Physical) ---
//!   Current:  Physical(500, 300), window scale = 1.00x
//!   Target:   "Built-in Retina" — Physical(2760, 200), monitor scale = 2.00x
//!   Command:  set_outer_position(Physical(2760, 200))
//!   Actual:   Physical(2760, 200)
//!   PASS — landed at correct position (delta 0, 0)
//! ```
//!
//! # Reproducing the bug (without the fix)
//!
//! Change `self.scale_factor_for(&position)` back to `self.scale_factor()` in
//! `set_outer_position`:
//!
//! | Platform | File                                 |
//! |----------|--------------------------------------|
//! | macOS    | `winit-appkit/src/window_delegate.rs` |
//! | Windows  | `winit-win32/src/window.rs`           |
//!
//! Rebuild and press Space (macOS) or L (Windows). The window will
//! jump to the wrong position and the log will show a significant delta:
//!
//! ```text
//! --- Test #1 (Physical) ---
//!   Current:  Physical(500, 300), window scale = 1.00x
//!   Target:   "Built-in Retina" — Physical(2760, 200), monitor scale = 2.00x
//!   Command:  set_outer_position(Physical(2760, 200))
//!   Actual:   Physical(4520, 400)
//!   FAIL — delta (1760, 200) px — wrong scale factor used!
//! ```

use std::error::Error;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, PhysicalPosition, PhysicalSize, Position};
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::Key;
use winit::window::{Window, WindowAttributes, WindowId};

#[path = "util/fill.rs"]
mod fill;

/// Offset in physical pixels from the target monitor's origin.
const POSITION_OFFSET: i32 = 200;

/// Maximum acceptable delta in physical pixels between target and actual.
const POSITION_TOLERANCE: i32 = 5;

struct MonitorInfo {
    name: String,
    phys_pos: PhysicalPosition<i32>,
    phys_size: PhysicalSize<u32>,
    scale: f64,
}

#[derive(Default)]
struct App {
    window: Option<Box<dyn Window>>,
    monitors: Vec<MonitorInfo>,
    test_count: u32,
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.monitors = event_loop
            .available_monitors()
            .filter_map(|m| {
                let phys_pos = m.position()?;
                let phys_size = m.current_video_mode()?.size();
                let name = m.name().map(|n| n.to_string()).unwrap_or_else(|| "Unknown".to_string());
                Some(MonitorInfo { name, phys_pos, phys_size, scale: m.scale_factor() })
            })
            .collect();

        let window_attributes =
            WindowAttributes::default().with_title("Cross-Monitor Position Test");
        self.window = match event_loop.create_window(window_attributes) {
            Ok(w) => Some(w),
            Err(err) => {
                eprintln!("Error creating window: {err}");
                event_loop.exit();
                return;
            },
        };

        println!("\n=== Cross-Monitor set_outer_position Test ===\n");
        println!("Monitors:");
        for (i, m) in self.monitors.iter().enumerate() {
            println!(
                "  [{i}] \"{}\" — origin ({}, {}), {}x{} px, {:.2}x scale",
                m.name, m.phys_pos.x, m.phys_pos.y, m.phys_size.width, m.phys_size.height, m.scale,
            );
        }

        let has_mixed = self.monitors.windows(2).any(|w| (w[0].scale - w[1].scale).abs() > 0.01);
        if !has_mixed {
            println!("\n  WARNING: All monitors have the same scale factor.");
            println!("  This test needs monitors with different scales to show the bug.\n");
        }

        println!("\nKeys:");
        println!("  Space = move to other monitor (Physical — shows bug on macOS)");
        println!("  L     = move to other monitor (Logical  — shows bug on Windows)");
        println!("  Q     = quit\n");
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput {
                event: KeyEvent { logical_key, state: ElementState::Pressed, .. },
                ..
            } => match logical_key.as_ref() {
                Key::Character(" ") => self.move_cross_monitor(true),
                Key::Character("l") => self.move_cross_monitor(false),
                Key::Character("q") => event_loop.exit(),
                _ => {},
            },
            WindowEvent::SurfaceResized(_) | WindowEvent::RedrawRequested => {
                if let Some(w) = self.window.as_ref() {
                    w.pre_present_notify();
                    fill::fill_window(w.as_ref());
                }
            },
            _ => {},
        }
    }
}

impl App {
    /// Move the window to a monitor with a different scale factor and verify
    /// it lands at the correct position.
    fn move_cross_monitor(&mut self, use_physical: bool) {
        let window = match self.window.as_ref() {
            Some(w) => w,
            None => return,
        };

        let current_pos = match window.outer_position() {
            Ok(p) => p,
            Err(e) => {
                println!("  Error reading position: {e}");
                return;
            },
        };
        let current_scale = window.scale_factor();

        // Find a monitor with a different scale factor than the window's current one.
        let target = match self.monitors.iter().find(|m| (m.scale - current_scale).abs() > 0.01) {
            Some(t) => t,
            None => {
                println!("  No monitor with a different scale factor found.");
                return;
            },
        };

        self.test_count += 1;
        let mode = if use_physical { "Physical" } else { "Logical" };

        // Target: POSITION_OFFSET physical pixels inside the target monitor.
        let target_phys_x = target.phys_pos.x + POSITION_OFFSET;
        let target_phys_y = target.phys_pos.y + POSITION_OFFSET;

        println!("--- Test #{} ({mode}) ---", self.test_count);
        println!(
            "  From:     {current_scale:.2}x — Physical({}, {})",
            current_pos.x, current_pos.y,
        );
        if use_physical {
            let pos = PhysicalPosition::new(target_phys_x, target_phys_y);
            println!(
                "  To:       {:.2}x — {} using set_outer_position(Physical({}, {}))",
                target.scale, target.name, pos.x, pos.y,
            );
            window.set_outer_position(Position::Physical(pos));
        } else {
            let lx = target_phys_x as f64 / target.scale;
            let ly = target_phys_y as f64 / target.scale;
            println!(
                "  To:       {:.2}x — {} using set_outer_position(Logical({lx:.1}, {ly:.1}))",
                target.scale, target.name,
            );
            window.set_outer_position(Position::Logical(LogicalPosition::new(lx, ly)));
        }

        let actual = match window.outer_position() {
            Ok(p) => p,
            Err(e) => {
                println!("  Error reading position after move: {e}");
                return;
            },
        };

        let dx = (actual.x - target_phys_x).abs();
        let dy = (actual.y - target_phys_y).abs();
        println!("  Target:   Physical({target_phys_x}, {target_phys_y})");
        println!("  Actual:   Physical({}, {})", actual.x, actual.y);

        if dx > POSITION_TOLERANCE || dy > POSITION_TOLERANCE {
            println!("  FAIL — delta ({dx}, {dy}) px — wrong scale factor used!");
        } else {
            println!("  PASS — landed at correct position (delta {dx}, {dy})");
        }
        println!();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(App::default())?;
    Ok(())
}
