use crate::event::{
    DeviceButton, DeviceEvent, DeviceEventExt, DeviceMouseMotion, DeviceMouseWheel, Event,
    UserEvent, WindowCursorEntered, WindowCursorLeft, WindowCursorMoved, WindowEvent,
    WindowEventExt, WindowKeyboardInput, WindowMouseInput, WindowMouseWheel,
    WindowScaleFactorChanged,
};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{RawKeyEvent, Touch};
use winit::keyboard::ModifiersState;
use winit::window::WindowId;

pub trait EventStream {
    fn event<'a>(&'a mut self) -> Pin<Box<dyn Future<Output = Event> + 'a>>;
    fn has_more(&self) -> bool {
        true
    }
}

impl<'a> dyn EventStream + 'a {
    pub async fn user_event(&mut self) -> UserEvent {
        loop {
            if let Event::UserEvent(ue) = self.event().await {
                return ue;
            }
        }
    }

    pub async fn window_event(&mut self) -> WindowEventExt {
        loop {
            if let Event::WindowEvent(we) = self.event().await {
                return we;
            }
        }
    }

    pub async fn device_event(&mut self) -> DeviceEventExt {
        loop {
            if let Event::DeviceEvent(we) = self.event().await {
                return we;
            }
        }
    }

    pub async fn redraw_requested_event(&mut self) -> WindowId {
        log::debug!("Awaiting window redraw_requested");
        loop {
            if let Event::RedrawRequested(we) = self.event().await {
                return we;
            }
        }
    }

    pub async fn device_added_event(&mut self) -> DeviceEventExt {
        log::info!("Waiting for device added event");
        loop {
            let de = self.device_event().await;
            if de.event == DeviceEvent::Added {
                return de;
            }
        }
    }

    pub async fn device_removed_event(&mut self) -> DeviceEventExt {
        log::info!("Waiting for device removed event");
        loop {
            let de = self.device_event().await;
            if de.event == DeviceEvent::Removed {
                return de;
            }
        }
    }

    pub async fn device_mouse_motion_event(&mut self) -> (DeviceEventExt, DeviceMouseMotion) {
        log::info!("Waiting for device move event");
        loop {
            let de = self.device_event().await;
            if let DeviceEvent::MouseMotion(dm) = &de.event {
                log::debug!("Got mouse motion event {:?}", dm);
                return (de.clone(), dm.clone());
            }
        }
    }

    pub async fn device_mouse_wheel_event(&mut self) -> (DeviceEventExt, DeviceMouseWheel) {
        log::info!("Waiting for device wheel event");
        loop {
            let de = self.device_event().await;
            if let DeviceEvent::MouseWheel(dm) = &de.event {
                log::debug!("Got mouse wheel event {:?}", dm);
                return (de.clone(), dm.clone());
            }
        }
    }

    pub async fn device_key_event(&mut self) -> (DeviceEventExt, RawKeyEvent) {
        log::info!("Waiting for device key event");
        loop {
            let de = self.device_event().await;
            if let DeviceEvent::Key(e) = de.event {
                log::debug!("Got key event {:?}", e);
                return (de, e);
            }
        }
    }

    pub async fn device_button_event(&mut self) -> (DeviceEventExt, DeviceButton) {
        log::info!("Waiting for device button event");
        loop {
            let de = self.device_event().await;
            if let DeviceEvent::Button(e) = &de.event {
                log::debug!("Got button event {:?}", e);
                return (de.clone(), e.clone());
            }
        }
    }

