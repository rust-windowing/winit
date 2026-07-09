use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::io;
use std::ops::{BitOr, ControlFlow};
use std::sync::{Arc, OnceLock};

use dispatch2::MainThreadBound;
use objc2::rc::{Retained, Weak};
use objc2::runtime::AnyObject;
use objc2::{AnyThread, DefinedClass as _, MainThreadMarker, Message, define_class, msg_send};
use objc2_app_kit::{
    NSDragOperation, NSPasteboard, NSPasteboardType, NSPasteboardTypeFileURL, NSPasteboardTypeHTML,
    NSPasteboardTypePNG, NSPasteboardTypeSound, NSPasteboardTypeString, NSPasteboardTypeTIFF,
    NSPasteboardWriting, NSPasteboardWritingOptions,
};
use objc2_foundation::{NSArray, NSData, NSObject, NSObjectProtocol, NSString};
use winit_core::data_transfer::{
    DataTransfer, DataTransferId, DataTransferSend, SendData, TransferType, TypeHint, TypedData,
};
use winit_core::event_loop::DndAction;
use winit_core::window::WindowId;

/// A thin wrapper around [`NSPasteboardType`], implementing [`TransferType`].
#[derive(PartialEq, Eq, Debug, Clone)]
pub struct PasteboardType {
    hint: Option<TypeHint>,
    // We need to convert `NSString` to `str` since `NSString` isn't `Send`/`Sync`
    inner: Arc<str>,
}

impl PasteboardType {
    fn from_hint(hint: TypeHint) -> Option<Self> {
        let hint_to_pasteboard_type = unsafe {
            [
                (TypeHint::UriList, NSPasteboardTypeFileURL),
                (TypeHint::Plaintext, NSPasteboardTypeString),
                (TypeHint::Html, NSPasteboardTypeHTML),
                (TypeHint::Image { extension_hint: Some("png") }, NSPasteboardTypePNG),
                (TypeHint::Image { extension_hint: Some("tiff") }, NSPasteboardTypeTIFF),
                (TypeHint::Audio { extension_hint: None }, NSPasteboardTypeSound),
            ]
        };

        hint_to_pasteboard_type.into_iter().find_map(|(haystack, inner)| {
            (haystack.matches(&hint))
                .then(|| Self { hint: Some(hint), inner: inner.to_string().into() })
        })
    }
}

impl From<Retained<NSPasteboardType>> for PasteboardType {
    fn from(value: Retained<NSPasteboardType>) -> Self {
        let pasteboard_type_to_hint = unsafe {
            [
                // Just in case the source application uses the deprecated method, we handle it
                // here
                #[expect(deprecated)]
                (objc2_app_kit::NSFilenamesPboardType, TypeHint::UriList),
                (NSPasteboardTypeFileURL, TypeHint::UriList),
                (NSPasteboardTypeString, TypeHint::Plaintext),
                (NSPasteboardTypeHTML, TypeHint::Html),
                (NSPasteboardTypePNG, TypeHint::Image { extension_hint: Some("png") }),
                (NSPasteboardTypeTIFF, TypeHint::Image { extension_hint: Some("tiff") }),
                (NSPasteboardTypeSound, TypeHint::Audio { extension_hint: None }),
            ]
        };

        let hint = pasteboard_type_to_hint
            .iter()
            .find_map(|(pb_type, hint)| (**pb_type == *value).then_some(hint));

        Self { hint: hint.copied(), inner: value.to_string().into() }
    }
}

impl TransferType for PasteboardType {
    fn hint(&self) -> Option<winit_core::data_transfer::TypeHint> {
        self.hint
    }

    fn matches(&self, other: &dyn TransferType) -> bool {
        if let Some(other_pb_type) = other.cast_ref::<Self>() {
            *self == *other_pb_type
        } else {
            // If either hint is `None`, return false
            self.hint().is_some_and(|hint| other.hint() == Some(hint))
        }
    }
}

/// A thin wrapper around [`NSPasteboard`], implementing [`DataTransfer`].
#[derive(Debug)]
pub struct Pasteboard {
    transfer_id: DataTransferId,
    ns_pasteboard: MainThreadBound<Retained<NSPasteboard>>,
    types: OnceLock<Arc<[PasteboardType]>>,
}

impl Clone for Pasteboard {
    fn clone(&self) -> Self {
        let inner = self.ns_pasteboard.get_on_main(|inner| {
            MainThreadBound::new(inner.clone(), MainThreadMarker::new().unwrap())
        });

        Self { transfer_id: self.transfer_id, ns_pasteboard: inner, types: self.types.clone() }
    }
}

