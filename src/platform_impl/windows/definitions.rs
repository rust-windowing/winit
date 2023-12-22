#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]

use std::ffi::c_void;

use windows_sys::{
    core::{IUnknown, GUID, HRESULT, PCWSTR},
    Win32::{
        Foundation::{BOOL, HWND, POINTL, RECT},
        System::Com::{
            IAdviseSink, IDataObject, IEnumFORMATETC, IEnumSTATDATA, FORMATETC, STGMEDIUM,
        },
        UI::{Controls::HIMAGELIST, WindowsAndMessaging::HICON},
    },
};

#[repr(C)]
pub struct IUnknownVtbl {
    pub QueryInterface: unsafe extern "system" fn(
        This: *mut IUnknown,
        riid: *const GUID,
        ppvObject: *mut *mut c_void,
    ) -> HRESULT,
    pub AddRef: unsafe extern "system" fn(This: *mut IUnknown) -> u32,
    pub Release: unsafe extern "system" fn(This: *mut IUnknown) -> u32,
}

#[repr(C)]
pub struct IDataObjectVtbl {
    pub parent: IUnknownVtbl,
    pub GetData: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetcIn: *const FORMATETC,
        pmedium: *mut STGMEDIUM,
    ) -> HRESULT,
    pub GetDataHere: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetc: *const FORMATETC,
        pmedium: *mut STGMEDIUM,
    ) -> HRESULT,
    QueryGetData:
        unsafe extern "system" fn(This: *mut IDataObject, pformatetc: *const FORMATETC) -> HRESULT,
    pub GetCanonicalFormatEtc: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetcIn: *const FORMATETC,
        pformatetcOut: *mut FORMATETC,
    ) -> HRESULT,
    pub SetData: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetc: *const FORMATETC,
        pformatetcOut: *const FORMATETC,
        fRelease: BOOL,
    ) -> HRESULT,
    pub EnumFormatEtc: unsafe extern "system" fn(
        This: *mut IDataObject,
        dwDirection: u32,
        ppenumFormatEtc: *mut *mut IEnumFORMATETC,
    ) -> HRESULT,
    pub DAdvise: unsafe extern "system" fn(
        This: *mut IDataObject,
        pformatetc: *const FORMATETC,
        advf: u32,
        pAdvSInk: *const IAdviseSink,
        pdwConnection: *mut u32,
    ) -> HRESULT,
    pub DUnadvise: unsafe extern "system" fn(This: *mut IDataObject, dwConnection: u32) -> HRESULT,
    pub EnumDAdvise: unsafe extern "system" fn(
        This: *mut IDataObject,
        ppenumAdvise: *const *const IEnumSTATDATA,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IDropTargetVtbl {
    pub parent: IUnknownVtbl,
    pub DragEnter: unsafe extern "system" fn(
        This: *mut IDropTarget,
        pDataObj: *const IDataObject,
        grfKeyState: u32,
        pt: *const POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT,
    pub DragOver: unsafe extern "system" fn(
        This: *mut IDropTarget,
        grfKeyState: u32,
        pt: *const POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT,
    pub DragLeave: unsafe extern "system" fn(This: *mut IDropTarget) -> HRESULT,
    pub Drop: unsafe extern "system" fn(
        This: *mut IDropTarget,
        pDataObj: *const IDataObject,
        grfKeyState: u32,
        pt: *const POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT,
}

#[repr(C)]
pub struct IDropTarget {
    pub lpVtbl: *const IDropTargetVtbl,
}

#[repr(C)]
pub struct ITaskbarListVtbl {
    pub parent: IUnknownVtbl,
    pub HrInit: unsafe extern "system" fn(This: *mut ITaskbarList) -> HRESULT,
    pub AddTab: unsafe extern "system" fn(This: *mut ITaskbarList, hwnd: HWND) -> HRESULT,
    pub DeleteTab: unsafe extern "system" fn(This: *mut ITaskbarList, hwnd: HWND) -> HRESULT,
    pub ActivateTab: unsafe extern "system" fn(This: *mut ITaskbarList, hwnd: HWND) -> HRESULT,
    pub SetActiveAlt: unsafe extern "system" fn(This: *mut ITaskbarList, hwnd: HWND) -> HRESULT,
}

#[repr(C)]
pub struct ITaskbarList {
    pub lpVtbl: *const ITaskbarListVtbl,
}

#[repr(C)]
pub struct ITaskbarList2Vtbl {
    pub parent: ITaskbarListVtbl,
    pub MarkFullscreenWindow: unsafe extern "system" fn(
        This: *mut ITaskbarList2,
        hwnd: HWND,
        fFullscreen: BOOL,
    ) -> HRESULT,
}

#[repr(C)]
pub struct ITaskbarList2 {
    pub lpVtbl: *const ITaskbarList2Vtbl,
}

#[repr(C)]
pub struct ITaskbarList3Vtbl {
    pub parent: ITaskbarList2Vtbl,
    pub SetProgressValue: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwnd: HWND,
        ullCompleted: u64,
        ullTotal: u64,
    ) -> HRESULT,
    pub SetProgressState:
        unsafe extern "system" fn(This: *mut ITaskbarList3, hwnd: HWND, tbpFlags: u32) -> HRESULT,
    pub RegisterTab: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwndTab: HWND,
        hwndMDI: HWND,
    ) -> HRESULT,
    pub UnregisterTab:
        unsafe extern "system" fn(This: *mut ITaskbarList3, hwndTab: HWND) -> HRESULT,
    pub SetTabOrder: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwndTab: HWND,
        hwndInsertBefore: HWND,
    ) -> HRESULT,
    pub SetTabActive: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwndTab: HWND,
        hwndMDI: HWND,
    ) -> HRESULT,
    pub ThumbBarAddButtons: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwnd: HWND,
        cButtons: u32,
        pButton: *const c_void,
    ) -> HRESULT,
    pub ThumbBarUpdateButtons: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwnd: HWND,
        cButtons: u32,
        pButton: *const c_void,
    ) -> HRESULT,
    pub ThumbBarSetImageList: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwnd: HWND,
        himl: HIMAGELIST,
    ) -> HRESULT,
    pub SetOverlayIcon: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwnd: HWND,
        hIcon: HICON,
        pszDescription: PCWSTR,
    ) -> HRESULT,
    pub SetThumbnailTooltip:
        unsafe extern "system" fn(This: *mut ITaskbarList3, hwnd: HWND, pszTip: PCWSTR) -> HRESULT,
    pub SetThumbnailClip: unsafe extern "system" fn(
        This: *mut ITaskbarList3,
        hwnd: HWND,
        prcClip: *const RECT,
    ) -> HRESULT,
}

#[repr(C)]
pub struct ITaskbarList3 {
    pub lpVtbl: *const ITaskbarList3Vtbl,
}

pub const CLSID_TaskbarList: GUID = GUID {
    data1: 0x56fdf344,
    data2: 0xfd6d,
    data3: 0x11d0,
    data4: [0x95, 0x8a, 0x00, 0x60, 0x97, 0xc9, 0xa0, 0x90],
};

pub const IID_ITaskbarList: GUID = GUID {
    data1: 0x56FDF342,
    data2: 0xFD6D,
    data3: 0x11D0,
    data4: [0x95, 0x8A, 0x00, 0x60, 0x97, 0xC9, 0xA0, 0x90],
};

pub const IID_ITaskbarList2: GUID = GUID {
    data1: 0x602d4995,
    data2: 0xb13a,
    data3: 0x429b,
    data4: [0xa6, 0x6e, 0x19, 0x35, 0xe4, 0x4f, 0x43, 0x17],
};

pub const IID_ITaskbarList3: GUID = GUID {
    data1: 0xea1afb91,
    data2: 0x9e28,
    data3: 0x4b86,
    data4: [0x90, 0xe9, 0x9e, 0x9f, 0x8a, 0x5e, 0xef, 0xaf],
};
