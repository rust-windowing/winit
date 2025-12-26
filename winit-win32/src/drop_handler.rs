use crate::definitions::{
    IDataObject, IDataObjectVtbl, IDropTarget, IDropTargetVtbl, IUnknown, IUnknownVtbl,
};
use dpi::PhysicalPosition;
use std::ffi::{OsString, c_void};
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::ptr::{self, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering};
use tracing::debug;
use windows_sys::Win32::Foundation::{DV_E_FORMATETC, E_NOINTERFACE, HWND, POINT, POINTL, S_OK};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL};
use windows_sys::Win32::System::Ole::{CF_HDROP, DROPEFFECT_COPY, DROPEFFECT_NONE};
use windows_sys::Win32::UI::Shell::{DragFinish, DragQueryFileW, HDROP};
use windows_sys::core::{GUID, HRESULT};
use winit_core::event::WindowEvent;

const IID_IUNKNOWN: GUID = GUID {
    data1: 0x00000000,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

const IID_IDROP_TARGET: GUID = GUID {
    data1: 0x00000122,
    data2: 0x0000,
    data3: 0x0000,
    data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
};

fn guid_eq(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

#[repr(C)]
pub struct FileDropHandlerData {
    pub interface: IDropTarget,
    refcount: AtomicUsize,
    window: HWND,
    send_event: Box<dyn Fn(WindowEvent)>,
    cursor_effect: u32,
    valid: bool, /* If the currently hovered item is not valid there must not be any
                  * `DragLeft` emitted */
}

pub struct FileDropHandler {
    pub data: *mut FileDropHandlerData,
}

#[allow(non_snake_case)]
impl FileDropHandler {
    pub(crate) fn new(window: HWND, send_event: Box<dyn Fn(WindowEvent)>) -> FileDropHandler {
        let data = Box::new(FileDropHandlerData {
            interface: IDropTarget { lpVtbl: &DROP_TARGET_VTBL as *const IDropTargetVtbl },
            refcount: AtomicUsize::new(1),
            window,
            send_event,
            cursor_effect: DROPEFFECT_NONE,
            valid: false,
        });
        FileDropHandler { data: Box::into_raw(data) }
    }

    // Implement IUnknown
    pub unsafe extern "system" fn QueryInterface(
        this: *mut IUnknown,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT {
        if riid.is_null() || ppvObject.is_null() {
            return E_NOINTERFACE;
        }

        let drop_handler_data = unsafe { Self::from_interface(this) };
        let requested = unsafe { &*riid };

        if guid_eq(requested, &IID_IUNKNOWN) || guid_eq(requested, &IID_IDROP_TARGET) {
            unsafe { *ppvObject = this as *mut c_void };
            drop_handler_data.refcount.fetch_add(1, Ordering::Release);
            return S_OK;
        }

        // Interface not supported
        unsafe { *ppvObject = null_mut() };
        E_NOINTERFACE
    }

    pub unsafe extern "system" fn AddRef(this: *mut IUnknown) -> u32 {
        let drop_handler_data = unsafe { Self::from_interface(this) };
        let count = drop_handler_data.refcount.fetch_add(1, Ordering::Release) + 1;
        count as u32
    }

    pub unsafe extern "system" fn Release(this: *mut IUnknown) -> u32 {
        let drop_handler = unsafe { Self::from_interface(this) };
        let count = drop_handler.refcount.fetch_sub(1, Ordering::Release) - 1;
        if count == 0 {
            // Destroy the underlying data
            drop(unsafe { Box::from_raw(drop_handler as *mut FileDropHandlerData) });
        }
        count as u32
    }

    pub unsafe extern "system" fn DragEnter(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        _grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT {
        let drop_handler = unsafe { Self::from_interface(this) };
        let mut pt = POINT { x: pt.x, y: pt.y };
        unsafe {
            ScreenToClient(drop_handler.window, &mut pt);
        }
        let position = PhysicalPosition::new(pt.x as f64, pt.y as f64);
        let mut paths = Vec::new();
        let hdrop = unsafe { Self::iterate_filenames(pDataObj, |path| paths.push(path)) };
        drop_handler.valid = hdrop.is_some();
        if drop_handler.valid {
            (drop_handler.send_event)(WindowEvent::DragEntered { paths, position });
        }
        drop_handler.cursor_effect =
            if drop_handler.valid { DROPEFFECT_COPY } else { DROPEFFECT_NONE };
        unsafe {
            *pdwEffect = drop_handler.cursor_effect;
        }

        S_OK
    }

    pub unsafe extern "system" fn DragOver(
        this: *mut IDropTarget,
        _grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT {
        let drop_handler = unsafe { Self::from_interface(this) };
        if drop_handler.valid {
            let mut pt = POINT { x: pt.x, y: pt.y };
            unsafe {
                ScreenToClient(drop_handler.window, &mut pt);
            }
            let position = PhysicalPosition::new(pt.x as f64, pt.y as f64);
            (drop_handler.send_event)(WindowEvent::DragMoved { position });
        }
        unsafe {
            *pdwEffect = drop_handler.cursor_effect;
        }

        S_OK
    }

    pub unsafe extern "system" fn DragLeave(this: *mut IDropTarget) -> HRESULT {
        let drop_handler = unsafe { Self::from_interface(this) };
        if drop_handler.valid {
            (drop_handler.send_event)(WindowEvent::DragLeft { position: None });
        }

        S_OK
    }

    pub unsafe extern "system" fn Drop(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        _grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT {
        let drop_handler = unsafe { Self::from_interface(this) };
        if drop_handler.valid {
            let mut pt = POINT { x: pt.x, y: pt.y };
            unsafe {
                ScreenToClient(drop_handler.window, &mut pt);
            }
            let position = PhysicalPosition::new(pt.x as f64, pt.y as f64);
            let mut paths = Vec::new();
            let hdrop = unsafe { Self::iterate_filenames(pDataObj, |path| paths.push(path)) };
            (drop_handler.send_event)(WindowEvent::DragDropped { paths, position });
            if let Some(hdrop) = hdrop {
                unsafe {
                    DragFinish(hdrop);
                }
            }
        }
        unsafe {
            *pdwEffect = drop_handler.cursor_effect;
        }

        S_OK
    }

    unsafe fn from_interface<'a, InterfaceT>(this: *mut InterfaceT) -> &'a mut FileDropHandlerData {
        unsafe { &mut *(this as *mut _) }
    }

    unsafe fn iterate_filenames<F>(data_obj: *const IDataObject, mut callback: F) -> Option<HDROP>
    where
        F: FnMut(PathBuf),
    {
        let drop_format = FORMATETC {
            cfFormat: CF_HDROP,
            ptd: ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL as u32,
        };

        let mut medium = unsafe { std::mem::zeroed() };
        let get_data_fn = unsafe { (*(*data_obj).cast::<IDataObjectVtbl>()).GetData };
        let get_data_result = unsafe { get_data_fn(data_obj as *mut _, &drop_format, &mut medium) };
        if get_data_result >= 0 {
            let hdrop = unsafe { medium.u.hGlobal as HDROP };

            // The second parameter (0xFFFFFFFF) instructs the function to return the item count
            let item_count = unsafe { DragQueryFileW(hdrop, 0xffffffff, ptr::null_mut(), 0) };

            for i in 0..item_count {
                // Get the length of the path string NOT including the terminating null character.
                // Previously, this was using a fixed size array of MAX_PATH length, but the
                // Windows API allows longer paths under certain circumstances.
                let character_count =
                    unsafe { DragQueryFileW(hdrop, i, ptr::null_mut(), 0) as usize };
                let str_len = character_count + 1;

                // Fill path_buf with the null-terminated file name
                let mut path_buf = Vec::with_capacity(str_len);
                unsafe {
                    DragQueryFileW(hdrop, i, path_buf.as_mut_ptr(), str_len as u32);
                    path_buf.set_len(str_len);
                }

                callback(OsString::from_wide(&path_buf[0..character_count]).into());
            }

            Some(hdrop)
        } else if get_data_result == DV_E_FORMATETC {
            // If the dropped item is not a file this error will occur.
            // In this case it is OK to return without taking further action.
            debug!("Error occurred while processing dropped/hovered item: item is not a file.");
            None
        } else {
            debug!("Unexpected error occurred while processing dropped/hovered item.");
            None
        }
    }
}

impl Drop for FileDropHandler {
    fn drop(&mut self) {
        unsafe {
            FileDropHandler::Release(self.data as *mut IUnknown);
        }
    }
}

static DROP_TARGET_VTBL: IDropTargetVtbl = IDropTargetVtbl {
    parent: IUnknownVtbl {
        QueryInterface: FileDropHandler::QueryInterface,
        AddRef: FileDropHandler::AddRef,
        Release: FileDropHandler::Release,
    },
    DragEnter: FileDropHandler::DragEnter,
    DragOver: FileDropHandler::DragOver,
    DragLeave: FileDropHandler::DragLeave,
    Drop: FileDropHandler::Drop,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::core::GUID;

    #[test]
    fn test_file_drop_handler_query_interface() {
        let handler = FileDropHandler::new(
            0 as HWND, // null window handle
            Box::new(|event| {
                println!("WindowEvent: {:?}", event);
            }),
        );

        unsafe {
            let mut ppv: *mut std::ffi::c_void = null_mut();
            let hr_iunknown = FileDropHandler::QueryInterface(
                handler.data as *mut IUnknown,
                &IID_IUNKNOWN,
                &mut ppv as *mut _,
            );
            assert_eq!(hr_iunknown, S_OK);
            assert!(!ppv.is_null());

            ppv = null_mut();

            let hr_idroptarget = FileDropHandler::QueryInterface(
                handler.data as *mut IUnknown,
                &IID_IDROP_TARGET,
                &mut ppv as *mut _,
            );
            assert_eq!(hr_idroptarget, S_OK);
            assert!(!ppv.is_null());

            let unknown_guid = GUID {
                data1: 0x12345678,
                data2: 0x1234,
                data3: 0x5678,
                data4: [0x90, 0xAB, 0xCD, 0xEF, 0x00, 0x11, 0x22, 0x33],
            };
            ppv = null_mut();
            let hr_unknown = FileDropHandler::QueryInterface(
                handler.data as *mut IUnknown,
                &unknown_guid,
                &mut ppv as *mut _,
            );
            assert_eq!(hr_unknown, E_NOINTERFACE);
            assert!(ppv.is_null());
        }

        // Drop the handler manually
        unsafe { FileDropHandler::Release(handler.data as *mut IUnknown) };
    }
}