impl Pasteboard {
    fn new(
        transfer_id: DataTransferId,
        ns_pasteboard: MainThreadBound<Retained<NSPasteboard>>,
    ) -> Self {
        Self { transfer_id, ns_pasteboard, types: Default::default() }
    }

    /// Get the array of [`PasteboardType`]s advertized by this [`Pasteboard`].
    pub fn types(&self) -> &[PasteboardType] {
        self.types.get_or_init(|| {
            self.ns_pasteboard.get_on_main(|pb| {
                pb.types()
                    .map(|types| types.into_iter().map(PasteboardType::from).collect::<Vec<_>>())
                    .unwrap_or_default()
                    .into()
            })
        })
    }

    /// Get the `DataTransferId` of this pasteboard.
    pub fn id(&self) -> DataTransferId {
        self.transfer_id
    }

    /// Get a typed reader for this pasteboard. This is only necessary in the cross-platform case,
    /// as a user downcasting to the platform-specific type can just access the `NSPasteboard`
    /// directly.
    pub(crate) fn with_type(&self, type_: PasteboardTypeSpec) -> PasteboardValue {
        PasteboardValue { type_, pasteboard: self.clone() }
    }
}

impl DataTransfer for Pasteboard {
    fn for_each_available_type<'this>(
        &'this self,
        func: &'_ mut dyn FnMut(&'this dyn TransferType) -> std::ops::ControlFlow<()>,
    ) {
        let _ = self.types().iter().map(|mime| mime as &dyn TransferType).try_for_each(func);
    }
}

#[derive(Debug, Clone)]
pub(crate) enum PasteboardTypeSpec {
    PasteboardType(PasteboardType),
    TypeHint(TypeHint),
}

impl PasteboardTypeSpec {
    pub(crate) fn from_dyn(type_: &dyn TransferType) -> Option<Self> {
        match type_.cast_ref::<PasteboardType>() {
            Some(pb_type) => Some(Self::PasteboardType(pb_type.clone())),
            None => type_.hint().map(Into::into),
        }
    }
}

impl From<TypeHint> for PasteboardTypeSpec {
    fn from(value: TypeHint) -> Self {
        match PasteboardType::from_hint(value) {
            Some(pb_type) => Self::PasteboardType(pb_type),
            None => Self::TypeHint(value),
        }
    }
}

impl PasteboardTypeSpec {
    fn pasteboard_type(&self) -> Option<&PasteboardType> {
        match self {
            PasteboardTypeSpec::PasteboardType(pasteboard_type) => Some(pasteboard_type),
            PasteboardTypeSpec::TypeHint(_) => None,
        }
    }
}

pub fn dnd_action_to_ns_drag_operation(value: DndAction) -> NSDragOperation {
    match value {
        DndAction::Copy => NSDragOperation::Copy,
        DndAction::Move => NSDragOperation::Move,
        DndAction::Link => NSDragOperation::Link,
        DndAction::Private => NSDragOperation::Private,
        _ => NSDragOperation::empty(),
    }
}

pub fn ns_drag_operation_to_dnd_action(value: NSDragOperation) -> Option<DndAction> {
    [
        (NSDragOperation::Copy, DndAction::Copy),
        (NSDragOperation::Move, DndAction::Move),
        (NSDragOperation::Link, DndAction::Link),
        (NSDragOperation::Private, DndAction::Private),
        // Sometimes the OS returns `Generic`, in which case we just fall back to `Copy`.
        (NSDragOperation::Generic, DndAction::Copy),
    ]
    .into_iter()
    .find_map(|(appkit, winit)| value.contains(appkit).then_some(winit))
}

pub fn dnd_actions_to_ns_drag_operation(value: &[DndAction]) -> NSDragOperation {
    value
        .iter()
        .copied()
        .map(dnd_action_to_ns_drag_operation)
        .fold(NSDragOperation::empty(), BitOr::bitor)
}

pub fn preferred_drag_operation(
    value: NSDragOperation,
    preference: &[DndAction],
) -> Option<DndAction> {
    preference
        .iter()
        .find(|action| value.intersects(dnd_action_to_ns_drag_operation(**action)))
        .copied()
}

/// A thin wrapper around [`NSPasteboard`], implementing [`TypedData`].
#[derive(Debug)]
pub struct PasteboardValue {
    // The concept of "top-level" types for a pasteboard doesn't always make sense on macOS due to
    // the use of `pasteboardItems`, so we allow using `TypeHint` instead to preserve the user's
    // intention.
    type_: PasteboardTypeSpec,
    pasteboard: Pasteboard,
}

