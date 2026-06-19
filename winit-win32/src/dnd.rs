use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{OsString, c_void};
use std::io;
use std::num::NonZeroU32;
use std::ops::{BitOr, ControlFlow};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{self, AtomicI64, AtomicUsize, Ordering};

use dpi::PhysicalPosition;
use windows_sys::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, DV_E_FORMATETC, E_ABORT,
    E_FAIL, E_NOINTERFACE, E_NOTIMPL, E_UNEXPECTED, GlobalFree, HGLOBAL, HWND,
    OLE_E_ADVISENOTSUPPORTED, POINT, POINTL, S_FALSE, S_OK,
};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::System::Com::{
    DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_ENHMF, TYMED_FILE, TYMED_GDI, TYMED_HGLOBAL,
    TYMED_MFPICT,
};
use windows_sys::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows_sys::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows_sys::Win32::System::Ole::{
    CF_HDROP, CF_UNICODETEXT, DROPEFFECT_COPY, DROPEFFECT_LINK, DROPEFFECT_MOVE, DROPEFFECT_NONE,
    OleDuplicateData, ReleaseStgMedium,
};
use windows_sys::Win32::System::SystemServices::{
    MK_LBUTTON, MK_MBUTTON, MK_RBUTTON, MK_XBUTTON1, MK_XBUTTON2,
};
use windows_sys::Win32::UI::Shell::{DROPFILES, DragQueryFileW, HDROP};
use windows_sys::core::{BOOL, GUID, HRESULT, IID_IUnknown};
use winit_core::data_transfer::{
    DataTransfer, DataTransferId, DataTransferSend, SendData, TransferType, TypeHint, TypedData,
};
use winit_core::event::WindowEvent;
use winit_core::event_loop::DndAction;
use winit_core::window::WindowId;

use crate::definitions::{
    IDataObject, IDataObjectVtbl, IDropSource, IDropSourceVtbl, IDropTarget, IDropTargetHelper,
    IDropTargetHelperVtbl, IDropTargetVtbl, IEnumFORMATETC, IEnumFORMATETCVtbl, IID_IDataObject,
    IID_IDropSource, IID_IDropTargetHelper, IID_IEnumFORMATETC, IUnknown, IUnknownVtbl,
};
use crate::event_loop::EventLoopRunner;
use crate::util;

#[derive(Debug)]
enum DataKind {
    Uris(Vec<OsString>),
    String(String),
    Bytes(Vec<u8>),
}

// TODO: Exposing the full native API to client applications is too error-prone so long as
// winit is still manually implementing refcounting and using the win32 APIs. For now, we
// just eagerly read all the data supported by cross-platform type hints on Windows. This
// would be resolved by migrating to `windows-rs`.
#[derive(Debug)]
pub(crate) struct DataObject {
    data: HashMap<TypeHint, DataKind>,
}

impl DataObject {
    unsafe fn from_idataobject(data_obj: *const IDataObject) -> Self {
        let mut data = HashMap::new();

        if let Some(text) = unsafe { read_unicode_text(data_obj) } {
            data.insert(TypeHint::Plaintext, DataKind::String(text));
        }

        if let Some(uris) = unsafe { read_uri_list(data_obj) } {
            if !uris.is_empty() {
                data.insert(TypeHint::UriList, DataKind::Uris(uris));
            }
        }

        if let Some(png) = unsafe { read_png(data_obj) } {
            if !png.is_empty() {
                data.insert(TypeHint::Image { extension_hint: Some("png") }, DataKind::Bytes(png));
            }
        }

        Self { data }
    }

    fn resolve(&self, requested: TypeHint) -> Option<TypeHint> {
        self.data.keys().copied().find(|stored| stored.matches(&requested))
    }
}

/// RAII wrapper around an STGMEDIUM returned by IDataObject::GetData, releasing it on drop.
struct StgMedium(STGMEDIUM);

impl StgMedium {
    /// Returns `None` if the object doesn't provide the format.
    unsafe fn get(data_obj: *const IDataObject, cf_format: u16) -> Option<Self> {
        let format = FORMATETC {
            cfFormat: cf_format,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT,
            lindex: -1,
            tymed: TYMED_HGLOBAL as u32,
        };

        let mut medium = unsafe { std::mem::zeroed::<STGMEDIUM>() };
        let get_data = unsafe { (*(*data_obj).cast::<IDataObjectVtbl>()).GetData };
        if unsafe { get_data(data_obj as *mut _, &format, &mut medium) } < 0 {
            return None;
        }

        Some(Self(medium))
    }

    fn hglobal(&self) -> HGLOBAL {
        unsafe { self.0.u.hGlobal }
    }
}

impl Drop for StgMedium {
    fn drop(&mut self) {
        unsafe { ReleaseStgMedium(&mut self.0) };
    }
}

unsafe fn read_unicode_text(data_obj: *const IDataObject) -> Option<String> {
    let medium = unsafe { StgMedium::get(data_obj, CF_UNICODETEXT) }?;
    let hglobal = medium.hglobal();

    let ptr = unsafe { GlobalLock(hglobal) };
    if ptr.is_null() {
        return None;
    }

    // `CF_UNICODETEXT` is a NUL-terminated UTF-16 string. Cap the scan by the allocation size in
    // case the buffer isn't terminated.
    let max_units = unsafe { GlobalSize(hglobal) } / std::mem::size_of::<u16>();
    let wide = unsafe { std::slice::from_raw_parts(ptr.cast::<u16>(), max_units) };
    let len = wide.iter().position(|&c| c == 0).unwrap_or(max_units);
    let text = String::from_utf16_lossy(&wide[..len]);

    unsafe { GlobalUnlock(hglobal) };

    Some(text)
}

unsafe fn read_uri_list(data_obj: *const IDataObject) -> Option<Vec<OsString>> {
    let medium = unsafe { StgMedium::get(data_obj, CF_HDROP) }?;
    let hdrop = medium.hglobal() as HDROP;

    // The second parameter (0xFFFFFFFF) instructs the function to return the item count.
    let item_count = unsafe { DragQueryFileW(hdrop, 0xffff_ffff, std::ptr::null_mut(), 0) };

    let mut paths = Vec::with_capacity(item_count as usize);
    for i in 0..item_count {
        // Query the path length (excluding the NUL terminator), reserve room for it plus the
        // terminator, then copy. `set_len` uses the count actually written, so a short copy can
        // never expose uninitialized memory.
        let character_count = unsafe { DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0) } as usize;

        let mut path_buf = Vec::<u16>::with_capacity(character_count + 1);
        let copied =
            unsafe { DragQueryFileW(hdrop, i, path_buf.as_mut_ptr(), character_count as u32 + 1) }
                as usize;
        unsafe { path_buf.set_len(copied) };

        paths.push(OsString::from_wide(&path_buf));
    }

    Some(paths)
}

unsafe fn read_png(data_obj: *const IDataObject) -> Option<Vec<u8>> {
    let format_name = util::encode_wide("PNG");
    let format = unsafe { RegisterClipboardFormatW(format_name.as_ptr()) };
    if format == 0 {
        return None;
    }

    let medium = unsafe { StgMedium::get(data_obj, format as u16) }?;
    let hglobal = medium.hglobal();

    let ptr = unsafe { GlobalLock(hglobal) };
    if ptr.is_null() {
        return None;
    }

    let len = unsafe { GlobalSize(hglobal) };
    let bytes = unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) }.to_vec();

    unsafe { GlobalUnlock(hglobal) };

    Some(bytes)
}

#[derive(Debug)]
pub(crate) struct WinDataTransfer {
    data: Arc<DataObject>,
}