    pub async fn window_destroyed_event(&mut self) -> WindowEventExt {
        log::debug!("Awaiting window destroyed");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::Destroyed = &we.event {
                log::debug!("Got window destroyed");
                return we.clone();
            };
        }
    }

    pub async fn window_hovered_file(&mut self) -> (WindowEventExt, PathBuf) {
        log::debug!("Awaiting hovered file");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::HoveredFile(mi) = &we.event {
                log::debug!("Got hovered file: {:?}", mi);
                return (we.clone(), mi.clone());
            };
        }
    }

    pub async fn window_hovered_file_canceled(&mut self) -> WindowEventExt {
        log::debug!("Awaiting hovered file cancelled");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::HoveredFileCancelled = &we.event {
                log::debug!("Got hovered file cancelled");
                return we.clone();
            };
        }
    }

    pub async fn window_dropped_file(&mut self) -> (WindowEventExt, PathBuf) {
        log::debug!("Awaiting dropped file");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::DroppedFile(wi) = &we.event {
                log::debug!("Got dropped file: {:?}", wi);
                return (we.clone(), wi.clone());
            };
        }
    }

    pub async fn window_mouse_input_event(&mut self) -> (WindowEventExt, WindowMouseInput) {
        log::debug!("Awaiting mouse input");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::MouseInput(mi) = &we.event {
                log::debug!("Got mouse input: {:?}", mi);
                return (we.clone(), mi.clone());
            };
        }
    }

    pub async fn window_cursor_left(&mut self) -> (WindowEventExt, WindowCursorLeft) {
        log::debug!("Awaiting cursor left");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::CursorLeft(cl) = &we.event {
                log::debug!("Got cursor left: {:?}", cl);
                return (we.clone(), cl.clone());
            };
        }
    }

    pub async fn window_scale_factor_changed(
        &mut self,
    ) -> (WindowEventExt, WindowScaleFactorChanged) {
        log::debug!("Awaiting scale factor changed");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::ScaleFactorChanged(cl) = &we.event {
                log::debug!("Got scale factor changed: {:?}", cl);
                return (we.clone(), cl.clone());
            };
        }
    }

    pub async fn window_cursor_entered(&mut self) -> (WindowEventExt, WindowCursorEntered) {
        log::debug!("Awaiting cursor entered");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::CursorEntered(cl) = &we.event {
                log::debug!("Got cursor entered: {:?}", cl);
                return (we.clone(), cl.clone());
            };
        }
    }

    pub async fn window_cursor_moved(&mut self) -> (WindowEventExt, WindowCursorMoved) {
        log::debug!("Awaiting cursor moved");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::CursorMoved(cl) = &we.event {
                log::debug!("Got cursor moved: {:?}", cl);
                return (we.clone(), cl.clone());
            };
        }
    }

    pub async fn window_mouse_wheel(&mut self) -> (WindowEventExt, WindowMouseWheel) {
        log::debug!("Awaiting mouse wheel");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::MouseWheel(cl) = &we.event {
                log::debug!("Got mouse wheel: {:?}", cl);
                return (we.clone(), cl.clone());
            };
        }
    }

    pub async fn window_focus_event(&mut self) -> (WindowEventExt, bool) {
        log::debug!("Awaiting window focus");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::Focused(v) = &we.event {
                log::debug!("Got window focus {}", v);
                return (we.clone(), *v);
            };
        }
    }

    pub async fn window_move_event(&mut self) -> (WindowEventExt, PhysicalPosition<i32>) {
        log::debug!("Awaiting window move");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::Moved(pos) = &we.event {
                log::debug!("Got window move: {:?}", pos);
                return (we.clone(), pos.clone());
            };
        }
    }

    pub async fn window_touch_event(&mut self) -> (WindowEventExt, Touch) {
        log::debug!("Awaiting window touch");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::Touch(touch) = &we.event {
                log::debug!("Got window touch: {:?}", touch);
                return (we.clone(), touch.clone());
            };
        }
    }

    pub async fn window_resize_event(&mut self) -> (WindowEventExt, PhysicalSize<u32>) {
        log::debug!("Awaiting window resize");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::Resized(pos) = &we.event {
                log::debug!("Got window resize");
                return (we.clone(), pos.clone());
            };
        }
    }

    pub async fn window_close_requested(&mut self) -> WindowEventExt {
        log::debug!("Awaiting window delete");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::CloseRequested = &we.event {
                log::debug!("Got close requested");
                return we;
            };
        }
    }

    pub async fn window_keyboard_input(&mut self) -> (WindowEventExt, WindowKeyboardInput) {
        log::debug!("Awaiting keyboard input");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::KeyboardInput(ki) = &we.event {
                log::debug!("Got keyboard input {:?}", ki);
                let ki = ki.clone();
                return (we, ki);
            }
        }
    }

    pub async fn window_modifiers(&mut self) -> (WindowEventExt, ModifiersState) {
        log::debug!("Awaiting window modifiers");
        loop {
            let we = self.window_event().await;
            if let WindowEvent::ModifiersChanged(ki) = &we.event {
                log::debug!("Got window modifiers {:?}", ki);
                let ki = ki.clone();
                return (we, ki);
            }
        }
    }
}
