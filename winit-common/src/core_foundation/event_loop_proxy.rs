use std::os::raw::c_void;
use std::ptr::NonNull;
use std::sync::Arc;
use std::task::{RawWaker, RawWakerVTable, Waker};

use objc2::MainThreadMarker;
use objc2_core_foundation::{
    kCFRunLoopCommonModes, CFIndex, CFRetained, CFRunLoop, CFRunLoopSource, CFRunLoopSourceContext,
    Type,
};
use winit_core::event_loop::EventLoopProxyProvider;

/// A waker that signals a `CFRunLoopSource` on the main thread.
///
/// We use this to integrate with the system as cleanly as possible (instead of e.g. keeping an
/// atomic around that we check on each iteration of the event loop).
///
/// See <https://developer.apple.com/documentation/corefoundation/cfrunloopsource?language=objc>.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EventLoopProxy {
    source: CFRetained<CFRunLoopSource>,
    /// Cached value of `CFRunLoopGetMain`.
    main_loop: CFRetained<CFRunLoop>,
}

// FIXME(madsmtm): Mark `CFRunLoopSource` + `CFRunLoop` as `Send` + `Sync`.
unsafe impl Send for EventLoopProxy {}
unsafe impl Sync for EventLoopProxy {}

impl EventLoopProxy {
    /// Create a new proxy, registering it to be performed on the main thread.
    ///
    /// The provided closure should call `proxy_wake_up` on the application.
    pub fn new<F: Fn() + 'static>(mtm: MainThreadMarker, signaller: F) -> Self {
        // We use an `Arc` here to make sure that the reference-counting of the signal container is
        // atomic (`Retained`/`CFRetained` would be valid alternatives too).
        let signaller = Arc::new(signaller);

        unsafe extern "C-unwind" fn retain<F>(info: *const c_void) -> *const c_void {
            // SAFETY: The pointer was passed to `CFRunLoopSourceContext.info` below.
            unsafe { Arc::increment_strong_count(info.cast::<F>()) };
            info
        }
        unsafe extern "C-unwind" fn release<F>(info: *const c_void) {
            // SAFETY: The pointer was passed to `CFRunLoopSourceContext.info` below.
            unsafe { Arc::decrement_strong_count(info.cast::<F>()) };
        }

        // Pointer equality / hashing.
        extern "C-unwind" fn equal(info1: *const c_void, info2: *const c_void) -> u8 {
            (info1 == info2) as u8
        }
        extern "C-unwind" fn hash(info: *const c_void) -> usize {
            info as usize
        }

        // Call the provided closure.
        unsafe extern "C-unwind" fn perform<F: Fn()>(info: *mut c_void) {
            // SAFETY: The pointer was passed to `CFRunLoopSourceContext.info` below.
            let signaller = unsafe { &*info.cast::<F>() };
            (signaller)();
        }

        // Fire last.
        let order = CFIndex::MAX - 1;

        // This is marked `mut` to match the signature of `CFRunLoopSourceCreate`, but the
        // information is copied, and not actually mutated.
        let mut context = CFRunLoopSourceContext {
            version: 0,
            // This is retained on creation.
            info: Arc::as_ptr(&signaller) as *mut c_void,
            retain: Some(retain::<F>),
            release: Some(release::<F>),
            copyDescription: None,
            equal: Some(equal),
            hash: Some(hash),
            schedule: None,
            cancel: None,
            perform: Some(perform::<F>),
        };

        // SAFETY: The normal callbacks are thread-safe (`retain`/`release` use atomics, and
        // `equal`/`hash` only access a pointer).
        //
        // Note that the `perform` callback isn't thread-safe (we don't have `F: Send + Sync`), but
        // that's okay, since we are on the main thread, and the source is only added to the main
        // run loop (below), and hence only performed there.
        //
        // Keeping the closure alive beyond this scope is fine, because `F: 'static`.
        let source = unsafe {
            let _ = mtm;
            CFRunLoopSource::new(None, order, &mut context).unwrap()
        };

        // Register the source to be performed on the main thread.
        let main_loop = CFRunLoop::main().unwrap();
        unsafe { main_loop.add_source(Some(&source), kCFRunLoopCommonModes) };

        Self { source, main_loop }
    }

    // FIXME(madsmtm): Use this on macOS too.
    // More difficult there, since the user can re-start the event loop.
    pub fn invalidate(&self) {
        // NOTE: We do NOT fire this on `Drop`, since we want the proxy to be cloneable, such that
        // we only need to register a single source even if there's multiple proxies in use.
        self.source.invalidate();
    }
}

impl EventLoopProxyProvider for EventLoopProxy {
    fn wake_up(&self) {
        // Signal the source, which ends up later invoking `perform` on the main thread.
        //
        // Multiple signals in quick succession are automatically coalesced into a single signal.
        self.source.signal();

        // Let the main thread know there's a new event.
        //
        // This is required since we may be (probably are) running on a different thread, and the
        // main loop may be sleeping (and `CFRunLoopSourceSignal` won't wake it).
        self.main_loop.wake_up();
    }

    fn waker(&self) -> Waker {
        const VTABLE: RawWakerVTable =
            RawWakerVTable::new(clone_waker, wake, wake_by_ref, drop_waker);

        unsafe fn clone_waker(data: *const ()) -> RawWaker {
            // SAFETY: The poiner came from `CFRunLoopSource` and is valid and non-null.
            let source = unsafe { &*data.cast::<CFRunLoopSource>() };

            // Increment reference count.
            let source = source.retain();

            // Pass ownership to the raw waker.
            let data: *const CFRunLoopSource = CFRetained::into_raw(source).as_ptr();
            RawWaker::new(data.cast(), &VTABLE)
        }

        unsafe fn wake(data: *const ()) {
            unsafe { wake_by_ref(data) };
            unsafe { drop_waker(data) };
        }

        unsafe fn wake_by_ref(data: *const ()) {
            // SAFETY: The poiner came from `CFRunLoopSource` and is valid and non-null.
            let source = unsafe { &*data.cast::<CFRunLoopSource>() };

            // Signal the source, which ends up later invoking `perform` on the main thread.
            //
            // Multiple signals in quick succession are automatically coalesced into a single
            // signal.
            source.signal();

            let main_loop = CFRunLoop::main().unwrap();
            // Let the main thread know there's a new event.
            //
            // This is required since we may be (probably are) running on a different thread, and
            // the main loop may be sleeping (and `CFRunLoopSourceSignal` won't wake it).
            main_loop.wake_up();
        }

        unsafe fn drop_waker(data: *const ()) {
            let source = data.cast::<CFRunLoopSource>().cast_mut();
            // SAFETY: The poiner came from `CFRunLoopSource` and is valid and non-null.
            // We take ownership of a retain count here.
            let _source = unsafe { CFRetained::from_raw(NonNull::new_unchecked(source)) };
        }

        let data: *const CFRunLoopSource = CFRetained::into_raw(self.source.clone()).as_ptr();
        unsafe { Waker::from_raw(RawWaker::new(data.cast(), &VTABLE)) }
    }
}
