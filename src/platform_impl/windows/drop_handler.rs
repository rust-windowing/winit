use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use winapi::{
    ctypes::c_void,
    shared::{
        guiddef::REFIID,
        minwindef::{DWORD, UINT, ULONG},
        windef::{HWND, POINTL},
        winerror::S_OK,
    },
    um::{
        objidl::IDataObject,
        oleidl::{IDropTarget, IDropTargetVtbl, DROPEFFECT_COPY, DROPEFFECT_NONE},
        shellapi, unknwnbase,
        winnt::HRESULT,
    },
};

use crate::platform_impl::platform::WindowId;

use crate::{event::Event, window::WindowId as SuperWindowId};

#[repr(C)]
pub struct FileDropHandlerData {
    pub interface: IDropTarget,
    refcount: AtomicUsize,
    window: HWND,
    send_event: Box<dyn Fn(Event<'static, ()>)>,
    cursor_effect: DWORD,
    hovered_is_valid: bool, /* If the currently hovered item is not valid there must not be any `HoveredFileCancelled` emitted */
}

pub struct FileDropHandler {
    pub data: *mut FileDropHandlerData,
}

#[allow(non_snake_case)]
impl FileDropHandler {
    pub fn new(window: HWND, send_event: Box<dyn Fn(Event<'static, ()>)>) -> FileDropHandler {
        let data = Box::new(FileDropHandlerData {
            interface: IDropTarget {
                lpVtbl: &DROP_TARGET_VTBL as *const IDropTargetVtbl,
            },
            refcount: AtomicUsize::new(1),
            window,
            send_event,
            cursor_effect: DROPEFFECT_NONE,
            hovered_is_valid: false,
        });
        FileDropHandler {
            data: Box::into_raw(data),
        }
    }

    // Implement IUnknown
    pub unsafe extern "system" fn QueryInterface(
        _this: *mut unknwnbase::IUnknown,
        _riid: REFIID,
        _ppvObject: *mut *mut c_void,
    ) -> HRESULT {
        // This function doesn't appear to be required for an `IDropTarget`.
        // An implementation would be nice however.
        unimplemented!();
    }

    pub unsafe extern "system" fn AddRef(this: *mut unknwnbase::IUnknown) -> ULONG {
        let drop_handler_data = Self::from_interface(this);
        let count = drop_handler_data.refcount.fetch_add(1, Ordering::Release) + 1;
        count as ULONG
    }

    pub unsafe extern "system" fn Release(this: *mut unknwnbase::IUnknown) -> ULONG {
        let drop_handler = Self::from_interface(this);
        let count = drop_handler.refcount.fetch_sub(1, Ordering::Release) - 1;
        if count == 0 {
            // Destroy the underlying data
            Box::from_raw(drop_handler as *mut FileDropHandlerData);
        }
        count as ULONG
    }

    pub unsafe extern "system" fn DragEnter(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        _grfKeyState: DWORD,
        _pt: *const POINTL,
        pdwEffect: *mut DWORD,
    ) -> HRESULT {
        use crate::event::WindowEvent::HoveredFile;
        let drop_handler = Self::from_interface(this);
        let hdrop = Self::iterate_filenames(pDataObj, |filename| {
            drop_handler.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(drop_handler.window)),
                event: HoveredFile(filename),
            });
        });
        drop_handler.hovered_is_valid = hdrop.is_some();
        drop_handler.cursor_effect = if drop_handler.hovered_is_valid {
            DROPEFFECT_COPY
        } else {
            DROPEFFECT_NONE
        };
        *pdwEffect = drop_handler.cursor_effect;

