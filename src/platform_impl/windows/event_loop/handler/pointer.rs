//! Handlers for pointer methods.

use super::super::{
    GET_POINTER_DEVICE_RECTS, GET_POINTER_FRAME_INFO_HISTORY, GET_POINTER_PEN_INFO,
    GET_POINTER_TOUCH_INFO, SKIP_POINTER_FRAME_MESSAGES,
};
use super::prelude::*;

use std::mem;
use std::ptr;

fn handle_touch(
    window: HWND,
    _: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    unsafe {
        let pcount = loword(wparam as u32) as usize;
        let mut inputs = Vec::with_capacity(pcount);
        let htouch = lparam;
        if GetTouchInputInfo(
            htouch,
            pcount as u32,
            inputs.as_mut_ptr(),
            mem::size_of::<TOUCHINPUT>() as i32,
        ) > 0
        {
            inputs.set_len(pcount);
            for input in &inputs {
                let mut location = POINT {
                    x: input.x / 100,
                    y: input.y / 100,
                };

                if ScreenToClient(window, &mut location) == false.into() {
                    continue;
                }

                let x = location.x as f64 + (input.x % 100) as f64 / 100f64;
                let y = location.y as f64 + (input.y % 100) as f64 / 100f64;
                let location = PhysicalPosition::new(x, y);
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::Touch(Touch {
                        phase: if util::has_flag(input.dwFlags, TOUCHEVENTF_DOWN) {
                            TouchPhase::Started
                        } else if util::has_flag(input.dwFlags, TOUCHEVENTF_UP) {
                            TouchPhase::Ended
                        } else if util::has_flag(input.dwFlags, TOUCHEVENTF_MOVE) {
                            TouchPhase::Moved
                        } else {
                            continue;
                        },
                        location,
                        force: None, // WM_TOUCH doesn't support pressure information
                        id: input.dwID as u64,
                        device_id: DEVICE_ID,
                    }),
                });
            }
        }
        CloseTouchInputHandle(htouch);
        0
    }
}

fn handle_pointer_change(
    window: HWND,
    _msg: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
    userdata: &dyn GenericWindowData,
) -> LRESULT {
    unsafe {
        if let (
            Some(GetPointerFrameInfoHistory),
            Some(SkipPointerFrameMessages),
            Some(GetPointerDeviceRects),
        ) = (
            *GET_POINTER_FRAME_INFO_HISTORY,
            *SKIP_POINTER_FRAME_MESSAGES,
            *GET_POINTER_DEVICE_RECTS,
        ) {
            let pointer_id = loword(wparam as u32) as u32;
            let mut entries_count = 0u32;
            let mut pointers_count = 0u32;
            if GetPointerFrameInfoHistory(
                pointer_id,
                &mut entries_count,
                &mut pointers_count,
                ptr::null_mut(),
            ) == false.into()
            {
                return 0;
            }

            let pointer_info_count = (entries_count * pointers_count) as usize;
            let mut pointer_infos = Vec::with_capacity(pointer_info_count);
            if GetPointerFrameInfoHistory(
                pointer_id,
                &mut entries_count,
                &mut pointers_count,
                pointer_infos.as_mut_ptr(),
            ) == false.into()
            {
                return 0;
            }
            pointer_infos.set_len(pointer_info_count);

            // https://docs.microsoft.com/en-us/windows/desktop/api/winuser/nf-winuser-getpointerframeinfohistory
            // The information retrieved appears in reverse chronological order, with the most recent entry in the first
            // row of the returned array
            for pointer_info in pointer_infos.iter().rev() {
                let mut device_rect = mem::MaybeUninit::uninit();
                let mut display_rect = mem::MaybeUninit::uninit();

                if GetPointerDeviceRects(
                    pointer_info.sourceDevice,
                    device_rect.as_mut_ptr(),
                    display_rect.as_mut_ptr(),
                ) == false.into()
                {
                    continue;
                }

                let device_rect = device_rect.assume_init();
                let display_rect = display_rect.assume_init();

                // For the most precise himetric to pixel conversion we calculate the ratio between the resolution
                // of the display device (pixel) and the touch device (himetric).
                let himetric_to_pixel_ratio_x = (display_rect.right - display_rect.left) as f64
                    / (device_rect.right - device_rect.left) as f64;
                let himetric_to_pixel_ratio_y = (display_rect.bottom - display_rect.top) as f64
                    / (device_rect.bottom - device_rect.top) as f64;

                // ptHimetricLocation's origin is 0,0 even on multi-monitor setups.
                // On multi-monitor setups we need to translate the himetric location to the rect of the
                // display device it's attached to.
                let x = display_rect.left as f64
                    + pointer_info.ptHimetricLocation.x as f64 * himetric_to_pixel_ratio_x;
                let y = display_rect.top as f64
                    + pointer_info.ptHimetricLocation.y as f64 * himetric_to_pixel_ratio_y;

                let mut location = POINT {
                    x: x.floor() as i32,
                    y: y.floor() as i32,
                };

                if ScreenToClient(window, &mut location) == false.into() {
                    continue;
                }

                let force = match pointer_info.pointerType {
                    PT_TOUCH => {
                        let mut touch_info = mem::MaybeUninit::uninit();
                        GET_POINTER_TOUCH_INFO.and_then(|GetPointerTouchInfo| {
                            match GetPointerTouchInfo(
                                pointer_info.pointerId,
                                touch_info.as_mut_ptr(),
                            ) {
                                0 => None,
                                _ => normalize_pointer_pressure(touch_info.assume_init().pressure),
                            }
                        })
                    }
                    PT_PEN => {
                        let mut pen_info = mem::MaybeUninit::uninit();
                        GET_POINTER_PEN_INFO.and_then(|GetPointerPenInfo| {
                            match GetPointerPenInfo(pointer_info.pointerId, pen_info.as_mut_ptr()) {
                                0 => None,
                                _ => normalize_pointer_pressure(pen_info.assume_init().pressure),
                            }
                        })
                    }
                    _ => None,
                };

                let x = location.x as f64 + x.fract();
                let y = location.y as f64 + y.fract();
                let location = PhysicalPosition::new(x, y);
                userdata.send_event(Event::WindowEvent {
                    window_id: RootWindowId(WindowId(window)),
                    event: WindowEvent::Touch(Touch {
                        phase: if util::has_flag(pointer_info.pointerFlags, POINTER_FLAG_DOWN) {
                            TouchPhase::Started
                        } else if util::has_flag(pointer_info.pointerFlags, POINTER_FLAG_UP) {
                            TouchPhase::Ended
                        } else if util::has_flag(pointer_info.pointerFlags, POINTER_FLAG_UPDATE) {
                            TouchPhase::Moved
                        } else {
                            continue;
                        },
                        location,
                        force,
                        id: pointer_info.pointerId as u64,
                        device_id: DEVICE_ID,
                    }),
                });
            }

            SkipPointerFrameMessages(pointer_id);
        }
        0
    }
}

submit! {
    (WM_TOUCH, handle_touch),
    (WM_POINTERDOWN, handle_pointer_change),
    (WM_POINTERUPDATE, handle_pointer_change),
    (WM_POINTERUP, handle_pointer_change),
}
