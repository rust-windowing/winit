use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{mem, ptr};

use winapi::ctypes::c_void;
use winapi::shared::guiddef::REFIID;
use winapi::shared::minwindef::{DWORD, MAX_PATH, UINT, ULONG};
use winapi::shared::windef::{HWND, POINTL};
use winapi::shared::winerror::S_OK;
use winapi::um::objidl::IDataObject;
use winapi::um::oleidl::{IDropTarget, IDropTargetVtbl};
use winapi::um::winnt::HRESULT;
use winapi::um::{shellapi, unknwnbase};

use platform::platform::events_loop::send_event;
use platform::platform::WindowId;

use {Event, WindowId as SuperWindowId};

#[repr(C)]
pub struct FileDropHandlerData {
    pub interface: IDropTarget,
    refcount: AtomicUsize,
    window: HWND,
}

pub struct FileDropHandler {
    pub data: *mut FileDropHandlerData,
}

#[allow(non_snake_case)]
impl FileDropHandler {
    pub fn new(window: HWND) -> FileDropHandler {
        let data = Box::new(FileDropHandlerData {
            interface: IDropTarget {
                lpVtbl: &DROP_TARGET_VTBL as *const IDropTargetVtbl,
            },
            refcount: AtomicUsize::new(1),
            window,
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
        _pdwEffect: *mut DWORD,
    ) -> HRESULT {
        use events::WindowEvent::HoveredFile;
        let drop_handler = Self::from_interface(this);
        Self::iterate_filenames(pDataObj, |filename| {
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(drop_handler.window)),
                event: HoveredFile(filename),
            });
        });

        S_OK
    }

    pub unsafe extern "system" fn DragOver(
        _this: *mut IDropTarget,
        _grfKeyState: DWORD,
        _pt: *const POINTL,
        _pdwEffect: *mut DWORD,
    ) -> HRESULT {
        S_OK
    }

    pub unsafe extern "system" fn DragLeave(this: *mut IDropTarget) -> HRESULT {
        use events::WindowEvent::HoveredFileCancelled;
        let drop_handler = Self::from_interface(this);
        send_event(Event::WindowEvent {
            window_id: SuperWindowId(WindowId(drop_handler.window)),
            event: HoveredFileCancelled,
        });

        S_OK
    }

    pub unsafe extern "system" fn Drop(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        _grfKeyState: DWORD,
        _pt: *const POINTL,
        _pdwEffect: *mut DWORD,
    ) -> HRESULT {
        use events::WindowEvent::DroppedFile;
        let drop_handler = Self::from_interface(this);
        let hdrop = Self::iterate_filenames(pDataObj, |filename| {
            send_event(Event::WindowEvent {
                window_id: SuperWindowId(WindowId(drop_handler.window)),
                event: DroppedFile(filename),
            });
        });
        shellapi::DragFinish(hdrop);

        S_OK
    }

    unsafe fn from_interface<'a, InterfaceT>(this: *mut InterfaceT) -> &'a mut FileDropHandlerData {
        &mut *(this as *mut _)
    }

    unsafe fn iterate_filenames<F>(data_obj: *const IDataObject, callback: F) -> shellapi::HDROP
    where
        F: Fn(PathBuf),
    {
        use winapi::ctypes::wchar_t;
        use winapi::shared::winerror::SUCCEEDED;
        use winapi::shared::wtypes::{CLIPFORMAT, DVASPECT_CONTENT};
        use winapi::um::objidl::{FORMATETC, TYMED_HGLOBAL};
        use winapi::um::shellapi::DragQueryFileW;
        use winapi::um::winuser::CF_HDROP;

        let mut drop_format = FORMATETC {
            cfFormat: CF_HDROP as CLIPFORMAT,
            ptd: ptr::null(),
            dwAspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL,
        };

        let mut medium = mem::uninitialized();
        if SUCCEEDED((*data_obj).GetData(&mut drop_format, &mut medium)) {
            let hglobal = (*medium.u).hGlobal();
            let hdrop = (*hglobal) as shellapi::HDROP;

            // The second parameter (0xFFFFFFFF) instructs the function to return the item count
            let item_count = DragQueryFileW(hdrop, 0xFFFFFFFF, ptr::null_mut(), 0);

            let mut pathbuf: [wchar_t; MAX_PATH] = mem::uninitialized();

            for i in 0..item_count {
                let character_count =
                    DragQueryFileW(hdrop, i, pathbuf.as_mut_ptr(), MAX_PATH as UINT) as usize;

                if character_count > 0 {
                    callback(OsString::from_wide(&pathbuf[0..character_count]).into());
                }
            }

            return hdrop;
        }

        // The call to `GetData` must succeed and the file handle must be returned before this
        // point
        unreachable!();
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