impl TypedData for PasteboardValue {
    fn type_(&self) -> &dyn TransferType {
        match &self.type_ {
            PasteboardTypeSpec::PasteboardType(pasteboard_type) => {
                pasteboard_type as &dyn TransferType
            },
            PasteboardTypeSpec::TypeHint(type_hint) => type_hint,
        }
    }

    fn try_read(&self) -> Option<Box<dyn io::BufRead>> {
        self.try_as_bytes()
            .ok()
            .map(|bytes| Box::new(io::Cursor::new(bytes)) as Box<dyn io::BufRead>)
    }

    fn try_as_bytes(&self) -> io::Result<Vec<u8>> {
        let type_ = self.type_.clone();
        self.pasteboard
            .ns_pasteboard
            .get_on_main(|pasteboard| {
                let bytes =
                    pasteboard.dataForType(&NSString::from_str(&type_.pasteboard_type()?.inner))?;
                Some(bytes.to_vec())
            })
            .ok_or_else(|| {
                io::Error::other(format!(
                    "NSPasteboard doesn't advertise a binary representation for type {:?}",
                    self.type_
                ))
            })
    }

    fn try_as_uris(&self) -> io::Result<Vec<String>> {
        // TODO: We should probably use `readObjects`, need to check how that works.
        if self.type_().hint() != Some(TypeHint::UriList) {
            return Err(io::ErrorKind::InvalidData.into());
        }

        self.pasteboard.ns_pasteboard.get_on_main(|pasteboard| {
            let Some(items) = pasteboard.pasteboardItems() else {
                // The pasteboard didn't expose any items, so we try with the deprecated method.
                #[expect(deprecated)]
                let property_list = match pasteboard
                    .propertyListForType(unsafe { objc2_app_kit::NSFilenamesPboardType })
                {
                    Some(property_list) => property_list,
                    None => {
                        return pasteboard
                            .stringForType(unsafe { NSPasteboardTypeFileURL })
                            .map(|ns_str| vec![ns_str.to_string()])
                            .ok_or_else(|| io::ErrorKind::InvalidData.into());
                    },
                };

                let paths = property_list
                    .downcast::<NSArray>()
                    .unwrap()
                    .into_iter()
                    .map(|file| file.downcast::<NSString>().unwrap().to_string())
                    .collect();

                return Ok(paths);
            };

            Ok(items
                .into_iter()
                .filter_map(|item| item.stringForType(unsafe { NSPasteboardTypeFileURL }))
                .map(|ns_str| ns_str.to_string())
                .collect())
        })
    }

    fn try_as_string(&self) -> io::Result<String> {
        let type_ = self.type_.clone();

        self.pasteboard.ns_pasteboard.get_on_main(|pasteboard| {
            pasteboard
                .stringForType(&NSString::from_str(
                    &type_.pasteboard_type().ok_or(io::ErrorKind::InvalidData)?.inner,
                ))
                .map(|ns_str| ns_str.to_string())
                .ok_or_else(|| io::ErrorKind::InvalidData.into())
        })
    }
}

#[derive(Debug)]
struct ActivePasteboard {
    window_ids: Vec<WindowId>,
    pb: MainThreadBound<Weak<NSPasteboard>>,
}

#[derive(Debug, Default)]
pub struct Pasteboards {
    inner: RefCell<HashMap<DataTransferId, ActivePasteboard>>,
}

impl Pasteboards {
    pub fn remove_deloaded_pasteboards(&self) {
        self.inner.borrow_mut().retain(|_, ActivePasteboard { pb, .. }| {
            pb.get_on_main(|state| state.load().is_some())
        });
    }

    /// If the data transfer exists, update the pasteboard it points to.
    pub fn set_pasteboard(
        &self,
        id: DataTransferId,
        new_pb: &MainThreadBound<Retained<NSPasteboard>>,
    ) {
        let mut inner = self.inner.borrow_mut();
        if let Some(ActivePasteboard { pb, .. }) = inner.get_mut(&id) {
            *pb = new_pb.get_on_main(|pb| {
                MainThreadBound::new(Weak::from_retained(pb), MainThreadMarker::new().unwrap())
            });
        }
    }

    pub fn insert(
        &self,
        transfer_id: DataTransferId,
        pb: &MainThreadBound<Retained<NSPasteboard>>,
        window_id: WindowId,
    ) {
        self.inner
            .borrow_mut()
            .entry(transfer_id)
            .or_insert_with(|| {
                pb.get_on_main(move |pb| ActivePasteboard {
                    window_ids: vec![],
                    pb: MainThreadBound::new(
                        Weak::from_retained(pb),
                        MainThreadMarker::new().unwrap(),
                    ),
                })
            })
            .window_ids
            .push(window_id);
    }

