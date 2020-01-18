use super::event_loop::EventLoop;
use crate::event::device;

mod gamepad;

pub(crate) use gamepad::*;

#[derive(Debug)]
pub(crate) enum DeviceId {
    Mouse(MouseId),
    Keyboard(KeyboardId),
    Hid(HidId),
    Gamepad(GamepadHandle, Gamepad),
}

macro_rules! device_id {
  ($name:ident, $enumerate:ident) => {
      #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
      pub(crate) struct $name;

      unsafe impl Send for $name {}
      unsafe impl Sync for $name {}

      impl $name {
          pub unsafe fn dummy() -> Self {
              Self {}
          }

          pub fn persistent_identifier(&self) -> Option<String> {
              // raw_input::get_raw_input_device_name(self.0)
              None
          }

          pub fn is_connected(&self) -> bool {
              // raw_input::get_raw_input_device_info(self.0).is_some()
              false
          }

          pub fn enumerate<'a, T>(
              event_loop: &'a EventLoop<T>,
          ) -> impl 'a + Iterator<Item = device::$name> {
              event_loop.$enumerate()
          }
      }

      impl From<$name> for device::$name {
          fn from(platform_id: $name) -> Self {
              Self(platform_id)
          }
      }
  };
}

device_id!(MouseId, mouses);
device_id!(KeyboardId, keyboards);
device_id!(HidId, hids);