        S_OK
    }

    pub unsafe extern "system" fn DragOver(
        this: *mut IDropTarget,
        _grfKeyState: DWORD,
        _pt: *const POINTL,
        pdwEffect: *mut DWORD,
    ) -> HRESULT {
        let drop_handler = Self::from_interface(this);
        *pdwEffect = drop_handler.cursor_effect;

        S_OK
    }

    pub unsafe extern "system" fn DragLeave(this: *mut IDropTarget) -> HRESULT {
        use crate::event::WindowEvent::HoveredFileCancelled;
        let drop_handler = Self::from_interface(this);
        if drop_handler.hovered_is_valid {
            drop_handler.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(drop_handler.window)),
                event: HoveredFileCancelled,
            });
        }

        S_OK
    }

    pub unsafe extern "system" fn Drop(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        _grfKeyState: DWORD,
        _pt: *const POINTL,
        _pdwEffect: *mut DWORD,
    ) -> HRESULT {
        use crate::event::WindowEvent::DroppedFile;
        let drop_handler = Self::from_interface(this);
        let hdrop = Self::iterate_filenames(pDataObj, |filename| {
            drop_handler.send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(drop_handler.window)),
                event: DroppedFile(filename),
            });
        });
        if let Some(hdrop) = hdrop {
            shellapi::DragFinish(hdrop);
        }

        S_OK
    }

    unsafe fn from_interface<'a, InterfaceT>(this: *mut InterfaceT) -> &'a mut FileDropHandlerData {
        &mut *(this as *mut _)
    }

    unsafe fn iterate_filenames<F>(
        data_obj: *const IDataObject,
        callback: F,
    ) -> Option<shellapi::HDROP>
    where
        F: Fn(PathBuf),
    {
        use winapi::{
            shared::{
                winerror::{DV_E_FORMATETC, SUCCEEDED},
                wtypes::{CLIPFORMAT, DVASPECT_CONTENT},
            },
            um::{
                objidl::{FORMATETC, TYMED_HGLOBAL},
                shellapi::DragQueryFileW,
                winuser::CF_HDROP,
            },
        };

        let mut drop_format = FORMATETC {
            cfFormat: CF_HDROP as CLIPFORMAT,
            ptd: ptr::null(),
            dwAspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL,
        };

        let mut medium = std::mem::zeroed();
        let get_data_result = (*data_obj).GetData(&mut drop_format, &mut medium);
        if SUCCEEDED(get_data_result) {
            let hglobal = (*medium.u).hGlobal();
            let hdrop = (*hglobal) as shellapi::HDROP;

            // The second parameter (0xFFFFFFFF) instructs the function to return the item count
            let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, ptr::null_mut(), 0);

            for i in 0..item_count {
                // Get the length of the path string NOT including the terminating null character.
                // Previously, this was using a fixed size array of MAX_PATH length, but the
                // Windows API allows longer paths under certain circumstances.
                let character_count = DragQueryFileW(hdrop, i, ptr::null_mut(), 0) as usize;
                let str_len = character_count + 1;

                // Fill path_buf with the null-terminated file name
                let mut path_buf = Vec::with_capacity(str_len);
                DragQueryFileW(hdrop, i, path_buf.as_mut_ptr(), str_len as UINT);
                path_buf.set_len(str_len);

                callback(OsString::from_wide(&path_buf[0..character_count]).into());
            }

            return Some(hdrop);
        } else if get_data_result == DV_E_FORMATETC {
            // If the dropped item is not a file this error will occur.
            // In this case it is OK to return without taking further action.
            debug!("Error occured while processing dropped/hovered item: item is not a file.");
            return None;
        } else {
            debug!("Unexpected error occured while processing dropped/hovered item.");
            return None;
        }
    }
}

impl FileDropHandlerData {
    fn send_event(&self, event: Event<'static, ()>) {
        (self.send_event)(event);
    }
}

impl Drop for FileDropHandler {
    fn drop(&mut self) {
        unsafe {
            FileDropHandler::Release(self.data as *mut unknwnbase::IUnknown);
        }
    }
}

static DROP_TARGET_VTBL: IDropTargetVtbl = IDropTargetVtbl {
    parent: unknwnbase::IUnknownVtbl {
        QueryInterface: FileDropHandler::QueryInterface,
        AddRef: FileDropHandler::AddRef,
        Release: FileDropHandler::Release,
    },
    DragEnter: FileDropHandler::DragEnter,
    DragOver: FileDropHandler::DragOver,
    DragLeave: FileDropHandler::DragLeave,
    Drop: FileDropHandler::Drop,
};
