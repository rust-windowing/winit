use std::{
    ffi::{c_void, OsString},
    os::windows::ffi::OsStringExt,
    path::PathBuf,
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use windows_sys::{
    core::{IUnknown, GUID, HRESULT},
    Win32::{
        Foundation::{DV_E_FORMATETC, HWND, POINTL, S_OK},
        System::{
            Com::{IDataObject, DVASPECT_CONTENT, FORMATETC, TYMED_HGLOBAL},
            Ole::{CF_HDROP, DROPEFFECT_COPY, DROPEFFECT_NONE},
        },
        UI::Shell::{DragFinish, DragQueryFileW, HDROP},
    },
};

use crate::platform_impl::platform::{
    definitions::{IDataObjectVtbl, IDropTarget, IDropTargetVtbl, IUnknownVtbl},
    WindowId,
};

use crate::{event::Event, window::WindowId as RootWindowId};

#[repr(C)]
pub struct FileDropHandlerData {
    pub interface: IDropTarget,
    refcount: AtomicUsize,
    window: HWND,
    send_event: Box<dyn Fn(Event<'static, ()>)>,
    cursor_effect: u32,
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
        _this: *mut IUnknown,
        _riid: *const GUID,
        _ppvObject: *mut *mut c_void,
    ) -> HRESULT {
        // This function doesn't appear to be required for an `IDropTarget`.
        // An implementation would be nice however.
        unimplemented!();
    }

    pub unsafe extern "system" fn AddRef(this: *mut IUnknown) -> u32 {
        let drop_handler_data = Self::from_interface(this);
        let count = drop_handler_data.refcount.fetch_add(1, Ordering::Release) + 1;
        count as u32
    }

    pub unsafe extern "system" fn Release(this: *mut IUnknown) -> u32 {
        let drop_handler = Self::from_interface(this);
        let count = drop_handler.refcount.fetch_sub(1, Ordering::Release) - 1;
        if count == 0 {
            // Destroy the underlying data
            drop(Box::from_raw(drop_handler as *mut FileDropHandlerData));
        }
        count as u32
    }

    pub unsafe extern "system" fn DragEnter(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        _grfKeyState: u32,
        _pt: *const POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT {
        use crate::event::WindowEvent::HoveredFile;
        let drop_handler = Self::from_interface(this);
        let hdrop = Self::iterate_filenames(pDataObj, |filename| {
            drop_handler.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(drop_handler.window)),
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
        _grfKeyState: u32,
        _pt: *const POINTL,
        pdwEffect: *mut u32,
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
                window_id: RootWindowId(WindowId(drop_handler.window)),
                event: HoveredFileCancelled,
            });
        }

        S_OK
    }

    pub unsafe extern "system" fn Drop(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        _grfKeyState: u32,
        _pt: *const POINTL,
        _pdwEffect: *mut u32,
    ) -> HRESULT {
        use crate::event::WindowEvent::DroppedFile;
        let drop_handler = Self::from_interface(this);
        let hdrop = Self::iterate_filenames(pDataObj, |filename| {
            drop_handler.send_event(Event::WindowEvent {
                window_id: RootWindowId(WindowId(drop_handler.window)),
                event: DroppedFile(filename),
            });
        });
        if let Some(hdrop) = hdrop {
            DragFinish(hdrop);
        }

        S_OK
    }

    unsafe fn from_interface<'a, InterfaceT>(this: *mut InterfaceT) -> &'a mut FileDropHandlerData {
        &mut *(this as *mut _)
    }

    unsafe fn iterate_filenames<F>(data_obj: *const IDataObject, callback: F) -> Option<HDROP>
    where
        F: Fn(PathBuf),
    {
        let drop_format = FORMATETC {
            cfFormat: CF_HDROP,
            ptd: ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL as u32,
        };

        let mut medium = std::mem::zeroed();
        let get_data_fn = (*(*data_obj).cast::<IDataObjectVtbl>()).GetData;
        let get_data_result = get_data_fn(data_obj as *mut _, &drop_format, &mut medium);
        if get_data_result >= 0 {
            let hdrop = medium.Anonymous.hGlobal;

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
                DragQueryFileW(hdrop, i, path_buf.as_mut_ptr(), str_len as u32);
                path_buf.set_len(str_len);

                callback(OsString::from_wide(&path_buf[0..character_count]).into());
            }

            Some(hdrop)
        } else if get_data_result == DV_E_FORMATETC {
            // If the dropped item is not a file this error will occur.
            // In this case it is OK to return without taking further action.
            debug!("Error occured while processing dropped/hovered item: item is not a file.");
            None
        } else {
            debug!("Unexpected error occured while processing dropped/hovered item.");
            None
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
