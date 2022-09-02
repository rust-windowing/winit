use objc2::encode::{Encode, Encoding};
use objc2::foundation::{
    CGFloat, NSArray, NSInteger, NSObject, NSPoint, NSRect, NSSize, NSString, NSUInteger,
};
use objc2::rc::{Id, Shared};
use objc2::runtime::Object;
use objc2::{extern_class, extern_methods, msg_send_id, ClassType};

use super::{NSButton, NSColor, NSEvent, NSPasteboardType, NSResponder, NSScreen, NSView};

extern_class!(
    /// Main-Thread-Only!
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub(crate) struct NSWindow;

    unsafe impl ClassType for NSWindow {
        #[inherits(NSObject)]
        type Super = NSResponder;
    }
);

// Documented as "Main Thread Only", but:
// > Thread safe in that you can create and manage them on a secondary thread.
// <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/CocoaFundamentals/AddingBehaviortoaCocoaProgram/AddingBehaviorCocoa.html#//apple_ref/doc/uid/TP40002974-CH5-SW47>
// <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html#//apple_ref/doc/uid/10000057i-CH12-123364>
//
// So could in theory be `Send`, and perhaps also `Sync` - but we would like
// interior mutability on windows, since that's just much easier, and in that
// case, they can't be!

extern_methods!(
    unsafe impl NSWindow {
        #[sel(frame)]
        pub fn frame(&self) -> NSRect;

        #[sel(backingScaleFactor)]
        pub fn backingScaleFactor(&self) -> CGFloat;

        pub fn contentView(&self) -> Id<NSView, Shared> {
            unsafe { msg_send_id![self, contentView] }
        }

        #[sel(setContentView:)]
        pub fn setContentView(&self, view: &NSView);

        #[sel(setInitialFirstResponder:)]
        pub fn setInitialFirstResponder(&self, view: &NSView);

        #[sel(makeFirstResponder:)]
        #[must_use]
        pub fn makeFirstResponder(&self, responder: Option<&NSResponder>) -> bool;

        #[sel(contentRectForFrameRect:)]
        pub fn contentRectForFrameRect(&self, windowFrame: NSRect) -> NSRect;

        pub fn screen(&self) -> Option<Id<NSScreen, Shared>> {
            unsafe { msg_send_id![self, screen] }
        }

        #[sel(setContentSize:)]
        pub fn setContentSize(&self, contentSize: NSSize);

        #[sel(setFrameTopLeftPoint:)]
        pub fn setFrameTopLeftPoint(&self, point: NSPoint);

        #[sel(setMinSize:)]
        pub fn setMinSize(&self, minSize: NSSize);

        #[sel(setMaxSize:)]
        pub fn setMaxSize(&self, maxSize: NSSize);

        #[sel(setResizeIncrements:)]
        pub fn setResizeIncrements(&self, increments: NSSize);

        #[sel(setFrame:display:)]
        pub fn setFrame_display(&self, frameRect: NSRect, flag: bool);

        #[sel(setMovable:)]
        pub fn setMovable(&self, movable: bool);

        #[sel(setOpaque:)]
        pub fn setOpaque(&self, opaque: bool);

        #[sel(hasShadow)]
        pub fn hasShadow(&self) -> bool;

        #[sel(setHasShadow:)]
        pub fn setHasShadow(&self, has_shadow: bool);

        #[sel(setIgnoresMouseEvents:)]
        pub fn setIgnoresMouseEvents(&self, ignores: bool);

        #[sel(setBackgroundColor:)]
        pub fn setBackgroundColor(&self, color: &NSColor);

        #[sel(styleMask)]
        pub fn styleMask(&self) -> NSWindowStyleMask;

        #[sel(setStyleMask:)]
        pub fn setStyleMask(&self, mask: NSWindowStyleMask);

        #[sel(registerForDraggedTypes:)]
        pub fn registerForDraggedTypes(&self, types: &NSArray<NSPasteboardType>);

        #[sel(makeKeyAndOrderFront:)]
        pub fn makeKeyAndOrderFront(&self, sender: Option<&Object>);

        #[sel(miniaturize:)]
        pub fn miniaturize(&self, sender: Option<&Object>);

        #[sel(sender:)]
        pub fn deminiaturize(&self, sender: Option<&Object>);

        #[sel(toggleFullScreen:)]
        pub fn toggleFullScreen(&self, sender: Option<&Object>);

        #[sel(orderOut:)]
        pub fn orderOut(&self, sender: Option<&Object>);

        #[sel(zoom:)]
        pub fn zoom(&self, sender: Option<&Object>);

        #[sel(selectNextKeyView:)]
        pub fn selectNextKeyView(&self, sender: Option<&Object>);

        #[sel(selectPreviousKeyView:)]
        pub fn selectPreviousKeyView(&self, sender: Option<&Object>);

        pub fn firstResponder(&self) -> Option<Id<NSResponder, Shared>> {
            unsafe { msg_send_id![self, firstResponder] }
        }

        pub fn standardWindowButton(&self, kind: NSWindowButton) -> Option<Id<NSButton, Shared>> {
            unsafe { msg_send_id![self, standardWindowButton: kind] }
        }

        #[sel(setTitle:)]
        pub fn setTitle(&self, title: &NSString);

        #[sel(setReleasedWhenClosed:)]
        pub fn setReleasedWhenClosed(&self, val: bool);

        #[sel(setAcceptsMouseMovedEvents:)]
        pub fn setAcceptsMouseMovedEvents(&self, val: bool);

        #[sel(setTitlebarAppearsTransparent:)]
        pub fn setTitlebarAppearsTransparent(&self, val: bool);

        #[sel(setTitleVisibility:)]
        pub fn setTitleVisibility(&self, visibility: NSWindowTitleVisibility);

        #[sel(setMovableByWindowBackground:)]
        pub fn setMovableByWindowBackground(&self, val: bool);

        #[sel(setLevel:)]
        pub fn setLevel(&self, level: NSWindowLevel);

        #[sel(occlusionState)]
        pub fn occlusionState(&self) -> NSWindowOcclusionState;

        #[sel(center)]
        pub fn center(&self);

        #[sel(isResizable)]
        pub fn isResizable(&self) -> bool;

        #[sel(isMiniaturized)]
        pub fn isMiniaturized(&self) -> bool;

        #[sel(isVisible)]
        pub fn isVisible(&self) -> bool;

        #[sel(isZoomed)]
        pub fn isZoomed(&self) -> bool;

        #[sel(close)]
        pub fn close(&self);

        #[sel(performWindowDragWithEvent:)]
        // TODO: Can this actually accept NULL?
        pub fn performWindowDragWithEvent(&self, event: Option<&NSEvent>);

        #[sel(invalidateCursorRectsForView:)]
        pub fn invalidateCursorRectsForView(&self, view: &NSView);

        #[sel(setDelegate:)]
        pub fn setDelegate(&self, delegate: Option<&NSObject>);

        #[sel(sendEvent:)]
        pub unsafe fn sendEvent(&self, event: &NSEvent);
    }
);

