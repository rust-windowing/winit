#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

use std::{convert::TryInto, ffi::CString, ops::BitOr, os::raw::*};

use objc::{runtime::Object, Encode, Encoding};

use crate::{
    dpi::LogicalSize,
    platform::ios::{Idiom, ScreenEdge, ValidOrientations},
};

pub type id = *mut Object;
pub const nil: id = 0 as id;

#[cfg(target_pointer_width = "32")]
pub type CGFloat = f32;
#[cfg(target_pointer_width = "64")]
pub type CGFloat = f64;

pub type NSInteger = isize;
pub type NSUInteger = usize;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct NSOperatingSystemVersion {
    pub major: NSInteger,
    pub minor: NSInteger,
    pub patch: NSInteger,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGSize {
    pub width: CGFloat,
    pub height: CGFloat,
}

impl CGSize {
    pub fn new(size: LogicalSize<f64>) -> CGSize {
        CGSize {
            width: size.width as _,
            height: size.height as _,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CGRect {
    pub origin: CGPoint,
    pub size: CGSize,
}

impl CGRect {
    pub fn new(origin: CGPoint, size: CGSize) -> CGRect {
        CGRect { origin, size }
    }
}

unsafe impl Encode for CGRect {
    fn encode() -> Encoding {
        unsafe {
            if cfg!(target_pointer_width = "32") {
                Encoding::from_str("{CGRect={CGPoint=ff}{CGSize=ff}}")
            } else if cfg!(target_pointer_width = "64") {
                Encoding::from_str("{CGRect={CGPoint=dd}{CGSize=dd}}")
            } else {
                unimplemented!()
            }
        }
    }
}
#[derive(Debug)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UITouchPhase {
    Began = 0,
    Moved,
    Stationary,
    Ended,
    Cancelled,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UIForceTouchCapability {
    Unknown = 0,
    Unavailable,
    Available,
}

#[derive(Debug, PartialEq, Eq)]
#[allow(dead_code)]
#[repr(isize)]
pub enum UITouchType {
    Direct = 0,
    Indirect,
    Pencil,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct UIEdgeInsets {
    pub top: CGFloat,
    pub left: CGFloat,
    pub bottom: CGFloat,
    pub right: CGFloat,
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIUserInterfaceIdiom(NSInteger);

unsafe impl Encode for UIUserInterfaceIdiom {
    fn encode() -> Encoding {
        NSInteger::encode()
    }
}

impl UIUserInterfaceIdiom {
    pub const Unspecified: UIUserInterfaceIdiom = UIUserInterfaceIdiom(-1);
    pub const Phone: UIUserInterfaceIdiom = UIUserInterfaceIdiom(0);
    pub const Pad: UIUserInterfaceIdiom = UIUserInterfaceIdiom(1);
    pub const TV: UIUserInterfaceIdiom = UIUserInterfaceIdiom(2);
    pub const CarPlay: UIUserInterfaceIdiom = UIUserInterfaceIdiom(3);
}

impl From<Idiom> for UIUserInterfaceIdiom {
    fn from(idiom: Idiom) -> UIUserInterfaceIdiom {
        match idiom {
            Idiom::Unspecified => UIUserInterfaceIdiom::Unspecified,
            Idiom::Phone => UIUserInterfaceIdiom::Phone,
            Idiom::Pad => UIUserInterfaceIdiom::Pad,
            Idiom::TV => UIUserInterfaceIdiom::TV,
            Idiom::CarPlay => UIUserInterfaceIdiom::CarPlay,
        }
    }
}
impl From<UIUserInterfaceIdiom> for Idiom {
    fn from(ui_idiom: UIUserInterfaceIdiom) -> Idiom {
        match ui_idiom {
            UIUserInterfaceIdiom::Unspecified => Idiom::Unspecified,
            UIUserInterfaceIdiom::Phone => Idiom::Phone,
            UIUserInterfaceIdiom::Pad => Idiom::Pad,
            UIUserInterfaceIdiom::TV => Idiom::TV,
            UIUserInterfaceIdiom::CarPlay => Idiom::CarPlay,
            _ => unreachable!(),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug)]
pub struct UIInterfaceOrientationMask(NSUInteger);

unsafe impl Encode for UIInterfaceOrientationMask {
    fn encode() -> Encoding {
        NSUInteger::encode()
    }
}

impl UIInterfaceOrientationMask {
    pub const Portrait: UIInterfaceOrientationMask = UIInterfaceOrientationMask(1 << 1);
    pub const PortraitUpsideDown: UIInterfaceOrientationMask = UIInterfaceOrientationMask(1 << 2);
    pub const LandscapeLeft: UIInterfaceOrientationMask = UIInterfaceOrientationMask(1 << 4);
    pub const LandscapeRight: UIInterfaceOrientationMask = UIInterfaceOrientationMask(1 << 3);
    pub const Landscape: UIInterfaceOrientationMask =
        UIInterfaceOrientationMask(Self::LandscapeLeft.0 | Self::LandscapeRight.0);
    pub const AllButUpsideDown: UIInterfaceOrientationMask =
        UIInterfaceOrientationMask(Self::Landscape.0 | Self::Portrait.0);
    pub const All: UIInterfaceOrientationMask =
        UIInterfaceOrientationMask(Self::AllButUpsideDown.0 | Self::PortraitUpsideDown.0);
}

impl BitOr for UIInterfaceOrientationMask {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        UIInterfaceOrientationMask(self.0 | rhs.0)
    }
}

impl UIInterfaceOrientationMask {
    pub fn from_valid_orientations_idiom(
        valid_orientations: ValidOrientations,
        idiom: Idiom,
    ) -> UIInterfaceOrientationMask {
        match (valid_orientations, idiom) {
            (ValidOrientations::LandscapeAndPortrait, Idiom::Phone) => {
                UIInterfaceOrientationMask::AllButUpsideDown
            }
            (ValidOrientations::LandscapeAndPortrait, _) => UIInterfaceOrientationMask::All,
            (ValidOrientations::Landscape, _) => UIInterfaceOrientationMask::Landscape,
            (ValidOrientations::Portrait, Idiom::Phone) => UIInterfaceOrientationMask::Portrait,
            (ValidOrientations::Portrait, _) => {
                UIInterfaceOrientationMask::Portrait
                    | UIInterfaceOrientationMask::PortraitUpsideDown
            }
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIRectEdge(NSUInteger);

unsafe impl Encode for UIRectEdge {
    fn encode() -> Encoding {
        NSUInteger::encode()
    }
}

impl From<ScreenEdge> for UIRectEdge {
    fn from(screen_edge: ScreenEdge) -> UIRectEdge {
        assert_eq!(
            screen_edge.bits() & !ScreenEdge::ALL.bits(),
            0,
            "invalid `ScreenEdge`"
        );
        UIRectEdge(screen_edge.bits().into())
    }
}

impl From<UIRectEdge> for ScreenEdge {
    fn from(ui_rect_edge: UIRectEdge) -> ScreenEdge {
        let bits: u8 = ui_rect_edge.0.try_into().expect("invalid `UIRectEdge`");
        ScreenEdge::from_bits(bits).expect("invalid `ScreenEdge`")
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UIScreenOverscanCompensation(NSInteger);

unsafe impl Encode for UIScreenOverscanCompensation {
    fn encode() -> Encoding {
        NSInteger::encode()
    }
}

#[allow(dead_code)]
impl UIScreenOverscanCompensation {
    pub const Scale: UIScreenOverscanCompensation = UIScreenOverscanCompensation(0);
    pub const InsetBounds: UIScreenOverscanCompensation = UIScreenOverscanCompensation(1);
    pub const None: UIScreenOverscanCompensation = UIScreenOverscanCompensation(2);
}

#[link(name = "UIKit", kind = "framework")]
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub static kCFRunLoopDefaultMode: CFRunLoopMode;
    pub static kCFRunLoopCommonModes: CFRunLoopMode;

    pub fn UIApplicationMain(
        argc: c_int,
        argv: *const c_char,
        principalClassName: id,
        delegateClassName: id,
    ) -> c_int;

    pub fn CFRunLoopGetMain() -> CFRunLoopRef;
    pub fn CFRunLoopWakeUp(rl: CFRunLoopRef);

    pub fn CFRunLoopObserverCreate(
        allocator: CFAllocatorRef,
        activities: CFOptionFlags,
        repeats: Boolean,
        order: CFIndex,
        callout: CFRunLoopObserverCallBack,
        context: *mut CFRunLoopObserverContext,
    ) -> CFRunLoopObserverRef;
    pub fn CFRunLoopAddObserver(
        rl: CFRunLoopRef,
        observer: CFRunLoopObserverRef,
        mode: CFRunLoopMode,
    );

    pub fn CFRunLoopTimerCreate(
        allocator: CFAllocatorRef,
        fireDate: CFAbsoluteTime,
        interval: CFTimeInterval,
        flags: CFOptionFlags,
        order: CFIndex,
        callout: CFRunLoopTimerCallBack,
        context: *mut CFRunLoopTimerContext,
    ) -> CFRunLoopTimerRef;
    pub fn CFRunLoopAddTimer(rl: CFRunLoopRef, timer: CFRunLoopTimerRef, mode: CFRunLoopMode);
    pub fn CFRunLoopTimerSetNextFireDate(timer: CFRunLoopTimerRef, fireDate: CFAbsoluteTime);
    pub fn CFRunLoopTimerInvalidate(time: CFRunLoopTimerRef);

    pub fn CFRunLoopSourceCreate(
        allocator: CFAllocatorRef,
        order: CFIndex,
        context: *mut CFRunLoopSourceContext,
    ) -> CFRunLoopSourceRef;
    pub fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFRunLoopMode);
    pub fn CFRunLoopSourceInvalidate(source: CFRunLoopSourceRef);
    pub fn CFRunLoopSourceSignal(source: CFRunLoopSourceRef);

    pub fn CFAbsoluteTimeGetCurrent() -> CFAbsoluteTime;
    pub fn CFRelease(cftype: *const c_void);
}

pub type Boolean = u8;
pub enum CFAllocator {}
pub type CFAllocatorRef = *mut CFAllocator;
pub enum CFRunLoop {}
pub type CFRunLoopRef = *mut CFRunLoop;
pub type CFRunLoopMode = CFStringRef;
pub enum CFRunLoopObserver {}
pub type CFRunLoopObserverRef = *mut CFRunLoopObserver;
pub enum CFRunLoopTimer {}
pub type CFRunLoopTimerRef = *mut CFRunLoopTimer;
pub enum CFRunLoopSource {}
pub type CFRunLoopSourceRef = *mut CFRunLoopSource;
pub enum CFString {}
pub type CFStringRef = *const CFString;

pub type CFHashCode = c_ulong;
pub type CFIndex = c_long;
pub type CFOptionFlags = c_ulong;
pub type CFRunLoopActivity = CFOptionFlags;

pub type CFAbsoluteTime = CFTimeInterval;
pub type CFTimeInterval = f64;

pub const kCFRunLoopEntry: CFRunLoopActivity = 0;
pub const kCFRunLoopBeforeWaiting: CFRunLoopActivity = 1 << 5;
pub const kCFRunLoopAfterWaiting: CFRunLoopActivity = 1 << 6;
pub const kCFRunLoopExit: CFRunLoopActivity = 1 << 7;

pub type CFRunLoopObserverCallBack =
    extern "C" fn(observer: CFRunLoopObserverRef, activity: CFRunLoopActivity, info: *mut c_void);
pub type CFRunLoopTimerCallBack = extern "C" fn(timer: CFRunLoopTimerRef, info: *mut c_void);

pub enum CFRunLoopObserverContext {}
pub enum CFRunLoopTimerContext {}

#[repr(C)]
pub struct CFRunLoopSourceContext {
    pub version: CFIndex,
    pub info: *mut c_void,
    pub retain: Option<extern "C" fn(*const c_void) -> *const c_void>,
    pub release: Option<extern "C" fn(*const c_void)>,
    pub copyDescription: Option<extern "C" fn(*const c_void) -> CFStringRef>,
    pub equal: Option<extern "C" fn(*const c_void, *const c_void) -> Boolean>,
    pub hash: Option<extern "C" fn(*const c_void) -> CFHashCode>,
    pub schedule: Option<extern "C" fn(*mut c_void, CFRunLoopRef, CFRunLoopMode)>,
    pub cancel: Option<extern "C" fn(*mut c_void, CFRunLoopRef, CFRunLoopMode)>,
    pub perform: Option<extern "C" fn(*mut c_void)>,
}

// This is named NSStringRust rather than NSString because the "Debug View Heirarchy" feature of
// Xcode requires a non-ambiguous reference to NSString for unclear reasons. This makes Xcode happy
// so please test if you change the name back to NSString.
pub trait NSStringRust: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSString), alloc]
    }

    unsafe fn initWithUTF8String_(self, c_string: *const c_char) -> id;
    unsafe fn stringByAppendingString_(self, other: id) -> id;
    unsafe fn init_str(self, string: &str) -> Self;
    unsafe fn UTF8String(self) -> *const c_char;
}

impl NSStringRust for id {
    unsafe fn initWithUTF8String_(self, c_string: *const c_char) -> id {
        msg_send![self, initWithUTF8String: c_string]
    }

    unsafe fn stringByAppendingString_(self, other: id) -> id {
        msg_send![self, stringByAppendingString: other]
    }

    unsafe fn init_str(self, string: &str) -> id {
        let cstring = CString::new(string).unwrap();
        self.initWithUTF8String_(cstring.as_ptr())
    }

    unsafe fn UTF8String(self) -> *const c_char {
        msg_send![self, UTF8String]
    }
}
