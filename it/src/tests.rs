macro_rules! test {
    ($f:ident) => {
        test!($f, crate::backend::BackendFlags::empty());
    };
    ($f:ident, $flags:expr) => {
        pub struct Test;

        impl super::Test for Test {
            fn name(&self) -> &str {
                module_path!().trim_start_matches("winit_it::tests::")
            }

            fn run<'a>(
                &'a self,
                instance: &'a dyn Instance,
            ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
                Box::pin($f(instance))
            }

            fn flags(&self) -> crate::backend::BackendFlags {
                $flags
            }
        }
    };
}

mod always_on_top;
mod available_monitors;
#[cfg(target_os = "linux")]
mod class;
mod current_monitor;
mod cursor_grab;
mod cursor_icon;
mod cursor_position;
mod cursor_visible;
mod decorations;
mod delete_window;
mod destroyed;
mod device_added;
mod device_key;
mod device_mouse;
mod dnd;
mod drag_window;
mod focused;
mod focused_multi_seat;
mod fullscreen;
mod fullscreen2;
mod icon;
mod logical_cursor_position;
mod logical_inner_size;
mod logical_size_bounds;
mod maximize;
mod minimize;
mod physical_inner_size;
mod physical_outer_position;
mod physical_size_bounds;
#[cfg(target_os = "linux")]
mod ping;
mod primary_monitor;
mod redraw_requested;
mod reset_dead_keys;
mod resizable;
mod set_position;
mod set_size;
mod title;
mod touch;
mod transparency;
mod urgency;
mod user_event;
mod visible;
mod window_keyboard;
mod window_mouse;

use crate::backend::{BackendFlags, Instance};
use std::future::Future;
use std::pin::Pin;

pub trait Test: Sync {
    fn name(&self) -> &str;
    fn run<'a>(&'a self, instance: &'a dyn Instance) -> Pin<Box<dyn Future<Output = ()> + 'a>>;

    fn flags(&self) -> BackendFlags {
        BackendFlags::empty()
    }
}

pub fn tests() -> Vec<Box<dyn Test>> {
    vec![
        //
        Box::new(window_keyboard::Test),
        Box::new(visible::Test),
        Box::new(always_on_top::Test),
        Box::new(decorations::Test),
        Box::new(physical_inner_size::Test),
        Box::new(physical_outer_position::Test),
        Box::new(title::Test),
        Box::new(maximize::Test),
        Box::new(physical_size_bounds::Test),
        Box::new(urgency::Test),
        #[cfg(target_os = "linux")]
        Box::new(class::Test),
        Box::new(delete_window::Test),
        #[cfg(target_os = "linux")]
        Box::new(ping::Test),
        Box::new(minimize::Test),
        Box::new(resizable::Test),
        Box::new(transparency::Test),
        Box::new(icon::Test),
        Box::new(set_position::Test),
        Box::new(set_size::Test),
        Box::new(device_added::Test),
        Box::new(device_key::Test),
        Box::new(reset_dead_keys::Test),
        Box::new(destroyed::Test),
        Box::new(focused::Test),
        Box::new(focused_multi_seat::Test),
        Box::new(user_event::Test),
        Box::new(available_monitors::Test),
        Box::new(primary_monitor::Test),
        Box::new(device_mouse::Test),
        Box::new(window_mouse::Test),
        Box::new(drag_window::Test),
        Box::new(dnd::Test),
        Box::new(cursor_grab::Test),
        Box::new(cursor_position::Test),
        Box::new(cursor_icon::Test),
        Box::new(cursor_visible::Test),
        Box::new(logical_inner_size::Test),
        Box::new(logical_cursor_position::Test),
        Box::new(logical_size_bounds::Test),
        Box::new(current_monitor::Test),
        Box::new(fullscreen::Test),
        Box::new(fullscreen2::Test),
        Box::new(touch::Test),
        Box::new(redraw_requested::Test),
    ]
}