#[allow(dead_code)]
#[repr(isize)] // NSInteger
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NSWindowTitleVisibility {
    #[doc(alias = "NSWindowTitleVisible")]
    Visible = 0,
    #[doc(alias = "NSWindowTitleHidden")]
    Hidden = 1,
}

unsafe impl Encode for NSWindowTitleVisibility {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

#[allow(dead_code)]
#[repr(usize)] // NSUInteger
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NSWindowButton {
    #[doc(alias = "NSWindowCloseButton")]
    Close = 0,
    #[doc(alias = "NSWindowMiniaturizeButton")]
    Miniaturize = 1,
    #[doc(alias = "NSWindowZoomButton")]
    Zoom = 2,
    #[doc(alias = "NSWindowToolbarButton")]
    Toolbar = 3,
    #[doc(alias = "NSWindowDocumentIconButton")]
    DocumentIcon = 4,
    #[doc(alias = "NSWindowDocumentVersionsButton")]
    DocumentVersions = 6,
    #[doc(alias = "NSWindowFullScreenButton")]
    #[deprecated = "Deprecated since macOS 10.12"]
    FullScreen = 7,
}

unsafe impl Encode for NSWindowButton {
    const ENCODING: Encoding = NSUInteger::ENCODING;
}

#[allow(dead_code)]
mod window_level_key {
    use objc2::foundation::NSInteger;
    pub const kCGBaseWindowLevelKey: NSInteger = 0;
    pub const kCGMinimumWindowLevelKey: NSInteger = 1;
    pub const kCGDesktopWindowLevelKey: NSInteger = 2;
    pub const kCGBackstopMenuLevelKey: NSInteger = 3;
    pub const kCGNormalWindowLevelKey: NSInteger = 4;
    pub const kCGFloatingWindowLevelKey: NSInteger = 5;
    pub const kCGTornOffMenuWindowLevelKey: NSInteger = 6;
    pub const kCGDockWindowLevelKey: NSInteger = 7;
    pub const kCGMainMenuWindowLevelKey: NSInteger = 8;
    pub const kCGStatusWindowLevelKey: NSInteger = 9;
    pub const kCGModalPanelWindowLevelKey: NSInteger = 10;
    pub const kCGPopUpMenuWindowLevelKey: NSInteger = 11;
    pub const kCGDraggingWindowLevelKey: NSInteger = 12;
    pub const kCGScreenSaverWindowLevelKey: NSInteger = 13;
    pub const kCGMaximumWindowLevelKey: NSInteger = 14;
    pub const kCGOverlayWindowLevelKey: NSInteger = 15;
    pub const kCGHelpWindowLevelKey: NSInteger = 16;
    pub const kCGUtilityWindowLevelKey: NSInteger = 17;
    pub const kCGDesktopIconWindowLevelKey: NSInteger = 18;
    pub const kCGCursorWindowLevelKey: NSInteger = 19;
    pub const kCGNumberOfWindowLevelKeys: NSInteger = 20;
}
use window_level_key::*;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
#[repr(isize)]
pub enum NSWindowLevel {
    #[doc(alias = "NSNormalWindowLevel")]
    Normal = kCGBaseWindowLevelKey,
    #[doc(alias = "NSFloatingWindowLevel")]
    Floating = kCGFloatingWindowLevelKey,
    #[doc(alias = "NSTornOffMenuWindowLevel")]
    TornOffMenu = kCGTornOffMenuWindowLevelKey,
    #[doc(alias = "NSModalPanelWindowLevel")]
    ModalPanel = kCGModalPanelWindowLevelKey,
    #[doc(alias = "NSMainMenuWindowLevel")]
    MainMenu = kCGMainMenuWindowLevelKey,
    #[doc(alias = "NSStatusWindowLevel")]
    Status = kCGStatusWindowLevelKey,
    #[doc(alias = "NSPopUpMenuWindowLevel")]
    PopUpMenu = kCGPopUpMenuWindowLevelKey,
    #[doc(alias = "NSScreenSaverWindowLevel")]
    ScreenSaver = kCGScreenSaverWindowLevelKey,
}

unsafe impl Encode for NSWindowLevel {
    const ENCODING: Encoding = NSInteger::ENCODING;
}

bitflags! {
    pub struct NSWindowOcclusionState: NSUInteger {
        const NSWindowOcclusionStateVisible = 1 << 1;
    }
}

unsafe impl Encode for NSWindowOcclusionState {
    const ENCODING: Encoding = NSUInteger::ENCODING;
}

bitflags! {
    pub struct NSWindowStyleMask: NSUInteger {
        const NSBorderlessWindowMask      = 0;
        const NSTitledWindowMask          = 1 << 0;
        const NSClosableWindowMask        = 1 << 1;
        const NSMiniaturizableWindowMask  = 1 << 2;
        const NSResizableWindowMask       = 1 << 3;
        const NSTexturedBackgroundWindowMask  = 1 << 8;
        const NSUnifiedTitleAndToolbarWindowMask  = 1 << 12;
        const NSFullScreenWindowMask      = 1 << 14;
        const NSFullSizeContentViewWindowMask = 1 << 15;
    }
}

unsafe impl Encode for NSWindowStyleMask {
    const ENCODING: Encoding = NSUInteger::ENCODING;
}