    pub fn get(&self, id: DataTransferId) -> Option<Pasteboard> {
        self.inner.borrow().get(&id).and_then(|ActivePasteboard { pb, .. }| {
            pb.get_on_main(|state| {
                let pb = state.load()?;
                let pb = MainThreadBound::new(pb, MainThreadMarker::new().unwrap());
                Some(Pasteboard::new(id, pb))
            })
        })
    }

    /// This should almost always contain only a single window, but we allow multiple just
    /// to avoid silently swallowing errors.
    pub fn window_ids(&self, id: DataTransferId) -> Ref<'_, [WindowId]> {
        Ref::map(self.inner.borrow(), |borrow| {
            borrow
                .get(&id)
                .map(|active_pasteboard| &active_pasteboard.window_ids[..])
                .unwrap_or(&[])
        })
    }
}

pub(crate) struct PasteboardWriterState {
    data: Box<dyn DataTransferSend>,
    // The macOS drag-and-drop API has some confusing aspects when handling multi-drag. The best
    // we can really do is have the first element contain all the cross-platform items, and
    // any further items are file paths only.
    uri: Option<Retained<NSString>>,
    writable_types: Retained<NSArray<NSPasteboardType>>,
}

impl PasteboardWriter {
    pub(crate) fn new(
        value: Box<dyn DataTransferSend>,
        uri: Option<Retained<NSString>>,
    ) -> Retained<Self> {
        let mut writable_types = Vec::<Retained<NSPasteboardType>>::new();
        value.for_each_available_type(&mut |type_| {
            let Some(spec) = PasteboardTypeSpec::from_dyn(type_) else {
                return ControlFlow::Continue(());
            };

            let Some(pb_type) = spec.pasteboard_type() else {
                return ControlFlow::Continue(());
            };

            writable_types.push(NSString::from_str(&pb_type.inner));

            ControlFlow::Continue(())
        });

        let pb_writer = Self::alloc().set_ivars(PasteboardWriterState {
            data: value,
            uri,
            writable_types: NSArray::from_retained_slice(&writable_types),
        });

        // Unsure if there's an easier way to do this, but this is how `WindowDelegate` does it.
        unsafe { msg_send![super(pb_writer), init] }
    }
}

impl PasteboardWriterState {
    fn data_for_pasteboard_type(
        &self,
        pasteboard_type: &NSPasteboardType,
    ) -> Option<Retained<AnyObject>> {
        if pasteboard_type == unsafe { NSPasteboardTypeFileURL } {
            if let Some(out) = self.uri.clone().map(Into::into) {
                return Some(out);
            }
        }
        let pb_type = PasteboardType::from(pasteboard_type.retain());

        let mut out = None;

        self.data.for_each_available_type(&mut |haystack| {
            if haystack.matches(&pb_type) {
                out = self.data.data_for_type(haystack);
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });

        match out? {
            // This should be handled separately
            // TODO: Is there a better way to do this?
            SendData::Uris(_) => None,
            SendData::String(string) => Some(NSString::from_str(&string).into()),
            SendData::Bytes(binary) => Some(NSData::from_vec(binary).into()),
        }
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = AnyThread]
    #[name = "WinitPasteboardWriter"]
    #[ivars = PasteboardWriterState]
    pub(crate) struct PasteboardWriter;

    unsafe impl NSObjectProtocol for PasteboardWriter {}

    unsafe impl NSPasteboardWriting for PasteboardWriter {
        #[unsafe(method_id(writableTypesForPasteboard:))]
        fn writable_types_for_pasteboard(
            &self,
            _: &NSPasteboard,
        ) -> Retained<NSArray<NSPasteboardType>> {
            let vars = self.ivars();
            vars.writable_types.clone()
        }

        #[unsafe(method(writingOptionsForType:pasteboard:))]
        fn writing_options_for_type(
            &self,
            type_: &NSPasteboardType,
            pasteboard: &NSPasteboard,
        ) -> NSPasteboardWritingOptions {
            let _ = type_;
            let _ = pasteboard;
            NSPasteboardWritingOptions::empty()
        }

        #[unsafe(method_id(pasteboardPropertyListForType:))]
        fn pasteboard_property_list_for_type(
            &self,
            type_: &NSPasteboardType,
        ) -> Option<Retained<AnyObject>> {
            let vars = self.ivars();
            vars.data_for_pasteboard_type(type_)
        }
    }
);
