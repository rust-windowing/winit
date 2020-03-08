use enigo::{Enigo, Key, KeyboardControllable, MouseButton, MouseControllable};
use std::f32::consts::PI;
use std::thread;
use std::time::Duration;

pub fn move_in_circle(enigo: &mut Enigo, pos_x: i32, pos_y: i32, r: f32) {
    let mut angle = 0.0;
    while angle < 2.0 * PI {
        thread::sleep(Duration::from_millis(1));
        let x = (angle.cos() * r) as i32;
        let y = (angle.sin() * r) as i32;
        enigo.mouse_move_to(pos_x + x, pos_y + y);
        angle += 0.02;
    }
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
