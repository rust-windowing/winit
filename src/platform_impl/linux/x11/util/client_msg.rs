use super::*;
use xcb_dl_util::format::XcbDataType;
use xcb_dl_util::void::XcbPendingCommand;

impl XConnection {
    pub fn send_event<T>(
        &self,
        target_window: ffi::xcb_window_t,
        event_mask: Option<ffi::xcb_event_mask_t>,
        event: &T,
    ) -> XcbPendingCommand {
        assert_eq!(
            mem::size_of::<T>(),
            mem::size_of::<ffi::xcb_raw_generic_event_t>()
        );
        let event_mask = event_mask.unwrap_or(ffi::XCB_EVENT_MASK_NO_EVENT);
        unsafe {
            self.xcb
                .xcb_send_event_checked(
                    self.c,
                    0,
                    target_window,
                    event_mask,
                    event as *const _ as _,
                )
                .into()
        }
    }

    pub fn send_client_msg<T: ClientMessageType>(
        &self,
        window: ffi::xcb_window_t, // The window this is "about"; not necessarily this window
        target_window: ffi::xcb_window_t, // The window we're sending to
        message_type: ffi::xcb_atom_t,
        event_mask: Option<ffi::xcb_event_mask_t>,
        data: T,
    ) -> XcbPendingCommand {
        let event = ffi::xcb_client_message_event_t {
            response_type: ffi::XCB_CLIENT_MESSAGE,
            format: T::DataType::XCB_BITS,
            type_: message_type,
            window,
            data: unsafe {
                assert_eq!(mem::size_of_val(&data), mem::size_of::<[u8; 20]>());
                ffi::xcb_client_message_data_t {
                    data8: *(&data as *const T as *const [u8; 20]),
                }
            },
            ..Default::default()
        };
        self.send_event(target_window, event_mask, &event)
    }
}

pub unsafe trait ClientMessageType {
    type DataType: XcbDataType;
}

macro_rules! imp {
    ($ty:ty, $n:expr) => {
        unsafe impl ClientMessageType for [$ty; $n] {
            type DataType = $ty;
        }
    };
}

imp!(u8, 20);
imp!(u16, 10);
imp!(u32, 5);
imp!(i8, 20);
imp!(i16, 10);
imp!(i32, 5);
