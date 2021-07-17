use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use std::f32::consts::PI;
use std::thread;
use std::time::Duration;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
};

/// Return the a location at the lower left corner of the window where a left click and cursor
/// movement would result in resizing the window. (If the window is resizable)
pub fn lower_left_resize_pos(
    window_inner_pos: PhysicalPosition<i32>,
    window_inner_size: PhysicalSize<u32>
) -> PhysicalPosition<i32> {
    lower_left_resize_pos_impl(window_inner_pos, window_inner_size)
}

#[cfg(target_os = "windows")]
fn lower_left_resize_pos_impl(
    window_inner_pos: PhysicalPosition<i32>,
    window_inner_size: PhysicalSize<u32>
) -> PhysicalPosition<i32> {
    PhysicalPosition::<i32>::new(
        window_inner_pos.x + window_inner_size.width as i32 + 2,
        window_inner_pos.y + window_inner_size.height as i32 + 2,
    )
}

/// Return the cursor position where a left click and cursor movement would result in moving the
/// window.
pub fn window_drag_location(window_inner_pos: PhysicalPosition<i32>) -> PhysicalPosition<i32> {
    window_drag_location_impl(window_inner_pos)
}

#[cfg(target_os = "windows")]
fn window_drag_location_impl(window_inner_pos: PhysicalPosition<i32>) -> PhysicalPosition<i32> {
    PhysicalPosition::<i32>::new(window_inner_pos.x + 2, window_inner_pos.y - 2)
}

pub fn move_slowly_to(
    enigo: &mut Enigo,
    pos_x: f32,
    pos_y: f32,
    target_x: f32,
    target_y: f32,
    duration: Duration,
) {
    let step_delay = Duration::from_millis(1);
    let steps = (duration.as_secs_f32() / step_delay.as_secs_f32()) as i32;
    for i in 0..steps {
        thread::sleep(step_delay);
        let ratio = (i + 1) as f32 / steps as f32;
        enigo.mouse_move_to(
            ((1.0 - ratio) * pos_x + ratio * target_x) as i32,
            ((1.0 - ratio) * pos_y + ratio * target_y) as i32,
        );
    }
}