impl WinDataTransfer {
    pub(crate) fn new(data: Arc<DataObject>) -> Self {
        Self { data }
    }
}

impl DataTransfer for WinDataTransfer {
    fn for_each_available_type<'this>(
        &'this self,
        func: &'_ mut dyn FnMut(&'this dyn TransferType) -> ControlFlow<()>,
    ) {
        for hint in self.data.data.keys() {
            if let ControlFlow::Break(()) = func(hint) {
                break;
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct WinTypedData {
    type_: TypeHint,
    data: Arc<DataObject>,
}

impl WinTypedData {
    pub(crate) fn new(data: Arc<DataObject>, requested: TypeHint) -> Option<Self> {
        let type_ = data.resolve(requested)?;
        Some(Self { type_, data })
    }
}

impl TypedData for WinTypedData {
    fn type_(&self) -> &dyn TransferType {
        &self.type_
    }

    fn try_read(&self) -> Option<Box<dyn io::BufRead>> {
        match self.data.data.get(&self.type_)? {
            DataKind::Bytes(bytes) => Some(Box::new(io::Cursor::new(bytes.clone()))),
            DataKind::String(string) => {
                Some(Box::new(io::Cursor::new(string.clone().into_bytes())))
            },
            // Windows URI drag-and-drop can't be neatly expressed as a binary blob.
            DataKind::Uris(_) => None,
        }
    }

    fn try_as_uris(&self) -> io::Result<Vec<OsString>> {
        match self.data.data.get(&self.type_) {
            Some(DataKind::Uris(uris)) => Ok(uris.clone()),
            _ => Err(io::ErrorKind::InvalidData.into()),
        }
    }

    fn try_as_string(&self) -> io::Result<String> {
        match self.data.data.get(&self.type_) {
            Some(DataKind::String(string)) => Ok(string.clone()),
            _ => Err(io::ErrorKind::InvalidData.into()),
        }
    }
}

#[repr(C)]
pub struct FileDropHandlerData {
    interface: IDropTarget,
    refcount: AtomicUsize,
    window: HWND,
    runner: Rc<EventLoopRunner>,
    send_event: Box<dyn Fn(WindowEvent)>,
    active_data_transfer_id: Option<DataTransferId>,
    // Shell drop-target helper. Lazy-init on first DragEnter; `None` means "not yet created" or
    // "creation failed and we're running without a drag image". Forwarding to this is what
    // makes the source's `IDragSourceHelper` bitmap actually render under the cursor over our
    // own window and any other helper-aware target.
    drop_target_helper: Option<NonNull<IDropTargetHelper>>,
}

pub struct FileDropHandler {
    data: *mut FileDropHandlerData,
}

#[allow(non_snake_case)]
impl FileDropHandler {
    pub(crate) fn new(
        window: HWND,
        runner: Rc<EventLoopRunner>,
        send_event: Box<dyn Fn(WindowEvent)>,
    ) -> FileDropHandler {
        let data = Box::new(FileDropHandlerData {
            interface: IDropTarget { lpVtbl: &DROP_TARGET_VTBL as *const IDropTargetVtbl },
            refcount: AtomicUsize::new(1),
            window,
            runner,
            send_event,
            active_data_transfer_id: None,
            drop_target_helper: None,
        });
        FileDropHandler { data: Box::into_raw(data) }
    }

    /// Lazy-create the shell drop-target helper. Returns `None` if creation failed; callers
    /// should treat that as "no drag image" and continue silently - failure is purely cosmetic.
    unsafe fn ensure_drop_target_helper(
        data: &mut FileDropHandlerData,
    ) -> Option<NonNull<IDropTargetHelper>> {
        use windows_sys::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
        use windows_sys::Win32::UI::Shell::CLSID_DragDropHelper;

        if let Some(helper) = data.drop_target_helper {
            return Some(helper);
        }
        let mut helper: *mut IDropTargetHelper = std::ptr::null_mut();
        let hr = unsafe {
            CoCreateInstance(
                &CLSID_DragDropHelper,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_IDropTargetHelper,
                &mut helper as *mut _ as *mut _,
            )
        };
        if hr < 0 {
            return None;
        }
        let helper = NonNull::new(helper)?;
        data.drop_target_helper = Some(helper);
        Some(helper)
    }

    unsafe fn helper_vtbl(helper: NonNull<IDropTargetHelper>) -> &'static IDropTargetHelperVtbl {
        unsafe { &*(*(helper.as_ptr() as *mut *const IDropTargetHelperVtbl)) }
    }

    pub(crate) unsafe fn interface_unchecked_mut(&mut self) -> &mut IDropTarget {
        unsafe { &mut (*self.data).interface }
    }

    // Implement IUnknown
    unsafe extern "system" fn QueryInterface(
        _this: *mut IUnknown,
        _riid: *const GUID,
        _ppvObject: *mut *mut c_void,
    ) -> HRESULT {
        // This function doesn't appear to be required for an `IDropTarget`.
        // An implementation would be nice however.
        // Can't use `unimplemented` here as it's invalid to panic over an FFI boundary.
        tracing::warn!("`QueryInterface` called, but it was unimplemented");
        E_FAIL
    }

    unsafe extern "system" fn AddRef(this: *mut IUnknown) -> u32 {
        let drop_handler_data = unsafe { Self::from_interface(this) };
        let count = drop_handler_data.refcount.fetch_add(1, Ordering::Relaxed) + 1;
        count as u32
    }

    unsafe extern "system" fn Release(this: *mut IUnknown) -> u32 {
        let drop_handler = unsafe { Self::from_interface(this) };
        // Release on decrement publishes any writes made through this reference before the
        // count is observed by other threads. When we hit zero, fence with Acquire so the
        // destructor sees all writes from prior Releases on other threads - the standard
        // Arc pattern. Without the fence, dropping the box could race with reads done by
        // the last releasing thread on another core.
        let count = drop_handler.refcount.fetch_sub(1, Ordering::Release) - 1;
        if count == 0 {
            atomic::fence(Ordering::Acquire);
            // Drop any transfer still in flight (e.g. the window was destroyed mid-drag, so no
            // `DragLeave`/`Drop` ever arrived to clean it up).
            if let Some(id) = drop_handler.active_data_transfer_id.take() {
                drop_handler.runner.remove_data_transfer(id);
            }
            // Release the shell drop-target helper if we created one.
            if let Some(helper) = drop_handler.drop_target_helper {
                let vtbl = unsafe { Self::helper_vtbl(helper) };
                unsafe {
                    (vtbl.parent.Release)(helper.as_ptr() as *mut IUnknown);
                }
            }
            // Destroy the underlying data
            drop(unsafe { Box::from_raw(drop_handler as *mut FileDropHandlerData) });
        }
        count as u32
    }

    unsafe extern "system" fn DragEnter(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT {
        let drop_handler = unsafe { Self::from_interface(this) };
        // If this is a self-drop (we initiated the drag from this process), reuse the source's id
        // and seed actions from the mask declared at `start_drag` - the app's `DragEntered`
        // handler can't call `set_valid_actions` in time because it's buffered until `DoDragDrop`
        // returns.
        let data_transfer_id = drop_handler
            .runner
            .source_drag
            .get()
            .map_or_else(next_data_transfer_id, |info| info.id);
        drop_handler.active_data_transfer_id = Some(data_transfer_id);

        let wid = WindowId::from_raw(drop_handler.window.addr());

        let data = Arc::new(unsafe { DataObject::from_idataobject(pDataObj) });
        drop_handler.runner.register_data_transfer(data_transfer_id, wid, data);

        let pt_screen = POINT { x: pt.x, y: pt.y };
        let mut pt_client = pt_screen;
        unsafe {
            ScreenToClient(drop_handler.window, &mut pt_client);
        }
        let position = PhysicalPosition::new(pt_client.x as f64, pt_client.y as f64);
        (drop_handler.send_event)(WindowEvent::DragEntered {
            id: data_transfer_id,
            position: Some(position),
        });

        // Get actions after the event handler has run, so that we update it based on the user's
        // supplied info.
        {
            let actions = drop_handler.runner.current_drag_actions(data_transfer_id);
            let source_allowed = unsafe { pdwEffect.read() };

            let new_effect = pick_effect(&actions, grfKeyState, source_allowed);
            unsafe {
                pdwEffect.write(new_effect);
            }
        }

        // Forward to the shell drop-target helper so any drag image attached by the source's
        // IDragSourceHelper renders the bitmap under the cursor while it's over our window.
        if let Some(helper) = unsafe { Self::ensure_drop_target_helper(drop_handler) } {
            let vtbl = unsafe { Self::helper_vtbl(helper) };
            unsafe {
                (vtbl.DragEnter)(
                    helper.as_ptr(),
                    drop_handler.window,
                    pDataObj as *mut IDataObject,
                    &pt_screen,
                    *pdwEffect,
                );
            }
        }

        S_OK
    }

    unsafe extern "system" fn DragOver(
        this: *mut IDropTarget,
        grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT {
        let drop_handler = unsafe { Self::from_interface(this) };
        let Some(data_transfer_id) = drop_handler.active_data_transfer_id else {
            unsafe {
                pdwEffect.write(DROPEFFECT_NONE);
            }

            return E_ABORT;
        };

        let effects = unsafe { pdwEffect.read() };

        let pt_screen = POINT { x: pt.x, y: pt.y };
        let mut pt_client = pt_screen;
        unsafe {
            ScreenToClient(drop_handler.window, &mut pt_client);
        }
        let position = PhysicalPosition::new(pt_client.x as f64, pt_client.y as f64);

        let proposed_action = drop_handler.runner.proposed_dnd_action(data_transfer_id, effects);

        (drop_handler.send_event)(WindowEvent::DragPosition {
            id: data_transfer_id,
            position,
            proposed_action,
        });

        // Get actions after the event handler has run, so that we update it based on the user's
        // supplied info.
        let actions = drop_handler.runner.current_drag_actions(data_transfer_id);
        let new_effect = pick_effect(&actions, grfKeyState, effects);
        unsafe {
            pdwEffect.write(new_effect);
        }

        if let Some(helper) = drop_handler.drop_target_helper {
            let vtbl = unsafe { Self::helper_vtbl(helper) };
            unsafe { (vtbl.DragOver)(helper.as_ptr(), &pt_screen, new_effect) };
        }

        S_OK
    }

    unsafe extern "system" fn DragLeave(this: *mut IDropTarget) -> HRESULT {
        let drop_handler = unsafe { Self::from_interface(this) };
        let Some(data_transfer_id) = drop_handler.active_data_transfer_id.take() else {
            return E_ABORT;
        };

        (drop_handler.send_event)(WindowEvent::DragLeft { id: data_transfer_id });
        drop_handler.runner.remove_data_transfer(data_transfer_id);

        if let Some(helper) = drop_handler.drop_target_helper {
            let vtbl = unsafe { Self::helper_vtbl(helper) };
            unsafe { (vtbl.DragLeave)(helper.as_ptr()) };
        }

        S_OK
    }

    unsafe extern "system" fn Drop(
        this: *mut IDropTarget,
        pDataObj: *const IDataObject,
        grfKeyState: u32,
        pt: POINTL,
        pdwEffect: *mut u32,
    ) -> HRESULT {
        let drop_handler = unsafe { Self::from_interface(this) };
        let Some(data_transfer_id) = drop_handler.active_data_transfer_id.take() else {
            unsafe {
                *pdwEffect = DROPEFFECT_NONE;
            }

            return E_ABORT;
        };

        let effects = unsafe { pdwEffect.read() };
        let proposed_action = drop_handler.runner.proposed_dnd_action(data_transfer_id, effects);

        let pt_screen = POINT { x: pt.x, y: pt.y };
        let mut pt_client = pt_screen;
        unsafe {
            ScreenToClient(drop_handler.window, &mut pt_client);
        }
        let pt = pt_client;
        let position = PhysicalPosition::new(pt.x as f64, pt.y as f64);

        (drop_handler.send_event)(WindowEvent::DragPosition {
            id: data_transfer_id,
            position,
            proposed_action,
        });

        // Get actions after the event handler has run, so that we update it based on the user's
        // supplied info.
        let actions = drop_handler.runner.current_drag_actions(data_transfer_id);
        let source_allowed = unsafe { pdwEffect.read() };
        let proposed_action = drop_handler.runner.proposed_dnd_action(data_transfer_id, effects);

        // Negotiate the effect first so we can pick the right outgoing event. If the app
        // rejected the drop (e.g. via `set_valid_actions(none())`), `pick_effect` returns
        // `DROPEFFECT_NONE`; in that case OLE reports back `effect_out == DROPEFFECT_NONE` to
        // the source, so the matching observer-facing event is `DragLeft`, not `DragDropped` -
        // otherwise target and source see contradictory outcomes.
        let effect = pick_effect(&actions, grfKeyState, source_allowed);
        let event = if effect == DROPEFFECT_NONE {
            WindowEvent::DragLeft { id: data_transfer_id }
        } else {
            WindowEvent::DragDropped { id: data_transfer_id, proposed_action }
        };
        (drop_handler.send_event)(event);
        unsafe {
            *pdwEffect = effect;
        }

        if let Some(helper) = drop_handler.drop_target_helper {
            let vtbl = unsafe { Self::helper_vtbl(helper) };
            unsafe {
                (vtbl.Drop)(helper.as_ptr(), pDataObj as *mut IDataObject, &pt_screen, effect);
            }
        }

        // External drop: the app's handler dispatched synchronously above and has already read
        // the data; safe to release the cache. Self-drop: the handler is buffered and hasn't
        // run yet, so defer cleanup until after `dispatch_buffered_events` drains.
        if drop_handler.runner.source_drag.get().is_some() {
            drop_handler.runner.defer_source_drag_cleanup(data_transfer_id);
        } else {
            drop_handler.runner.remove_data_transfer(data_transfer_id);
        }

        S_OK
    }

    unsafe fn from_interface<'a, InterfaceT>(this: *mut InterfaceT) -> &'a mut FileDropHandlerData {
        unsafe { &mut *(this as *mut _) }
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

pub(crate) type DropEffect = u32;

/// Map the app's [`DndActions`] to the win32 `DROPEFFECT_*` bitmask.
pub(crate) fn dnd_action_to_dropeffect_mask(action: DndAction) -> DropEffect {
    match action {
        DndAction::Move => DROPEFFECT_MOVE,
        DndAction::Copy => DROPEFFECT_COPY,
        DndAction::Link => DROPEFFECT_LINK,
        _ => DROPEFFECT_NONE,
    }
}

/// Map the app's [`DndActions`] to the win32 `DROPEFFECT_*` bitmask.
pub(crate) fn dnd_actions_to_dropeffect_mask(actions: &[DndAction]) -> DropEffect {
    actions
        .iter()
        .copied()
        .map(dnd_action_to_dropeffect_mask)
        .fold(DropEffect::default(), BitOr::bitor)
}

pub(crate) fn drop_effect_to_dnd_action(effect: DropEffect) -> Option<DndAction> {
    match effect {
        DROPEFFECT_MOVE => Some(DndAction::Move),
        DROPEFFECT_COPY => Some(DndAction::Copy),
        DROPEFFECT_LINK => Some(DndAction::Link),
        _ => None,
    }
}

// Intersect the app's valid actions with the source's allowed effects, honoring Ctrl/Shift.
fn pick_effect(actions: &[DndAction], key_state: u32, source_allowed: u32) -> u32 {
    const MK_SHIFT: u32 = 0x0004;
    const MK_CONTROL: u32 = 0x0008;

    // TODO: On macOS, option will disable copy (so if copy has higher precedence than move you can
    // select move via option). Is alt expected to act similarly on Windows?
    let mut allowed = dnd_actions_to_dropeffect_mask(actions) & source_allowed;
    if allowed == 0 {
        return DROPEFFECT_NONE;
    }

    // If holding a modifier would result in no valid values, ignore
    // Need to use filter instead of if-let chains for 1.85 compatibility.
    if let Some(new_allowed) =
        NonZeroU32::new(allowed & !DROPEFFECT_MOVE).filter(|_| key_state & MK_CONTROL != 0)
    {
        allowed = new_allowed.get();
    }

    // If holding a modifier would result in no valid values, ignore
    // Need to use filter instead of if-let chains for 1.85 compatibility.
    if let Some(new_allowed) =
        NonZeroU32::new(allowed & !DROPEFFECT_COPY).filter(|_| key_state & MK_SHIFT != 0)
    {
        allowed = new_allowed.get();
    }

    for action in actions {
        let effect = dnd_action_to_dropeffect_mask(*action);
        if (allowed & effect) != 0 {
            return effect;
        }
    }

    DROPEFFECT_NONE
}

// ============================================================================
// Source side: providing data and controlling a `DoDragDrop` session.
// ============================================================================

/// Mint a unique [`DataTransferId`] for either a target-side `DragEnter` or a source-side
/// `start_drag` so the two never collide.
pub(crate) fn next_data_transfer_id() -> DataTransferId {
    static COUNTER: AtomicI64 = AtomicI64::new(0);
    DataTransferId::from_raw(COUNTER.fetch_add(1, Ordering::Relaxed))
}

fn guids_eq(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1 && a.data2 == b.data2 && a.data3 == b.data3 && a.data4 == b.data4
}

fn register_clipboard_format(name: &str) -> Option<u16> {
    let wide = util::encode_wide(name);
    let atom = unsafe { RegisterClipboardFormatW(wide.as_ptr()) };
    (atom != 0).then_some(atom as u16)
}

// Returns the (cf_format, specialised hint) pairs we can lower `hint` to.
//
// Most hints map 1:1. Wildcard hints like `Image { extension_hint: None }` -
// the cross-platform way to say "any image format" - fan out to one entry per
// concrete format we support, each paired with the specialised hint so
// `data_for_type` is invoked with a concrete extension instead of `None`.
fn cf_formats_for_hint(hint: TypeHint) -> Vec<(u16, TypeHint)> {
    fn one(cf: Option<u16>, hint: TypeHint) -> Vec<(u16, TypeHint)> {
        cf.map(|cf| vec![(cf, hint)]).unwrap_or_default()
    }
    match hint {
        TypeHint::Plaintext => vec![(CF_UNICODETEXT, hint)],
        TypeHint::UriList => vec![(CF_HDROP, hint)],
        TypeHint::Html => one(register_clipboard_format("HTML Format"), hint),
        TypeHint::Image { extension_hint: Some("png") } => {
            one(register_clipboard_format("PNG"), hint)
        },
        TypeHint::Image { extension_hint: None } => {
            // Fan out to every concrete image format we can produce.
            let mut out = Vec::new();
            if let Some(cf) = register_clipboard_format("PNG") {
                out.push((cf, TypeHint::Image { extension_hint: Some("png") }));
            }
            out
        },
        _ => Vec::new(),
    }
}

/// Duplicate a `STGMEDIUM` for handing out via `GetData` without losing the original.
///
/// Delegates to `OleDuplicateData` which knows how to clone HGLOBAL, HBITMAP, HENHMETAFILE,
/// HMETAFILEPICT and file-name mediums. Returns `None` for tymeds the shell helper doesn't use
/// (interface-based mediums like IStream / IStorage) so we never hand out an aliased pointer
/// the caller would later `Release` once we also drop ours.
unsafe fn duplicate_stgmedium(src: &STGMEDIUM, cf_format: u16) -> Option<STGMEDIUM> {
    use windows_sys::Win32::Foundation::HANDLE;

    // Pull the handle out of the union by tymed. The shell drag helper only uses HGLOBAL today,
    // but forwarding every handle-typed tymed `OleDuplicateData` understands is essentially
    // free, so do it. Interface-based tymeds (IStream / IStorage) deliberately fall through -
    // duplicating them properly requires AddRef, not OleDuplicateData.
    let tymed = src.tymed as i32;
    let handle: HANDLE = unsafe {
        if tymed == TYMED_HGLOBAL {
            src.u.hGlobal as HANDLE
        } else if tymed == TYMED_FILE {
            src.u.lpszFileName as HANDLE
        } else if tymed == TYMED_GDI {
            src.u.hBitmap as HANDLE
        } else if tymed == TYMED_MFPICT {
            src.u.hMetaFilePict as HANDLE
        } else if tymed == TYMED_ENHMF {
            src.u.hEnhMetaFile as HANDLE
        } else {
            return None;
        }
    };
    let dup = unsafe { OleDuplicateData(handle, cf_format, 0) };
    if dup.is_null() {
        return None;
    }
    let mut out: STGMEDIUM = unsafe { std::mem::zeroed() };
    out.tymed = src.tymed;
    if tymed == TYMED_HGLOBAL {
        out.u.hGlobal = dup as _;
    } else if tymed == TYMED_FILE {
        out.u.lpszFileName = dup as _;
    } else if tymed == TYMED_GDI {
        out.u.hBitmap = dup as _;
    } else if tymed == TYMED_MFPICT {
        out.u.hMetaFilePict = dup as _;
    } else if tymed == TYMED_ENHMF {
        out.u.hEnhMetaFile = dup as _;
    } else {
        // Should be unreachable - the same `tymed` matched the first chain above. Return
        // `None` rather than panic: this runs across the COM/FFI boundary, where unwinding
        // would be UB.
        return None;
    }
    Some(out)
}

fn alloc_hglobal_from(src: &[u8]) -> Option<HGLOBAL> {
    let hglobal = unsafe { GlobalAlloc(GMEM_MOVEABLE, src.len()) };
    if hglobal.is_null() {
        return None;
    }
    let dst = unsafe { GlobalLock(hglobal) };
    if dst.is_null() {
        // `GlobalAlloc` succeeded but locking failed - free before bailing so we don't
        // leak the moveable handle.
        unsafe { GlobalFree(hglobal) };
        return None;
    }
    unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), dst as *mut u8, src.len()) };
    unsafe { GlobalUnlock(hglobal) };
    Some(hglobal)
}

/// Build the [HTML Clipboard Format] wire bytes from an app-supplied HTML string.
///
/// The format is a UTF-8 buffer with a small text header naming byte offsets into itself. Each
/// offset placeholder is exactly 10 zero-padded decimal digits, which fixes the header at a
/// known constant length and removes the chicken-and-egg between header length and offsets.
///
/// If the input already contains `<!--StartFragment-->` we trust the caller's wrapping;
/// otherwise we wrap it in a minimal `<html><body>` document with fragment markers.
///
/// [HTML Clipboard Format]: https://learn.microsoft.com/en-us/windows/win32/dataxchg/html-clipboard-format
fn build_html_clipboard_format(html: &str) -> Vec<u8> {
    use std::borrow::Cow;

    const START_MARKER: &str = "<!--StartFragment-->";
    const END_MARKER: &str = "<!--EndFragment-->";

    let body: Cow<'_, str> = if html.contains(START_MARKER) {
        Cow::Borrowed(html)
    } else {
        Cow::Owned(format!("<html><body>\r\n{START_MARKER}{html}{END_MARKER}\r\n</body></html>"))
    };

    const HEADER_LEN: usize = concat!(
        "Version:0.9\r\n",
        "StartHTML:0000000000\r\n",
        "EndHTML:0000000000\r\n",
        "StartFragment:0000000000\r\n",
        "EndFragment:0000000000\r\n",
    )
    .len();

    let start_html = HEADER_LEN;
    let end_html = HEADER_LEN + body.len();
    let start_fragment = HEADER_LEN + body.find(START_MARKER).unwrap() + START_MARKER.len();
    let end_fragment = HEADER_LEN + body.find(END_MARKER).unwrap();

    let header = format!(
        "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:\
         {start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n",
    );
    debug_assert_eq!(header.len(), HEADER_LEN);

    let mut buf = Vec::with_capacity(header.len() + body.len());
    buf.extend_from_slice(header.as_bytes());
    buf.extend_from_slice(body.as_bytes());
    buf
}

/// Convert app-supplied [`SendData`] into an `HGLOBAL`-backed [`STGMEDIUM`].
///
/// Callers must have already validated that the `SendData` variant matches the on-the-wire shape
/// the requested clipboard format expects (see `variant_matches_hint` at the `GetData` call site).
unsafe fn send_data_to_stgmedium(data: SendData, hint: TypeHint) -> Option<STGMEDIUM> {
    let hglobal = match data {
        SendData::String(s) if matches!(hint, TypeHint::Html) => {
            // HTML Clipboard Format: UTF-8 with a Version/StartHTML/EndHTML/StartFragment/
            // EndFragment header. Targets parse the header before reading the wrapped HTML.
            let bytes = build_html_clipboard_format(&s);
            alloc_hglobal_from(&bytes)?
        },
        SendData::String(s) => {
            // UTF-16 + NUL - used for `CF_UNICODETEXT` and other text-ish registered formats.
            let utf16 = util::encode_wide(&s);
            let utf16_bytes =
                unsafe { std::slice::from_raw_parts(utf16.as_ptr() as *const u8, utf16.len() * 2) };
            alloc_hglobal_from(utf16_bytes)?
        },
        SendData::Uris(paths) => {
            // CF_HDROP: `DROPFILES` header + double-NUL-terminated UTF-16 path list.
            let mut wide: Vec<u16> = Vec::new();
            for path in paths {
                let path = 'uri_to_path: {
                    if let Some(path_str) = path.to_str() {
                        // There's no `strip_prefix` etc on `OsStr` so we need to go via `str`
                        // Windows is the only platform that sends raw file paths instead of URIs
                        let Some(path_str) = path_str.strip_prefix("file://") else {
                            break 'uri_to_path path;
                        };

                        // Even though "/" is theoretically a valid path separator on Windows, it
                        // doesn't seem to work for drag-and-drop specifically.
                        OsString::from(path_str.replace("/", "\\"))
                    } else {
                        path
                    }
                };

                wide.extend(path.encode_wide());
                wide.push(0);
            }
            wide.push(0);
            let header = DROPFILES {
                pFiles: std::mem::size_of::<DROPFILES>() as u32,
                pt: POINT { x: 0, y: 0 },
                fNC: 0,
                fWide: 1,
            };
            let total = std::mem::size_of::<DROPFILES>() + wide.len() * 2;
            let hglobal = unsafe { GlobalAlloc(GMEM_MOVEABLE, total) };
            if hglobal.is_null() {
                return None;
            }
            let dst = unsafe { GlobalLock(hglobal) };
            if dst.is_null() {
                unsafe { GlobalFree(hglobal) };
                return None;
            }
            unsafe {
                std::ptr::write_unaligned(dst as *mut DROPFILES, header);
                let paths_dst = (dst as *mut u8).add(std::mem::size_of::<DROPFILES>());
                std::ptr::copy_nonoverlapping(
                    wide.as_ptr() as *const u8,
                    paths_dst,
                    wide.len() * 2,
                );
                GlobalUnlock(hglobal);
            }
            hglobal
        },
        SendData::Bytes(b) => alloc_hglobal_from(&b)?,
    };

    let mut medium = unsafe { std::mem::zeroed::<STGMEDIUM>() };
    medium.tymed = TYMED_HGLOBAL as u32;
    medium.u.hGlobal = hglobal;
    Some(medium)
}

/// True if the `SendData` variant carries the on-the-wire shape OLE expects for `hint`'s mapped
/// clipboard format. Mismatches (e.g. `SendData::String` for a `UriList` hint) would otherwise
/// produce malformed `HGLOBAL` payloads (e.g. UTF-16 text mislabeled as `CF_HDROP`), which the
/// target would parse as wild offsets - memory corruption in the receiving process.
fn variant_matches_hint(hint: TypeHint, data: &SendData) -> bool {
    matches!(
        (hint, data),
        (TypeHint::UriList, SendData::Uris(_))
            | (TypeHint::Plaintext | TypeHint::Html | TypeHint::Rtf, SendData::String(_))
            | (TypeHint::Image { .. } | TypeHint::Audio { .. }, SendData::Bytes(_))
    )
}

// ---- IEnumFORMATETC: a cursor over a precomputed `Vec<FORMATETC>`. -----------

#[repr(C)]
#[allow(non_snake_case)]
struct IEnumFORMATETCInterface {
    lpVtbl: *const IEnumFORMATETCVtbl,
}

/// Generates the shared `IUnknown` boilerplate for our hand-rolled COM source-side objects.
///
/// Each such object is a `Box`-allocated `#[repr(C)]` struct whose first field is its COM
/// interface and which carries an `AtomicUsize` `refcount`. The thunks are identical across
/// objects apart from the concrete type and the one extra IID accepted by `QueryInterface`, so
/// the subtle refcount memory ordering lives in exactly one place.
macro_rules! com_iunknown_impl {
    ($ty:ty, $extra_iid:expr) => {
        #[allow(non_snake_case)]
        impl $ty {
            unsafe fn from_interface<'a, I>(this: *mut I) -> &'a mut Self {
                unsafe { &mut *(this as *mut _) }
            }

            unsafe extern "system" fn QueryInterface(
                this: *mut IUnknown,
                riid: *const GUID,
                ppv: *mut *mut c_void,
            ) -> HRESULT {
                let riid = unsafe { &*riid };
                if guids_eq(riid, &IID_IUnknown) || guids_eq(riid, $extra_iid) {
                    unsafe { *ppv = this as *mut c_void };
                    unsafe { Self::AddRef(this) };
                    S_OK
                } else {
                    unsafe { *ppv = std::ptr::null_mut() };
                    E_NOINTERFACE
                }
            }

            unsafe extern "system" fn AddRef(this: *mut IUnknown) -> u32 {
                let me = unsafe { Self::from_interface(this) };
                // Mere refcount bump - Relaxed is enough; the caller already holds a synchronized
                // reference to the object.
                me.refcount.fetch_add(1, Ordering::Relaxed) as u32 + 1
            }

            unsafe extern "system" fn Release(this: *mut IUnknown) -> u32 {
                let me = unsafe { Self::from_interface(this) };
                // Release on decrement publishes any writes made through this reference before the
                // count is observed by other threads. When we hit zero, fence with Acquire so the
                // destructor sees all writes from prior Releases on other threads - the standard
                // Arc pattern. Without the fence, dropping the box could race with reads done by
                // the last releasing thread on another core.
                let count = me.refcount.fetch_sub(1, Ordering::Release) - 1;
                if count == 0 {
                    atomic::fence(Ordering::Acquire);
                    drop(unsafe { Box::from_raw(me as *mut Self) });
                }
                count as u32
            }
        }
    };
}

#[repr(C)]
struct SourceFormatEnumerator {
    interface: IEnumFORMATETCInterface,
    refcount: AtomicUsize,
    formats: Vec<FORMATETC>,
    cursor: Cell<usize>,
}

com_iunknown_impl!(SourceFormatEnumerator, &IID_IEnumFORMATETC);

#[allow(non_snake_case)]
impl SourceFormatEnumerator {
    fn new_boxed(formats: Vec<FORMATETC>) -> *mut Self {
        Box::into_raw(Box::new(Self {
            interface: IEnumFORMATETCInterface {
                lpVtbl: &SOURCE_ENUM_FORMATETC_VTBL as *const IEnumFORMATETCVtbl,
            },
            refcount: AtomicUsize::new(1),
            formats,
            cursor: Cell::new(0),
        }))
    }

    unsafe extern "system" fn Next(
        this: *mut IEnumFORMATETC,
        celt: u32,
        rgelt: *mut FORMATETC,
        pcelt_fetched: *mut u32,
    ) -> HRESULT {
        let me = unsafe { Self::from_interface(this) };
        let cursor = me.cursor.get();
        let to_copy = (celt as usize).min(me.formats.len().saturating_sub(cursor));
        for i in 0..to_copy {
            unsafe { *rgelt.add(i) = me.formats[cursor + i] };
        }
        me.cursor.set(cursor + to_copy);
        if !pcelt_fetched.is_null() {
            unsafe { *pcelt_fetched = to_copy as u32 };
        }
        if to_copy < celt as usize { S_FALSE } else { S_OK }
    }

    unsafe extern "system" fn Skip(this: *mut IEnumFORMATETC, celt: u32) -> HRESULT {
        let me = unsafe { Self::from_interface(this) };
        let new_cursor = me.cursor.get().saturating_add(celt as usize);
        if new_cursor > me.formats.len() {
            me.cursor.set(me.formats.len());
            S_FALSE
        } else {
            me.cursor.set(new_cursor);
            S_OK
        }
    }

    unsafe extern "system" fn Reset(this: *mut IEnumFORMATETC) -> HRESULT {
        let me = unsafe { Self::from_interface(this) };
        me.cursor.set(0);
        S_OK
    }

    unsafe extern "system" fn Clone(
        this: *mut IEnumFORMATETC,
        ppenum: *mut *mut IEnumFORMATETC,
    ) -> HRESULT {
        let me = unsafe { Self::from_interface(this) };
        let cloned = SourceFormatEnumerator::new_boxed(me.formats.clone());
        unsafe { (*cloned).cursor.set(me.cursor.get()) };
        unsafe { *ppenum = cloned as *mut IEnumFORMATETC };
        S_OK
    }
}

static SOURCE_ENUM_FORMATETC_VTBL: IEnumFORMATETCVtbl = IEnumFORMATETCVtbl {
    parent: IUnknownVtbl {
        QueryInterface: SourceFormatEnumerator::QueryInterface,
        AddRef: SourceFormatEnumerator::AddRef,
        Release: SourceFormatEnumerator::Release,
    },
    Next: SourceFormatEnumerator::Next,
    Skip: SourceFormatEnumerator::Skip,
    Reset: SourceFormatEnumerator::Reset,
    Clone: SourceFormatEnumerator::Clone,
};

// ---- IDataObject (source) ---------------------------------------------------

#[repr(C)]
#[allow(non_snake_case)]
struct IDataObjectInterface {
    lpVtbl: *const IDataObjectVtbl,
}

/// Owning wrapper around a `STGMEDIUM` that releases the underlying handle on drop.
struct OwnedStgMedium(STGMEDIUM);

impl Drop for OwnedStgMedium {
    fn drop(&mut self) {
        unsafe { ReleaseStgMedium(&mut self.0) };
    }
}

#[repr(C)]
struct SourceDataObjectData {
    interface: IDataObjectInterface,
    refcount: AtomicUsize,
    send_data: RefCell<Box<dyn DataTransferSend>>,
    // (cf_format, hint) pairs we advertise to the target.
    formats: Vec<(u16, TypeHint)>,
    // Formats injected via `SetData` - primarily by `IDragSourceHelper::InitializeFromBitmap`,
    // which stores the drag image bits (CFSTR_DRAGIMAGEBITS) and related shell formats here so
    // the target-side `IDropTargetHelper` can read them back via `GetData`.
    extras: RefCell<Vec<(FORMATETC, OwnedStgMedium)>>,
}

com_iunknown_impl!(SourceDataObjectData, &IID_IDataObject);

#[allow(non_snake_case)]
impl SourceDataObjectData {
    fn new_boxed(send_data: Box<dyn DataTransferSend>) -> *mut Self {
        let mut formats: Vec<(u16, TypeHint)> = Vec::new();
        send_data.for_each_available_type(&mut |ty| {
            if let Some(hint) = ty.hint() {
                for (cf, specialised) in cf_formats_for_hint(hint) {
                    if !formats.iter().any(|(c, _)| *c == cf) {
                        formats.push((cf, specialised));
                    }
                }
            }
            ControlFlow::Continue(())
        });

        Box::into_raw(Box::new(Self {
            interface: IDataObjectInterface {
                lpVtbl: &SOURCE_DATA_OBJECT_VTBL as *const IDataObjectVtbl,
            },
            refcount: AtomicUsize::new(1),
            send_data: RefCell::new(send_data),
            formats,
            extras: RefCell::new(Vec::new()),
        }))
    }

    unsafe extern "system" fn GetData(
        this: *mut IDataObject,
        pformatetc_in: *const FORMATETC,
        pmedium: *mut STGMEDIUM,
    ) -> HRESULT {
        let me = unsafe { Self::from_interface(this) };
        let format = unsafe { &*pformatetc_in };
        if (format.tymed & TYMED_HGLOBAL as u32) == 0 {
            return DV_E_FORMATETC;
        }
        // Shell-helper-injected formats live in `extras`; serve those first by duplicating the
        // stored HGLOBAL so the caller can `ReleaseStgMedium` independently of our storage.
        if let Ok(extras) = me.extras.try_borrow() {
            if let Some((stored_fmt, stored)) =
                extras.iter().find(|(f, _)| f.cfFormat == format.cfFormat)
            {
                if (stored_fmt.tymed & format.tymed) != 0 {
                    if let Some(dup) = unsafe { duplicate_stgmedium(&stored.0, format.cfFormat) } {
                        unsafe { *pmedium = dup };
                        return S_OK;
                    }
                    return E_FAIL;
                }
            }
        }
        let Some(&(_, hint)) = me.formats.iter().find(|&&(cf, _)| cf == format.cfFormat) else {
            return DV_E_FORMATETC;
        };
        // `try_borrow_mut` rather than `borrow_mut`: `data_for_type` is the app's callback and
        // may itself reach back into this `IDataObject`. A panic across the `extern "system"`
        // boundary would be UB; return `E_UNEXPECTED` instead.
        let data = {
            let Ok(send_data) = me.send_data.try_borrow_mut() else {
                return E_UNEXPECTED;
            };
            send_data.data_for_type(&hint)
        };
        let Some(data) = data else {
            return DV_E_FORMATETC;
        };
        if !variant_matches_hint(hint, &data) {
            return DV_E_FORMATETC;
        }
        let Some(medium) = (unsafe { send_data_to_stgmedium(data, hint) }) else {
            return E_FAIL;
        };
        unsafe { *pmedium = medium };
        S_OK
    }

    unsafe extern "system" fn GetDataHere(
        _this: *mut IDataObject,
        _pformatetc: *const FORMATETC,
        _pmedium: *mut STGMEDIUM,
    ) -> HRESULT {
        E_NOTIMPL
    }

    unsafe extern "system" fn QueryGetData(
        this: *mut IDataObject,
        pformatetc: *const FORMATETC,
    ) -> HRESULT {
        let me = unsafe { Self::from_interface(this) };
        let format = unsafe { &*pformatetc };
        if (format.tymed & TYMED_HGLOBAL as u32) == 0 {
            return DV_E_FORMATETC;
        }
        if me.formats.iter().any(|&(cf, _)| cf == format.cfFormat) {
            return S_OK;
        }
        if let Ok(extras) = me.extras.try_borrow() {
            if extras.iter().any(|(f, _)| f.cfFormat == format.cfFormat) {
                return S_OK;
            }
        }
        S_FALSE
    }

    unsafe extern "system" fn GetCanonicalFormatEtc(
        _this: *mut IDataObject,
        _pformatetc_in: *const FORMATETC,
        _pformatetc_out: *mut FORMATETC,
    ) -> HRESULT {
        E_NOTIMPL
    }

    unsafe extern "system" fn SetData(
        this: *mut IDataObject,
        pformatetc: *const FORMATETC,
        pmedium: *const STGMEDIUM,
        f_release: BOOL,
    ) -> HRESULT {
        // Primary caller is `IDragSourceHelper::InitializeFromBitmap`, which attaches the drag
        // image bits and related shell formats to our data object. We don't interpret them -
        // just hold them so `GetData` can hand them back to `IDropTargetHelper`.
        let me = unsafe { Self::from_interface(this) };
        if pformatetc.is_null() || pmedium.is_null() {
            return E_FAIL;
        }
        let format = unsafe { *pformatetc };
        let medium = if f_release != 0 {
            // We take ownership of the passed-in medium as-is.
            unsafe { *pmedium }
        } else {
            // Caller retains ownership; we must duplicate.
            let Some(dup) = (unsafe { duplicate_stgmedium(&*pmedium, format.cfFormat) }) else {
                return E_FAIL;
            };
            dup
        };
        let Ok(mut extras) = me.extras.try_borrow_mut() else {
            if f_release != 0 {
                // We promised to take ownership but can't store - release immediately so we
                // don't leak.
                let mut m = medium;
                unsafe { ReleaseStgMedium(&mut m) };
            }
            return E_UNEXPECTED;
        };
        // Replace any earlier entry with the same format - last-write-wins matches what real
        // shell apps do and avoids growing the vec unboundedly on repeated SetData calls.
        if let Some(slot) = extras.iter_mut().find(|(f, _)| f.cfFormat == format.cfFormat) {
            slot.0 = format;
            slot.1 = OwnedStgMedium(medium);
        } else {
            extras.push((format, OwnedStgMedium(medium)));
        }
        S_OK
    }

    unsafe extern "system" fn EnumFormatEtc(
        this: *mut IDataObject,
        dw_direction: u32,
        ppenum: *mut *mut IEnumFORMATETC,
    ) -> HRESULT {
        const DATADIR_GET: u32 = 1;
        if dw_direction != DATADIR_GET {
            return E_NOTIMPL;
        }
        let me = unsafe { Self::from_interface(this) };
        let mut formats: Vec<FORMATETC> = me
            .formats
            .iter()
            .map(|&(cf, _)| FORMATETC {
                cfFormat: cf,
                ptd: std::ptr::null_mut(),
                dwAspect: DVASPECT_CONTENT,
                lindex: -1,
                tymed: TYMED_HGLOBAL as u32,
            })
            .collect();
        if let Ok(extras) = me.extras.try_borrow() {
            for (fmt, _) in extras.iter() {
                if !formats.iter().any(|f| f.cfFormat == fmt.cfFormat) {
                    formats.push(*fmt);
                }
            }
        }
        let enumerator = SourceFormatEnumerator::new_boxed(formats);
        unsafe { *ppenum = enumerator as *mut IEnumFORMATETC };
        S_OK
    }

    unsafe extern "system" fn DAdvise(
        _this: *mut IDataObject,
        _pformatetc: *const FORMATETC,
        _advf: u32,
        _adv_sink: *const crate::definitions::IAdviseSink,
        _pdw_connection: *mut u32,
    ) -> HRESULT {
        OLE_E_ADVISENOTSUPPORTED
    }

    unsafe extern "system" fn DUnadvise(_this: *mut IDataObject, _connection: u32) -> HRESULT {
        OLE_E_ADVISENOTSUPPORTED
    }

    unsafe extern "system" fn EnumDAdvise(
        _this: *mut IDataObject,
        _ppenum_advise: *const *const crate::definitions::IEnumSTATDATA,
    ) -> HRESULT {
        OLE_E_ADVISENOTSUPPORTED
    }
}

static SOURCE_DATA_OBJECT_VTBL: IDataObjectVtbl = IDataObjectVtbl {
    parent: IUnknownVtbl {
        QueryInterface: SourceDataObjectData::QueryInterface,
        AddRef: SourceDataObjectData::AddRef,
        Release: SourceDataObjectData::Release,
    },
    GetData: SourceDataObjectData::GetData,
    GetDataHere: SourceDataObjectData::GetDataHere,
    QueryGetData: SourceDataObjectData::QueryGetData,
    GetCanonicalFormatEtc: SourceDataObjectData::GetCanonicalFormatEtc,
    SetData: SourceDataObjectData::SetData,
    EnumFormatEtc: SourceDataObjectData::EnumFormatEtc,
    DAdvise: SourceDataObjectData::DAdvise,
    DUnadvise: SourceDataObjectData::DUnadvise,
    EnumDAdvise: SourceDataObjectData::EnumDAdvise,
};

pub(crate) struct SourceDataObject {
    data: *mut SourceDataObjectData,
}

impl SourceDataObject {
    pub(crate) fn new(send_data: Box<dyn DataTransferSend>) -> Self {
        Self { data: SourceDataObjectData::new_boxed(send_data) }
    }

    pub(crate) fn interface_ptr(&self) -> *mut c_void {
        self.data as *mut c_void
    }
}

impl Drop for SourceDataObject {
    fn drop(&mut self) {
        unsafe { SourceDataObjectData::Release(self.data as *mut IUnknown) };
    }
}

// ---- IDropSource ------------------------------------------------------------

#[repr(C)]
#[allow(non_snake_case)]
struct IDropSourceInterface {
    lpVtbl: *const IDropSourceVtbl,
}

#[repr(C)]
struct DropSourceData {
    interface: IDropSourceInterface,
    refcount: AtomicUsize,
}

com_iunknown_impl!(DropSourceData, &IID_IDropSource);

#[allow(non_snake_case)]
impl DropSourceData {
    fn new_boxed() -> *mut Self {
        Box::into_raw(Box::new(Self {
            interface: IDropSourceInterface { lpVtbl: &DROP_SOURCE_VTBL as *const IDropSourceVtbl },
            refcount: AtomicUsize::new(1),
        }))
    }

    unsafe extern "system" fn QueryContinueDrag(
        _this: *mut IDropSource,
        escape_pressed: BOOL,
        grf_key_state: u32,
    ) -> HRESULT {
        // Drop when no mouse button is pressed - matches Microsoft's documented
        // `IDropSource` example and covers right-button / middle-button drags as
        // well as the common left-button case. Hardcoding `MK_LBUTTON` would make
        // a right-drag (e.g. context-menu drag) never terminate by mouse release.
        const ANY_MOUSE_BUTTON: u32 =
            MK_LBUTTON | MK_RBUTTON | MK_MBUTTON | MK_XBUTTON1 | MK_XBUTTON2;
        if escape_pressed != 0 {
            return DRAGDROP_S_CANCEL;
        }
        if (grf_key_state & ANY_MOUSE_BUTTON) == 0 {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    unsafe extern "system" fn GiveFeedback(_this: *mut IDropSource, _dw_effect: u32) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

static DROP_SOURCE_VTBL: IDropSourceVtbl = IDropSourceVtbl {
    parent: IUnknownVtbl {
        QueryInterface: DropSourceData::QueryInterface,
        AddRef: DropSourceData::AddRef,
        Release: DropSourceData::Release,
    },
    QueryContinueDrag: DropSourceData::QueryContinueDrag,
    GiveFeedback: DropSourceData::GiveFeedback,
};

pub(crate) struct DropSource {
    data: *mut DropSourceData,
}

impl DropSource {
    pub(crate) fn new() -> Self {
        Self { data: DropSourceData::new_boxed() }
    }

    pub(crate) fn interface_ptr(&self) -> *mut c_void {
        self.data as *mut c_void
    }
}

impl Drop for DropSource {
    fn drop(&mut self) {
        unsafe { DropSourceData::Release(self.data as *mut IUnknown) };
    }
}

/// Attach a drag image to `data_object` so the shell renders the app's icon under the cursor
/// during the drag instead of the default no-image cursor.
///
/// `rgba` is the icon's pixel buffer in straight RGBA8 with `width * height * 4` bytes;
/// `offset` matches `DragIcon::offset` - `(0, 0)` sits the cursor at the icon's top-left,
/// `(-w/2, -h/2)` centres the icon on the cursor. Windows expresses this as the offset from
/// the icon's upper-left to the cursor hot spot (sign inverted), so we negate when filling
/// in `SHDRAGIMAGE::ptOffset`.
///
/// Returns `Ok(())` on success. On failure the caller's data object is left untouched and the
/// drag still runs - just without a custom image. We never propagate the error: a missing drag
/// preview is purely cosmetic and shouldn't fail the whole `start_drag`.
pub(crate) unsafe fn apply_drag_image(
    data_object: *mut IDataObject,
    width: u32,
    height: u32,
    rgba: &[u8],
    offset: dpi::PhysicalPosition<i32>,
) -> Result<(), HRESULT> {
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HDC,
    };
    use windows_sys::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
    use windows_sys::Win32::UI::Shell::{CLSID_DragDropHelper, SHDRAGIMAGE};

    use crate::definitions::{IDragSourceHelper, IDragSourceHelperVtbl, IID_IDragSourceHelper};

    if width == 0 || height == 0 || rgba.len() != (width as usize) * (height as usize) * 4 {
        return Err(E_FAIL);
    }

    // Build a top-down 32bpp BGRA DIB. Top-down means negative biHeight; the shell helper
    // expects BGRA byte order (B, G, R, A) with premultiplied alpha for smooth compositing.
    let mut header: BITMAPINFO = unsafe { std::mem::zeroed() };
    header.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width as i32,
        biHeight: -(height as i32),
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        biSizeImage: width * height * 4,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    let mut bits_ptr: *mut c_void = std::ptr::null_mut();
    let hbitmap = unsafe {
        CreateDIBSection(
            std::ptr::null_mut::<HDC>() as HDC,
            &header,
            DIB_RGB_COLORS,
            &mut bits_ptr,
            std::ptr::null_mut(),
            0,
        )
    };
    if hbitmap.is_null() || bits_ptr.is_null() {
        return Err(E_FAIL);
    }

    // Copy RGBA -> premultiplied BGRA so the shell can render the image with smooth alpha
    // edges. Without premultiplication, the cursor preview shows a halo on every translucent
    // pixel.
    let dst = unsafe { std::slice::from_raw_parts_mut(bits_ptr as *mut u8, rgba.len()) };
    for (src_px, dst_px) in rgba.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        let (r, g, b, a) = (src_px[0] as u32, src_px[1] as u32, src_px[2] as u32, src_px[3]);
        dst_px[0] = ((b * a as u32) / 255) as u8;
        dst_px[1] = ((g * a as u32) / 255) as u8;
        dst_px[2] = ((r * a as u32) / 255) as u8;
        dst_px[3] = a;
    }

    let mut helper: *mut IDragSourceHelper = std::ptr::null_mut();
    let hr = unsafe {
        CoCreateInstance(
            &CLSID_DragDropHelper,
            std::ptr::null_mut(),
            CLSCTX_ALL,
            &IID_IDragSourceHelper,
            &mut helper as *mut _ as *mut _,
        )
    };
    if hr < 0 || helper.is_null() {
        unsafe { DeleteObject(hbitmap as _) };
        return Err(hr);
    }

    // The helper API takes `ptOffset` as the cursor's position inside the image (positive
    // values into the image), but cross-platform `DragIcon::offset` is the icon-relative
    // offset where the cursor sits (negative values mean the icon extends up/left of the
    // cursor). Negate to translate between the two conventions.
    let sdi = SHDRAGIMAGE {
        sizeDragImage: windows_sys::Win32::Foundation::SIZE { cx: width as i32, cy: height as i32 },
        ptOffset: POINT { x: -offset.x, y: -offset.y },
        hbmpDragImage: hbitmap,
        crColorKey: 0xffff_ffff, // CLR_NONE - use the alpha channel
    };

    let vtbl = unsafe {
        &*((*(helper as *mut *mut IDragSourceHelperVtbl)) as *const IDragSourceHelperVtbl)
    };
    let init_hr = unsafe { (vtbl.InitializeFromBitmap)(helper, &sdi, data_object) };
    let release = vtbl.parent.Release;
    unsafe { release(helper as *mut IUnknown) };

    if init_hr < 0 {
        // On failure the helper did not take ownership of the bitmap - we still own it.
        unsafe { DeleteObject(hbitmap as _) };
        return Err(init_hr);
    }
    // On success the helper stores the bitmap on the data object and will delete it later;
    // do NOT call DeleteObject here.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_offset(buf: &[u8], name: &str) -> usize {
        let prefix = format!("{name}:");
        let head = std::str::from_utf8(buf).unwrap();
        let pos = head.find(&prefix).unwrap() + prefix.len();
        head[pos..pos + 10].parse().unwrap()
    }

    #[test]
    fn html_clipboard_format_brackets_user_html() {
        let html = "<span><strong>Winit</strong> example</span>";
        let buf = build_html_clipboard_format(html);

        assert!(buf.starts_with(b"Version:0.9\r\n"));

        let start_html = parse_offset(&buf, "StartHTML");
        let end_html = parse_offset(&buf, "EndHTML");
        let doc = std::str::from_utf8(&buf[start_html..end_html]).unwrap();
        assert!(doc.starts_with("<html><body>"));
        assert!(doc.ends_with("</body></html>"));

        let start_fragment = parse_offset(&buf, "StartFragment");
        let end_fragment = parse_offset(&buf, "EndFragment");
        let fragment = std::str::from_utf8(&buf[start_fragment..end_fragment]).unwrap();
        assert_eq!(fragment, html);
    }

    #[test]
    fn html_clipboard_format_preserves_pre_wrapped() {
        let pre_wrapped =
            "<html><body><!--StartFragment--><p>hi</p><!--EndFragment--></body></html>";
        let buf = build_html_clipboard_format(pre_wrapped);

        let start_fragment = parse_offset(&buf, "StartFragment");
        let end_fragment = parse_offset(&buf, "EndFragment");
        let fragment = std::str::from_utf8(&buf[start_fragment..end_fragment]).unwrap();
        assert_eq!(fragment, "<p>hi</p>");
    }
}
