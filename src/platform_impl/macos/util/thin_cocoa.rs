#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]

use core::ptr;

use libc;
pub use objc::{
    runtime,
    runtime::{BOOL, NO, YES},
};

#[allow(non_camel_case_types)]
pub type id = *mut runtime::Object;

pub unsafe fn NSApp() -> id {
    msg_send![class!(NSApplication), sharedApplication]
}

#[cfg(target_pointer_width = "32")]
pub type NSInteger = libc::c_int;
#[cfg(target_pointer_width = "32")]
pub type NSUInteger = libc::c_uint;

#[cfg(target_pointer_width = "64")]
pub type NSInteger = libc::c_long;
#[cfg(target_pointer_width = "64")]
pub type NSUInteger = libc::c_ulong;

#[cfg(target_pointer_width = "64")]
pub type CGFloat = libc::c_double;
#[cfg(not(target_pointer_width = "64"))]
pub type CGFloat = libc::c_float;

// pub type Class = *mut runtime::Class;

#[allow(non_upper_case_globals)]
pub const nil: id = 0 as id;
#[allow(non_upper_case_globals)]
// pub const Nil: Class = 0 as Class;
#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NSApplicationActivationPolicy {
    NSApplicationActivationPolicyRegular = 0,
    NSApplicationActivationPolicyAccessory = 1,
    NSApplicationActivationPolicyProhibited = 2,
    // NSApplicationActivationPolicyERROR = -1,
}

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NSRequestUserAttentionType {
    NSCriticalRequest = 0,
    NSInformationalRequest = 10,
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

bitflags! {
    pub struct NSApplicationPresentationOptions : NSUInteger {
        const NSApplicationPresentationDefault = 0;
        const NSApplicationPresentationAutoHideDock = 1 << 0;
        const NSApplicationPresentationHideDock = 1 << 1;
        const NSApplicationPresentationAutoHideMenuBar = 1 << 2;
        const NSApplicationPresentationHideMenuBar = 1 << 3;
        const NSApplicationPresentationDisableAppleMenu = 1 << 4;
        const NSApplicationPresentationDisableProcessSwitching = 1 << 5;
        const NSApplicationPresentationDisableForceQuit = 1 << 6;
        const NSApplicationPresentationDisableSessionTermination = 1 << 7;
        const NSApplicationPresentationDisableHideApplication = 1 << 8;
        const NSApplicationPresentationDisableMenuBarTransparency = 1 << 9;
        const NSApplicationPresentationFullScreen = 1 << 10;
        const NSApplicationPresentationAutoHideToolbar = 1 << 11;
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct NSPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

impl NSPoint {
    #[inline]
    pub fn new(x: CGFloat, y: CGFloat) -> NSPoint {
        NSPoint { x, y }
    }
}

unsafe impl objc::Encode for NSPoint {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            "{{CGPoint={}{}}}",
            CGFloat::encode().as_str(),
            CGFloat::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct NSSize {
    pub width: CGFloat,
    pub height: CGFloat,
}

impl NSSize {
    #[inline]
    pub fn new(width: CGFloat, height: CGFloat) -> NSSize {
        NSSize { width, height }
    }
}

unsafe impl objc::Encode for NSSize {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            "{{CGSize={}{}}}",
            CGFloat::encode().as_str(),
            CGFloat::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct NSRect {
    pub origin: NSPoint,
    pub size: NSSize,
}

impl NSRect {
    #[inline]
    pub fn new(origin: NSPoint, size: NSSize) -> NSRect {
        NSRect { origin, size }
    }

    /*
    #[inline]
    pub fn as_CGRect(&self) -> &CGRect {
        unsafe { mem::transmute::<&NSRect, &CGRect>(self) }
    }

    #[inline]
    pub fn inset(&self, x: CGFloat, y: CGFloat) -> NSRect {
        unsafe { NSInsetRect(*self, x, y) }
    }
    */
}

unsafe impl objc::Encode for NSRect {
    fn encode() -> objc::Encoding {
        let encoding = format!(
            "{{CGRect={}{}}}",
            NSPoint::encode().as_str(),
            NSSize::encode().as_str()
        );
        unsafe { objc::Encoding::from_str(&encoding) }
    }
}

pub trait NSString: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSString), alloc]
    }

    unsafe fn stringByAppendingString_(self, other: id) -> id;
    unsafe fn init_str(self, string: &str) -> Self;
    unsafe fn UTF8String(self) -> *const libc::c_char;
    unsafe fn len(self) -> usize;
    unsafe fn isEqualToString(self, other: &str) -> bool;
    unsafe fn substringWithRange(self, range: NSRange) -> id;
}

const UTF8_ENCODING: usize = 4;

impl NSString for id {
    unsafe fn isEqualToString(self, other: &str) -> bool {
        let other = NSString::alloc(nil).init_str(other);
        let rv: BOOL = msg_send![self, isEqualToString: other];
        rv != NO
    }

    unsafe fn stringByAppendingString_(self, other: id) -> id {
        msg_send![self, stringByAppendingString: other]
    }

    unsafe fn init_str(self, string: &str) -> id {
        return msg_send![self,
                         initWithBytes:string.as_ptr()
                             length:string.len()
                             encoding:UTF8_ENCODING as id];
    }

    unsafe fn len(self) -> usize {
        msg_send![self, lengthOfBytesUsingEncoding: UTF8_ENCODING]
    }

    unsafe fn UTF8String(self) -> *const libc::c_char {
        msg_send![self, UTF8String]
    }

    unsafe fn substringWithRange(self, range: NSRange) -> id {
        msg_send![self, substringWithRange: range]
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct NSRange {
    pub location: NSUInteger,
    pub length: NSUInteger,
}

/*
impl NSRange {
    #[inline]
    pub fn new(location: NSUInteger, length: NSUInteger) -> NSRange {
        NSRange { location, length }
    }
}
*/

pub trait NSApplication: Sized {
    unsafe fn sharedApplication(_: Self) -> id {
        msg_send![class!(NSApplication), sharedApplication]
    }

    unsafe fn mainMenu(self) -> id;
    unsafe fn setActivationPolicy_(self, policy: NSApplicationActivationPolicy) -> BOOL;
    unsafe fn setPresentationOptions_(self, options: NSApplicationPresentationOptions) -> BOOL;
    unsafe fn presentationOptions_(self) -> NSApplicationPresentationOptions;
    unsafe fn setMainMenu_(self, menu: id);
    unsafe fn setServicesMenu_(self, menu: id);
    unsafe fn setWindowsMenu_(self, menu: id);
    unsafe fn activateIgnoringOtherApps_(self, ignore: BOOL);
    unsafe fn run(self);
    unsafe fn finishLaunching(self);
    unsafe fn nextEventMatchingMask_untilDate_inMode_dequeue_(
        self,
        mask: NSUInteger,
        expiration: id,
        in_mode: id,
        dequeue: BOOL,
    ) -> id;
    unsafe fn sendEvent_(self, an_event: id);
    unsafe fn postEvent_atStart_(self, anEvent: id, flag: BOOL);
    unsafe fn stop_(self, sender: id);
    unsafe fn setApplicationIconImage_(self, image: id);
    unsafe fn requestUserAttention_(self, requestType: NSRequestUserAttentionType);
}

impl NSApplication for id {
    unsafe fn mainMenu(self) -> id {
        msg_send![self, mainMenu]
    }

    unsafe fn setActivationPolicy_(self, policy: NSApplicationActivationPolicy) -> BOOL {
        msg_send![self, setActivationPolicy: policy as NSInteger]
    }

    unsafe fn setPresentationOptions_(self, options: NSApplicationPresentationOptions) -> BOOL {
        msg_send![self, setPresentationOptions:options.bits]
    }

    unsafe fn presentationOptions_(self) -> NSApplicationPresentationOptions {
        let options = msg_send![self, presentationOptions];
        return NSApplicationPresentationOptions::from_bits(options).unwrap();
    }

    unsafe fn setMainMenu_(self, menu: id) {
        msg_send![self, setMainMenu: menu]
    }

    unsafe fn setServicesMenu_(self, menu: id) {
        msg_send![self, setServicesMenu: menu]
    }

    unsafe fn setWindowsMenu_(self, menu: id) {
        msg_send![self, setWindowsMenu: menu]
    }

    unsafe fn activateIgnoringOtherApps_(self, ignore: BOOL) {
        msg_send![self, activateIgnoringOtherApps: ignore]
    }

    unsafe fn run(self) {
        msg_send![self, run]
    }

    unsafe fn finishLaunching(self) {
        msg_send![self, finishLaunching]
    }

    unsafe fn nextEventMatchingMask_untilDate_inMode_dequeue_(
        self,
        mask: NSUInteger,
        expiration: id,
        in_mode: id,
        dequeue: BOOL,
    ) -> id {
        msg_send![self, nextEventMatchingMask:mask
                                    untilDate:expiration
                                       inMode:in_mode
                                      dequeue:dequeue]
    }

    unsafe fn sendEvent_(self, an_event: id) {
        msg_send![self, sendEvent: an_event]
    }

    unsafe fn postEvent_atStart_(self, anEvent: id, flag: BOOL) {
        msg_send![self, postEvent:anEvent atStart:flag]
    }

    unsafe fn stop_(self, sender: id) {
        msg_send![self, stop: sender]
    }

    unsafe fn setApplicationIconImage_(self, icon: id) {
        msg_send![self, setApplicationIconImage: icon]
    }

    unsafe fn requestUserAttention_(self, requestType: NSRequestUserAttentionType) {
        msg_send![self, requestUserAttention: requestType]
    }
}

pub trait NSEvent: Sized {
    /*
    // Creating Events
    unsafe fn keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode_(
        _: Self,
        eventType: NSEventType,
        location: NSPoint,
        modifierFlags: NSEventModifierFlags,
        timestamp: NSTimeInterval,
        windowNumber: NSInteger,
        context: id /* (NSGraphicsContext *) */,
        characters: id /* (NSString *) */,
        unmodCharacters: id /* (NSString *) */,
        repeatKey: BOOL,
        code: libc::c_ushort) -> id /* (NSEvent *) */;
    unsafe fn mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure_(
        _: Self,
        eventType: NSEventType,
        location: NSPoint,
        modifierFlags: NSEventModifierFlags,
        timestamp: NSTimeInterval,
        windowNumber: NSInteger,
        context: id /* (NSGraphicsContext *) */,
        eventNumber: NSInteger,
        clickCount: NSInteger,
        pressure: libc::c_float) -> id /* (NSEvent *) */;
    unsafe fn enterExitEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_trackingNumber_userData_(
        _: Self,
        eventType: NSEventType,
        location: NSPoint,
        modifierFlags: NSEventModifierFlags,
        timestamp: NSTimeInterval,
        windowNumber: NSInteger,
        context: id /* (NSGraphicsContext *) */,
        eventNumber: NSInteger,
        trackingNumber: NSInteger,
        userData: *mut c_void) -> id /* (NSEvent *) */;
    unsafe fn otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2_(
        _: Self,
        eventType: NSEventType,
        location: NSPoint,
        modifierFlags: NSEventModifierFlags,
        timestamp: NSTimeInterval,
        windowNumber: NSInteger,
        context: id /* (NSGraphicsContext *) */,
        subtype: NSEventSubtype,
        data1: NSInteger,
        data2: NSInteger) -> id /* (NSEvent *) */;
    unsafe fn eventWithEventRef_(_: Self, eventRef: *const c_void) -> id;
    unsafe fn eventWithCGEvent_(_: Self, cgEvent: *mut c_void /* CGEventRef */) -> id;

    // Getting General Event Information
    unsafe fn context(self) -> id /* (NSGraphicsContext *) */;
     */
    unsafe fn locationInWindow(self) -> NSPoint;

    unsafe fn modifierFlags(self) -> NSEventModifierFlags;
    /*
    unsafe fn timestamp(self) -> NSTimeInterval;
    // NOTE: renamed from `- type` due to Rust keyword collision
    */
    unsafe fn eventType(self) -> NSEventType;
    /*
    unsafe fn window(self) -> id /* (NSWindow *) */;
    unsafe fn windowNumber(self) -> NSInteger;
    unsafe fn eventRef(self) -> *const c_void;
    unsafe fn CGEvent(self) -> *mut c_void /* CGEventRef */;

    // Getting Key Event Information
    // NOTE: renamed from `+ modifierFlags` due to conflict with `- modifierFlags`
    unsafe fn currentModifierFlags(_: Self) -> NSEventModifierFlags;
    unsafe fn keyRepeatDelay(_: Self) -> NSTimeInterval;
    unsafe fn keyRepeatInterval(_: Self) -> NSTimeInterval;
    unsafe fn characters(self) -> id /* (NSString *) */;
    unsafe fn charactersIgnoringModifiers(self) -> id /* (NSString *) */;
    unsafe fn keyCode(self) -> libc::c_ushort;
    unsafe fn isARepeat(self) -> BOOL;

    // Getting Mouse Event Information
    unsafe fn pressedMouseButtons(_: Self) -> NSUInteger;
    unsafe fn doubleClickInterval(_: Self) -> NSTimeInterval;
    unsafe fn mouseLocation(_: Self) -> NSPoint;

    */
    unsafe fn buttonNumber(self) -> NSInteger;

    /*
    unsafe fn clickCount(self) -> NSInteger;
    */

    unsafe fn pressure(self) -> libc::c_float;
    unsafe fn stage(self) -> NSInteger;
    /*
    unsafe fn setMouseCoalescingEnabled_(_: Self, flag: BOOL);
    unsafe fn isMouseCoalescingEnabled(_: Self) -> BOOL;

    // Getting Mouse-Tracking Event Information
    unsafe fn eventNumber(self) -> NSInteger;
    unsafe fn trackingNumber(self) -> NSInteger;
    unsafe fn trackingArea(self) -> id /* (NSTrackingArea *) */;
    unsafe fn userData(self) -> *const c_void;

    // Getting Custom Event Information
    unsafe fn data1(self) -> NSInteger;
    unsafe fn data2(self) -> NSInteger;
    unsafe fn subtype(self) -> NSEventSubtype;

    // Getting Scroll Wheel Event Information
    */
    unsafe fn deltaX(self) -> CGFloat;
    unsafe fn deltaY(self) -> CGFloat;
    unsafe fn deltaZ(self) -> CGFloat;

    /*
    // Getting Tablet Proximity Information
    unsafe fn capabilityMask(self) -> NSUInteger;
    unsafe fn deviceID(self) -> NSUInteger;
    unsafe fn pointingDeviceID(self) -> NSUInteger;
    unsafe fn pointingDeviceSerialNumber(self) -> NSUInteger;
    unsafe fn pointingDeviceType(self) -> NSPointingDeviceType;
    unsafe fn systemTabletID(self) -> NSUInteger;
    unsafe fn tabletID(self) -> NSUInteger;
    unsafe fn uniqueID(self) -> libc::c_ulonglong;
    unsafe fn vendorID(self) -> NSUInteger;
    unsafe fn vendorPointingDeviceType(self) -> NSUInteger;

    // Getting Tablet Pointing Information
    unsafe fn absoluteX(self) -> NSInteger;
    unsafe fn absoluteY(self) -> NSInteger;
    unsafe fn absoluteZ(self) -> NSInteger;
    unsafe fn buttonMask(self) -> NSEventButtonMask;
    unsafe fn rotation(self) -> libc::c_float;
    unsafe fn tangentialPressure(self) -> libc::c_float;
    unsafe fn tilt(self) -> NSPoint;
    unsafe fn vendorDefined(self) -> id;

    // Requesting and Stopping Periodic Events
    unsafe fn startPeriodicEventsAfterDelay_withPeriod_(_: Self, delaySeconds: NSTimeInterval, periodSeconds: NSTimeInterval);
    unsafe fn stopPeriodicEvents(_: Self);

    // Getting Touch and Gesture Information
    unsafe fn magnification(self) -> CGFloat;
    unsafe fn touchesMatchingPhase_inView_(self, phase: NSTouchPhase, view: id /* (NSView *) */) -> id /* (NSSet *) */;
    unsafe fn isSwipeTrackingFromScrollEventsEnabled(_: Self) -> BOOL;

    // Monitoring Application Events
    // TODO: addGlobalMonitorForEventsMatchingMask_handler_ (unsure how to bind to blocks)
    // TODO: addLocalMonitorForEventsMatchingMask_handler_ (unsure how to bind to blocks)
    unsafe fn removeMonitor_(_: Self, eventMonitor: id);
    */
    // Scroll Wheel and Flick Events
    unsafe fn hasPreciseScrollingDeltas(self) -> BOOL;
    unsafe fn scrollingDeltaX(self) -> CGFloat;
    unsafe fn scrollingDeltaY(self) -> CGFloat;

    /*
    unsafe fn momentumPhase(self) -> NSEventPhase;
     */
    unsafe fn phase(self) -> NSEventPhase;
    /*
    // TODO: trackSwipeEventWithOptions_dampenAmountThresholdMin_max_usingHandler_ (unsure how to bind to blocks)

    // Converting a Mouse Event’s Position into a Sprite Kit Node’s Coordinate Space
    unsafe fn locationInNode_(self, node: id /* (SKNode *) */) -> CGPoint;
    */
}

impl NSEvent for id {
    /*
    // Creating Events

    unsafe fn keyEventWithType_location_modifierFlags_timestamp_windowNumber_context_characters_charactersIgnoringModifiers_isARepeat_keyCode_(
        _: Self,
        eventType: NSEventType,
        location: NSPoint,
        modifierFlags: NSEventModifierFlags,
        timestamp: NSTimeInterval,
        windowNumber: NSInteger,
        context: id /* (NSGraphicsContext *) */,
        characters: id /* (NSString *) */,
        unmodCharacters: id /* (NSString *) */,
        repeatKey: BOOL,
        code: libc::c_ushort) -> id /* (NSEvent *) */
    {
        msg_send![class!(NSEvent), keyEventWithType:eventType
                                            location:location
                                       modifierFlags:modifierFlags
                                           timestamp:timestamp
                                        windowNumber:windowNumber
                                             context:context
                                          characters:characters
                         charactersIgnoringModifiers:unmodCharacters
                                           isARepeat:repeatKey
                                             keyCode:code]
    }

    unsafe fn mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure_(
        _: Self,
        eventType: NSEventType,
        location: NSPoint,
        modifierFlags: NSEventModifierFlags,
        timestamp: NSTimeInterval,
        windowNumber: NSInteger,
        context: id /* (NSGraphicsContext *) */,
        eventNumber: NSInteger,
        clickCount: NSInteger,
        pressure: libc::c_float) -> id /* (NSEvent *) */
    {
        msg_send![class!(NSEvent), mouseEventWithType:eventType
                                              location:location
                                         modifierFlags:modifierFlags
                                             timestamp:timestamp
                                          windowNumber:windowNumber
                                               context:context
                                           eventNumber:eventNumber
                                            clickCount:clickCount
                                              pressure:pressure]
    }

    unsafe fn enterExitEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_trackingNumber_userData_(
        _: Self,
        eventType: NSEventType,
        location: NSPoint,
        modifierFlags: NSEventModifierFlags,
        timestamp: NSTimeInterval,
        windowNumber: NSInteger,
        context: id /* (NSGraphicsContext *) */,
        eventNumber: NSInteger,
        trackingNumber: NSInteger,
        userData: *mut c_void) -> id /* (NSEvent *) */
    {
        msg_send![class!(NSEvent), enterExitEventWithType:eventType
                                                  location:location
                                             modifierFlags:modifierFlags
                                                 timestamp:timestamp
                                              windowNumber:windowNumber
                                                   context:context
                                               eventNumber:eventNumber
                                            trackingNumber:trackingNumber
                                                  userData:userData]
    }

    unsafe fn otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2_(
        _: Self,
        eventType: NSEventType,
        location: NSPoint,
        modifierFlags: NSEventModifierFlags,
        timestamp: NSTimeInterval,
        windowNumber: NSInteger,
        context: id /* (NSGraphicsContext *) */,
        subtype: NSEventSubtype,
        data1: NSInteger,
        data2: NSInteger) -> id /* (NSEvent *) */
    {
        msg_send![class!(NSEvent), otherEventWithType:eventType
                                              location:location
                                         modifierFlags:modifierFlags
                                             timestamp:timestamp
                                          windowNumber:windowNumber
                                               context:context
                                               subtype:subtype
                                                 data1:data1
                                                 data2:data2]
    }

    unsafe fn eventWithEventRef_(_: Self, eventRef: *const c_void) -> id {
        msg_send![class!(NSEvent), eventWithEventRef:eventRef]
    }

    unsafe fn eventWithCGEvent_(_: Self, cgEvent: *mut c_void /* CGEventRef */) -> id {
        msg_send![class!(NSEvent), eventWithCGEvent:cgEvent]
    }

    // Getting General Event Information

    unsafe fn context(self) -> id /* (NSGraphicsContext *) */ {
        msg_send![self, context]
    }
    */
    unsafe fn locationInWindow(self) -> NSPoint {
        msg_send![self, locationInWindow]
    }

    unsafe fn modifierFlags(self) -> NSEventModifierFlags {
        msg_send![self, modifierFlags]
    }

    /*
    unsafe fn timestamp(self) -> NSTimeInterval {
        msg_send![self, timestamp]
    }
    // NOTE: renamed from `- type` due to Rust keyword collision
    */

    unsafe fn eventType(self) -> NSEventType {
        msg_send![self, type]
    }

    /*
    unsafe fn window(self) -> id /* (NSWindow *) */ {
        msg_send![self, window]
    }

    unsafe fn windowNumber(self) -> NSInteger {
        msg_send![self, windowNumber]
    }

    unsafe fn eventRef(self) -> *const c_void {
        msg_send![self, eventRef]
    }

    unsafe fn CGEvent(self) -> *mut c_void /* CGEventRef */ {
        msg_send![self, CGEvent]
    }

    // Getting Key Event Information

    // NOTE: renamed from `+ modifierFlags` due to conflict with `- modifierFlags`

    unsafe fn currentModifierFlags(_: Self) -> NSEventModifierFlags {
        msg_send![class!(NSEvent), currentModifierFlags]
    }

    unsafe fn keyRepeatDelay(_: Self) -> NSTimeInterval {
        msg_send![class!(NSEvent), keyRepeatDelay]
    }

    unsafe fn keyRepeatInterval(_: Self) -> NSTimeInterval {
        msg_send![class!(NSEvent), keyRepeatInterval]
    }

    unsafe fn characters(self) -> id /* (NSString *) */ {
        msg_send![self, characters]
    }

    unsafe fn charactersIgnoringModifiers(self) -> id /* (NSString *) */ {
        msg_send![self, charactersIgnoringModifiers]
    }

    unsafe fn keyCode(self) -> libc::c_ushort {
        msg_send![self, keyCode]
    }

    unsafe fn isARepeat(self) -> BOOL {
        msg_send![self, isARepeat]
    }

    // Getting Mouse Event Information

    unsafe fn pressedMouseButtons(_: Self) -> NSUInteger {
        msg_send![class!(NSEvent), pressedMouseButtons]
    }

    unsafe fn doubleClickInterval(_: Self) -> NSTimeInterval {
        msg_send![class!(NSEvent), doubleClickInterval]
    }

    unsafe fn mouseLocation(_: Self) -> NSPoint {
        msg_send![class!(NSEvent), mouseLocation]
    }
    */

    unsafe fn buttonNumber(self) -> NSInteger {
        msg_send![self, buttonNumber]
    }

    /*
    unsafe fn clickCount(self) -> NSInteger {
        msg_send![self, clickCount]
    }
    */
    unsafe fn pressure(self) -> libc::c_float {
        msg_send![self, pressure]
    }

    unsafe fn stage(self) -> NSInteger {
        msg_send![self, stage]
    }
    /*
    unsafe fn setMouseCoalescingEnabled_(_: Self, flag: BOOL) {
        msg_send![class!(NSEvent), setMouseCoalescingEnabled:flag]
    }

    unsafe fn isMouseCoalescingEnabled(_: Self) -> BOOL {
        msg_send![class!(NSEvent), isMouseCoalescingEnabled]
    }

    // Getting Mouse-Tracking Event Information

    unsafe fn eventNumber(self) -> NSInteger {
        msg_send![self, eventNumber]
    }

    unsafe fn trackingNumber(self) -> NSInteger {
        msg_send![self, trackingNumber]
    }

    unsafe fn trackingArea(self) -> id /* (NSTrackingArea *) */ {
        msg_send![self, trackingArea]
    }

    unsafe fn userData(self) -> *const c_void {
        msg_send![self, userData]
    }

    // Getting Custom Event Information

    unsafe fn data1(self) -> NSInteger {
        msg_send![self, data1]
    }

    unsafe fn data2(self) -> NSInteger {
        msg_send![self, data2]
    }

    unsafe fn subtype(self) -> NSEventSubtype {
        msg_send![self, subtype]
    }

    // Getting Scroll Wheel Event Information
    */

    unsafe fn deltaX(self) -> CGFloat {
        msg_send![self, deltaX]
    }

    unsafe fn deltaY(self) -> CGFloat {
        msg_send![self, deltaY]
    }

    unsafe fn deltaZ(self) -> CGFloat {
        msg_send![self, deltaZ]
    }

    /*

    // Getting Tablet Proximity Information

    unsafe fn capabilityMask(self) -> NSUInteger {
        msg_send![self, capabilityMask]
    }

    unsafe fn deviceID(self) -> NSUInteger {
        msg_send![self, deviceID]
    }

    unsafe fn pointingDeviceID(self) -> NSUInteger {
        msg_send![self, pointingDeviceID]
    }

    unsafe fn pointingDeviceSerialNumber(self) -> NSUInteger {
        msg_send![self, pointingDeviceSerialNumber]
    }

    unsafe fn pointingDeviceType(self) -> NSPointingDeviceType {
        msg_send![self, pointingDeviceType]
    }

    unsafe fn systemTabletID(self) -> NSUInteger {
        msg_send![self, systemTabletID]
    }

    unsafe fn tabletID(self) -> NSUInteger {
        msg_send![self, tabletID]
    }

    unsafe fn uniqueID(self) -> libc::c_ulonglong {
        msg_send![self, uniqueID]
    }

    unsafe fn vendorID(self) -> NSUInteger {
        msg_send![self, vendorID]
    }

    unsafe fn vendorPointingDeviceType(self) -> NSUInteger {
        msg_send![self, vendorPointingDeviceType]
    }

    // Getting Tablet Pointing Information

    unsafe fn absoluteX(self) -> NSInteger {
        msg_send![self, absoluteX]
    }

    unsafe fn absoluteY(self) -> NSInteger {
        msg_send![self, absoluteY]
    }

    unsafe fn absoluteZ(self) -> NSInteger {
        msg_send![self, absoluteZ]
    }

    unsafe fn buttonMask(self) -> NSEventButtonMask {
        msg_send![self, buttonMask]
    }

    unsafe fn rotation(self) -> libc::c_float {
        msg_send![self, rotation]
    }

    unsafe fn tangentialPressure(self) -> libc::c_float {
        msg_send![self, tangentialPressure]
    }

    unsafe fn tilt(self) -> NSPoint {
        msg_send![self, tilt]
    }

    unsafe fn vendorDefined(self) -> id {
        msg_send![self, vendorDefined]
    }

    // Requesting and Stopping Periodic Events

    unsafe fn startPeriodicEventsAfterDelay_withPeriod_(_: Self, delaySeconds: NSTimeInterval, periodSeconds: NSTimeInterval) {
        msg_send![class!(NSEvent), startPeriodicEventsAfterDelay:delaySeconds withPeriod:periodSeconds]
    }

    unsafe fn stopPeriodicEvents(_: Self) {
        msg_send![class!(NSEvent), stopPeriodicEvents]
    }

    // Getting Touch and Gesture Information

    unsafe fn magnification(self) -> CGFloat {
        msg_send![self, magnification]
    }

    unsafe fn touchesMatchingPhase_inView_(self, phase: NSTouchPhase, view: id /* (NSView *) */) -> id /* (NSSet *) */ {
        msg_send![self, touchesMatchingPhase:phase inView:view]
    }

    unsafe fn isSwipeTrackingFromScrollEventsEnabled(_: Self) -> BOOL {
        msg_send![class!(NSEvent), isSwipeTrackingFromScrollEventsEnabled]
    }

    // Monitoring Application Events

    // TODO: addGlobalMonitorForEventsMatchingMask_handler_ (unsure how to bind to blocks)
    // TODO: addLocalMonitorForEventsMatchingMask_handler_ (unsure how to bind to blocks)

    unsafe fn removeMonitor_(_: Self, eventMonitor: id) {
        msg_send![class!(NSEvent), removeMonitor:eventMonitor]
    }

    // Scroll Wheel and Flick Events
    */

    unsafe fn hasPreciseScrollingDeltas(self) -> BOOL {
        msg_send![self, hasPreciseScrollingDeltas]
    }
    unsafe fn scrollingDeltaX(self) -> CGFloat {
        msg_send![self, scrollingDeltaX]
    }

    unsafe fn scrollingDeltaY(self) -> CGFloat {
        msg_send![self, scrollingDeltaY]
    }

    /*
    unsafe fn momentumPhase(self) -> NSEventPhase {
        msg_send![self, momentumPhase]
    }
    */
    unsafe fn phase(self) -> NSEventPhase {
        msg_send![self, phase]
    }
    /*
    // TODO: trackSwipeEventWithOptions_dampenAmountThresholdMin_max_usingHandler_ (unsure how to bind to blocks)

    // Converting a Mouse Event’s Position into a Sprite Kit Node’s Coordinate Space
    unsafe fn locationInNode_(self, node: id /* (SKNode *) */) -> CGPoint {
        msg_send![self, locationInNode:node]
    }
    */
}

bitflags! {
    pub struct NSEventModifierFlags: NSUInteger {
        const NSAlphaShiftKeyMask                     = 1 << 16;
        const NSShiftKeyMask                          = 1 << 17;
        const NSControlKeyMask                        = 1 << 18;
        const NSAlternateKeyMask                      = 1 << 19;
        const NSCommandKeyMask                        = 1 << 20;
        const NSNumericPadKeyMask                     = 1 << 21;
        const NSHelpKeyMask                           = 1 << 22;
        const NSFunctionKeyMask                       = 1 << 23;
        const NSDeviceIndependentModifierFlagsMask    = 0xffff0000;
    }
}

#[repr(i16)]
pub enum NSEventSubtype {
    // TODO: Not sure what these values are
    // NSMouseEventSubtype           = NX_SUBTYPE_DEFAULT,
    // NSTabletPointEventSubtype     = NX_SUBTYPE_TABLET_POINT,
    // NSTabletProximityEventSubtype = NX_SUBTYPE_TABLET_PROXIMITY
    // NSTouchEventSubtype           = NX_SUBTYPE_MOUSE_TOUCH,
    NSWindowExposedEventType = 0,
    // NSApplicationActivatedEventType = 1,
    // NSApplicationDeactivatedEventType = 2,
    // NSWindowMovedEventType = 4,
    // NSScreenChangedEventType = 8,
    // NSAWTEventType = 16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u64)] // NSUInteger
#[allow(dead_code)]
pub enum NSEventType {
    NSLeftMouseDown = 1,
    NSLeftMouseUp = 2,
    NSRightMouseDown = 3,
    NSRightMouseUp = 4,
    NSMouseMoved = 5,
    NSLeftMouseDragged = 6,
    NSRightMouseDragged = 7,
    NSMouseEntered = 8,
    NSMouseExited = 9,
    NSKeyDown = 10,
    NSKeyUp = 11,
    NSFlagsChanged = 12,
    NSAppKitDefined = 13,
    NSSystemDefined = 14,
    NSApplicationDefined = 15,
    NSPeriodic = 16,
    NSCursorUpdate = 17,
    NSScrollWheel = 22,
    NSTabletPoint = 23,
    NSTabletProximity = 24,
    NSOtherMouseDown = 25,
    NSOtherMouseUp = 26,
    NSOtherMouseDragged = 27,
    NSEventTypeGesture = 29,
    NSEventTypeMagnify = 30,
    NSEventTypeSwipe = 31,
    NSEventTypeRotate = 18,
    NSEventTypeBeginGesture = 19,
    NSEventTypeEndGesture = 20,
    NSEventTypePressure = 34,
}

pub type NSTimeInterval = libc::c_double;

/// A convenience method to convert the name of a selector to the selector object.
#[inline]
pub fn selector(name: &str) -> SEL {
    runtime::Sel::register(name)
}
pub type SEL = runtime::Sel;

pub trait NSMenu: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSMenu), alloc]
    }

    unsafe fn new(_: Self) -> id {
        msg_send![class!(NSMenu), new]
    }

    unsafe fn initWithTitle_(self, title: id /* NSString */) -> id;
    unsafe fn setAutoenablesItems(self, state: BOOL);

    unsafe fn addItem_(self, menu_item: id);
    unsafe fn addItemWithTitle_action_keyEquivalent(self, title: id, action: SEL, key: id) -> id;
    unsafe fn itemAtIndex_(self, index: NSInteger) -> id;
}

impl NSMenu for id {
    unsafe fn initWithTitle_(self, title: id /* NSString */) -> id {
        msg_send![self, initWithTitle: title]
    }

    unsafe fn setAutoenablesItems(self, state: BOOL) {
        msg_send![self, setAutoenablesItems: state]
    }

    unsafe fn addItem_(self, menu_item: id) {
        msg_send![self, addItem: menu_item]
    }

    unsafe fn addItemWithTitle_action_keyEquivalent(self, title: id, action: SEL, key: id) -> id {
        msg_send![self, addItemWithTitle:title action:action keyEquivalent:key]
    }

    unsafe fn itemAtIndex_(self, index: NSInteger) -> id {
        msg_send![self, itemAtIndex: index]
    }
}

pub trait NSMenuItem: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSMenuItem), alloc]
    }

    unsafe fn new(_: Self) -> id {
        msg_send![class!(NSMenuItem), new]
    }

    unsafe fn separatorItem(_: Self) -> id {
        msg_send![class!(NSMenuItem), separatorItem]
    }

    unsafe fn initWithTitle_action_keyEquivalent_(self, title: id, action: SEL, key: id) -> id;
    unsafe fn setKeyEquivalentModifierMask_(self, mask: NSEventModifierFlags);
    unsafe fn setSubmenu_(self, submenu: id);
    unsafe fn setTarget_(self, target: id);
}

impl NSMenuItem for id {
    unsafe fn initWithTitle_action_keyEquivalent_(self, title: id, action: SEL, key: id) -> id {
        msg_send![self, initWithTitle:title action:action keyEquivalent:key]
    }

    unsafe fn setKeyEquivalentModifierMask_(self, mask: NSEventModifierFlags) {
        msg_send![self, setKeyEquivalentModifierMask: mask]
    }

    unsafe fn setSubmenu_(self, submenu: id) {
        msg_send![self, setSubmenu: submenu]
    }

    unsafe fn setTarget_(self, target: id) {
        msg_send![self, setTarget: target]
    }
}

pub trait NSProcessInfo: Sized {
    unsafe fn processInfo(_: Self) -> id {
        msg_send![class!(NSProcessInfo), processInfo]
    }

    unsafe fn processName(self) -> id;
    // unsafe fn operatingSystemVersion(self) -> NSOperatingSystemVersion;
    // unsafe fn isOperatingSystemAtLeastVersion(self, version: NSOperatingSystemVersion) -> bool;
}

impl NSProcessInfo for id {
    unsafe fn processName(self) -> id {
        msg_send![self, processName]
    }

    // unsafe fn operatingSystemVersion(self) -> NSOperatingSystemVersion {
    //     msg_send![self, operatingSystemVersion]
    // }
    //
    // unsafe fn isOperatingSystemAtLeastVersion(self, version: NSOperatingSystemVersion) -> bool {
    //     msg_send![self, isOperatingSystemAtLeastVersion: version]
    // }
}

pub trait NSScreen: Sized {
    // Getting NSScreen Objects
    unsafe fn mainScreen(_: Self) -> id /* (NSScreen *) */;
    //  unsafe fn deepestScreen(_: Self) -> id /* (NSScreen *) */;
    unsafe fn screens(_: Self) -> id /* (NSArray *) */;
    //
    //  // Getting Screen Information
    //  unsafe fn depth(self) -> NSWindowDepth;
    unsafe fn frame(self) -> NSRect;
    //  unsafe fn supportedWindowDepths(self) -> *const NSWindowDepth;
    unsafe fn deviceDescription(self) -> id /* (NSDictionary *) */;
    unsafe fn visibleFrame(self) -> NSRect;
    //  unsafe fn colorSpace(self) -> id /* (NSColorSpace *) */;
    //  unsafe fn screensHaveSeparateSpaces(_: Self) -> BOOL;
    //
    //  // Screen Backing Coordinate Conversion
    //  unsafe fn backingAlignedRect_options_(
    //      self,
    //      aRect: NSRect,
    //      options: NSAlignmentOptions,
    //  ) -> NSRect;
    unsafe fn backingScaleFactor(self) -> CGFloat;
    //  unsafe fn convertRectFromBacking_(self, aRect: NSRect) -> NSRect;
    //  unsafe fn convertRectToBacking_(self, aRect: NSRect) -> NSRect;
}

impl NSScreen for id {
    // Getting NSScreen Objects

    unsafe fn mainScreen(_: Self) -> id /* (NSScreen *) */ {
        msg_send![class!(NSScreen), mainScreen]
    }

    /*
            unsafe fn deepestScreen(_: Self) -> id /* (NSScreen *) */ {
                msg_send![class!(NSScreen), deepestScreen]
            }

    */
    unsafe fn screens(_: Self) -> id /* (NSArray *) */ {
        msg_send![class!(NSScreen), screens]
    }

    /*
              // Getting Screen Information

              unsafe fn depth(self) -> NSWindowDepth {
                  msg_send![self, depth]
              }
    */
    unsafe fn frame(self) -> NSRect {
        msg_send![self, frame]
    }
    /*
          unsafe fn supportedWindowDepths(self) -> *const NSWindowDepth {
              msg_send![self, supportedWindowDepths]
          }
    */
    unsafe fn deviceDescription(self) -> id /* (NSDictionary *) */ {
        msg_send![self, deviceDescription]
    }

    unsafe fn visibleFrame(self) -> NSRect {
        msg_send![self, visibleFrame]
    }

    /*
        unsafe fn colorSpace(self) -> id /* (NSColorSpace *) */ {
            msg_send![self, colorSpace]
        }

        unsafe fn screensHaveSeparateSpaces(_: Self) -> BOOL {
            msg_send![class!(NSScreen), screensHaveSeparateSpaces]
        }

        // Screen Backing Coordinate Conversion

        unsafe fn backingAlignedRect_options_(
            self,
            aRect: NSRect,
            options: NSAlignmentOptions,
        ) -> NSRect {
            msg_send![self, backingAlignedRect:aRect options:options]
        }
    */
    unsafe fn backingScaleFactor(self) -> CGFloat {
        msg_send![self, backingScaleFactor]
    }

    /*

    unsafe fn convertRectFromBacking_(self, aRect: NSRect) -> NSRect {
        msg_send![self, convertRectFromBacking: aRect]
    }

    unsafe fn convertRectToBacking_(self, aRect: NSRect) -> NSRect {
        msg_send![self, convertRectToBacking: aRect]
    }
    */
}

pub trait NSView: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSView), alloc]
    }

    unsafe fn init(self) -> id;
    unsafe fn initWithFrame_(self, frameRect: NSRect) -> id;
    unsafe fn bounds(self) -> NSRect;
    unsafe fn frame(self) -> NSRect;
    unsafe fn setFrameSize(self, frameSize: NSSize);
    unsafe fn setFrameOrigin(self, frameOrigin: NSPoint);
    unsafe fn display_(self);
    unsafe fn setWantsBestResolutionOpenGLSurface_(self, flag: BOOL);
    unsafe fn convertPoint_fromView_(self, point: NSPoint, view: id) -> NSPoint;
    unsafe fn addSubview_(self, view: id);
    unsafe fn superview(self) -> id;
    unsafe fn removeFromSuperview(self);
    //  unsafe fn setAutoresizingMask_(self, autoresizingMask: NSAutoresizingMaskOptions);

    unsafe fn wantsLayer(self) -> BOOL;
    unsafe fn setWantsLayer(self, wantsLayer: BOOL);
    unsafe fn layer(self) -> id;
    unsafe fn setLayer(self, layer: id);

    unsafe fn widthAnchor(self) -> id;
    unsafe fn heightAnchor(self) -> id;
    unsafe fn convertRectToBacking(self, rect: NSRect) -> NSRect;

    // unsafe fn layerContentsPlacement(self) -> NSViewLayerContentsPlacement;
    // unsafe fn setLayerContentsPlacement(self, placement: NSViewLayerContentsPlacement);
}

impl NSView for id {
    unsafe fn init(self) -> id {
        msg_send![self, init]
    }

    unsafe fn initWithFrame_(self, frameRect: NSRect) -> id {
        msg_send![self, initWithFrame: frameRect]
    }

    unsafe fn bounds(self) -> NSRect {
        msg_send![self, bounds]
    }

    unsafe fn frame(self) -> NSRect {
        msg_send![self, frame]
    }

    unsafe fn setFrameSize(self, frameSize: NSSize) {
        msg_send![self, setFrameSize: frameSize]
    }

    unsafe fn setFrameOrigin(self, frameOrigin: NSPoint) {
        msg_send![self, setFrameOrigin: frameOrigin]
    }

    unsafe fn display_(self) {
        msg_send![self, display]
    }

    unsafe fn setWantsBestResolutionOpenGLSurface_(self, flag: BOOL) {
        msg_send![self, setWantsBestResolutionOpenGLSurface: flag]
    }

    unsafe fn convertPoint_fromView_(self, point: NSPoint, view: id) -> NSPoint {
        msg_send![self, convertPoint:point fromView:view]
    }

    unsafe fn addSubview_(self, view: id) {
        msg_send![self, addSubview: view]
    }

    unsafe fn superview(self) -> id {
        msg_send![self, superview]
    }

    unsafe fn removeFromSuperview(self) {
        msg_send![self, removeFromSuperview]
    }

    // unsafe fn setAutoresizingMask_(self, autoresizingMask: NSAutoresizingMaskOptions) {
    //     msg_send![self, setAutoresizingMask: autoresizingMask]
    // }

    unsafe fn wantsLayer(self) -> BOOL {
        msg_send![self, wantsLayer]
    }

    unsafe fn setWantsLayer(self, wantsLayer: BOOL) {
        msg_send![self, setWantsLayer: wantsLayer]
    }

    unsafe fn layer(self) -> id {
        msg_send![self, layer]
    }

    unsafe fn setLayer(self, layer: id) {
        msg_send![self, setLayer: layer]
    }

    unsafe fn widthAnchor(self) -> id {
        msg_send![self, widthAnchor]
    }

    unsafe fn heightAnchor(self) -> id {
        msg_send![self, heightAnchor]
    }

    unsafe fn convertRectToBacking(self, rect: NSRect) -> NSRect {
        msg_send![self, convertRectToBacking: rect]
    }

    // unsafe fn layerContentsPlacement(self) -> NSViewLayerContentsPlacement {
    //     msg_send![self, layerContentsPlacement]
    // }
    //
    // unsafe fn setLayerContentsPlacement(self, placement: NSViewLayerContentsPlacement) {
    //     msg_send![self, setLayerContentsPlacement: placement]
    // }
}

pub trait NSWindow: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSWindow), alloc]
    }

    // Creating Windows
    unsafe fn initWithContentRect_styleMask_backing_defer_(
        self,
        rect: NSRect,
        style: NSWindowStyleMask,
        backing: NSBackingStoreType,
        defer: BOOL,
    ) -> id;
    unsafe fn initWithContentRect_styleMask_backing_defer_screen_(
        self,
        rect: NSRect,
        style: NSWindowStyleMask,
        backing: NSBackingStoreType,
        defer: BOOL,
        screen: id,
    ) -> id;

    // Configuring Windows
    unsafe fn styleMask(self) -> NSWindowStyleMask;
    unsafe fn setStyleMask_(self, styleMask: NSWindowStyleMask);
    unsafe fn toggleFullScreen_(self, sender: id);
    unsafe fn worksWhenModal(self) -> BOOL;
    unsafe fn alphaValue(self) -> CGFloat;
    unsafe fn setAlphaValue_(self, windowAlpha: CGFloat);
    unsafe fn backgroundColor(self) -> id;
    unsafe fn setBackgroundColor_(self, color: id);
    unsafe fn colorSpace(self) -> id;
    unsafe fn setColorSpace_(self, colorSpace: id);
    unsafe fn contentView(self) -> id;
    unsafe fn setContentView_(self, view: id);
    unsafe fn canHide(self) -> BOOL;
    unsafe fn setCanHide_(self, canHide: BOOL);
    unsafe fn hidesOnDeactivate(self) -> BOOL;
    unsafe fn setHidesOnDeactivate_(self, hideOnDeactivate: BOOL);
    // unsafe fn collectionBehavior(self) -> NSWindowCollectionBehavior;
    // unsafe fn setCollectionBehavior_(self, collectionBehavior: NSWindowCollectionBehavior);
    unsafe fn setOpaque_(self, opaque: BOOL);
    unsafe fn hasShadow(self) -> BOOL;
    unsafe fn setHasShadow_(self, hasShadow: BOOL);
    unsafe fn invalidateShadow(self);
    //  unsafe fn autorecalculatesContentBorderThicknessForEdge_(self, edge: NSRectEdge) -> BOOL;
    //  unsafe fn setAutorecalculatesContentBorderThickness_forEdge_(
    //      self,
    //      autorecalculateContentBorderThickness: BOOL,
    //      edge: NSRectEdge,
    //  ) -> BOOL;
    //  unsafe fn contentBorderThicknessForEdge_(self, edge: NSRectEdge) -> CGFloat;
    // unsafe fn setContentBorderThickness_forEdge_(self, borderThickness: CGFloat, edge: NSRectEdge);
    unsafe fn delegate(self) -> id;
    unsafe fn setDelegate_(self, delegate: id);
    unsafe fn preventsApplicationTerminationWhenModal(self) -> BOOL;
    unsafe fn setPreventsApplicationTerminationWhenModal_(self, flag: BOOL);

    // TODO: Accessing Window Information

    // Getting Layout Information
    unsafe fn contentRectForFrameRect_styleMask_(
        self,
        windowFrame: NSRect,
        windowStyle: NSWindowStyleMask,
    ) -> NSRect;
    unsafe fn frameRectForContentRect_styleMask_(
        self,
        windowContentRect: NSRect,
        windowStyle: NSWindowStyleMask,
    ) -> NSRect;
    unsafe fn minFrameWidthWithTitle_styleMask_(
        self,
        windowTitle: id,
        windowStyle: NSWindowStyleMask,
    ) -> CGFloat;
    unsafe fn contentRectForFrameRect_(self, windowFrame: NSRect) -> NSRect;
    unsafe fn frameRectForContentRect_(self, windowContent: NSRect) -> NSRect;

    // Managing Windows
    unsafe fn drawers(self) -> id;
    unsafe fn windowController(self) -> id;
    unsafe fn setWindowController_(self, windowController: id);

    // TODO: Managing Sheets

    // Sizing Windows
    unsafe fn frame(self) -> NSRect;
    unsafe fn setFrameOrigin_(self, point: NSPoint);
    unsafe fn setFrameTopLeftPoint_(self, point: NSPoint);
    unsafe fn constrainFrameRect_toScreen_(self, frameRect: NSRect, screen: id);
    unsafe fn cascadeTopLeftFromPoint_(self, topLeft: NSPoint) -> NSPoint;
    unsafe fn setFrame_display_(self, windowFrame: NSRect, display: BOOL);
    unsafe fn setFrame_displayViews_(self, windowFrame: NSRect, display: BOOL);
    unsafe fn aspectRatio(self) -> NSSize;
    unsafe fn setAspectRatio_(self, aspectRatio: NSSize);
    unsafe fn minSize(self) -> NSSize;
    unsafe fn setMinSize_(self, minSize: NSSize);
    unsafe fn maxSize(self) -> NSSize;
    unsafe fn setMaxSize_(self, maxSize: NSSize);
    unsafe fn performZoom_(self, sender: id);
    unsafe fn zoom_(self, sender: id);
    unsafe fn resizeFlags(self) -> NSInteger;
    unsafe fn showsResizeIndicator(self) -> BOOL;
    unsafe fn setShowsResizeIndicator_(self, showsResizeIndicator: BOOL);
    unsafe fn resizeIncrements(self) -> NSSize;
    unsafe fn setResizeIncrements_(self, resizeIncrements: NSSize);
    unsafe fn preservesContentDuringLiveResize(self) -> BOOL;
    unsafe fn setPreservesContentDuringLiveResize_(self, preservesContentDuringLiveResize: BOOL);
    unsafe fn inLiveResize(self) -> BOOL;

    // Sizing Content
    unsafe fn contentAspectRatio(self) -> NSSize;
    unsafe fn setContentAspectRatio_(self, contentAspectRatio: NSSize);
    unsafe fn contentMinSize(self) -> NSSize;
    unsafe fn setContentMinSize_(self, contentMinSize: NSSize);
    unsafe fn contentSize(self) -> NSSize;
    unsafe fn setContentSize_(self, contentSize: NSSize);
    unsafe fn contentMaxSize(self) -> NSSize;
    unsafe fn setContentMaxSize_(self, contentMaxSize: NSSize);
    unsafe fn contentResizeIncrements(self) -> NSSize;
    unsafe fn setContentResizeIncrements_(self, contentResizeIncrements: NSSize);

    // Managing Window Visibility and Occlusion State
    unsafe fn isVisible(self) -> BOOL; // NOTE: Deprecated in 10.9
                                       // unsafe fn occlusionState(self) -> NSWindowOcclusionState;

    // Managing Window Layers
    unsafe fn orderOut_(self, sender: id);
    unsafe fn orderBack_(self, sender: id);
    unsafe fn orderFront_(self, sender: id);
    unsafe fn orderFrontRegardless(self);
    //  unsafe fn orderFrontWindow_relativeTo_(
    //      self,
    //      orderingMode: NSWindowOrderingMode,
    //      otherWindowNumber: NSInteger,
    //  );
    unsafe fn level(self) -> NSInteger;
    unsafe fn setLevel_(self, level: NSInteger);

    // Managing Key Status
    unsafe fn isKeyWindow(self) -> BOOL;
    unsafe fn canBecomeKeyWindow(self) -> BOOL;
    unsafe fn makeKeyWindow(self);
    unsafe fn makeKeyAndOrderFront_(self, sender: id);
    // skipped: becomeKeyWindow (should not be invoked directly, according to Apple's documentation)
    // skipped: resignKeyWindow (should not be invoked directly, according to Apple's documentation)

    // Managing Main Status
    unsafe fn canBecomeMainWindow(self) -> BOOL;
    unsafe fn makeMainWindow(self);
    // skipped: becomeMainWindow (should not be invoked directly, according to Apple's documentation)
    // skipped: resignMainWindow (should not be invoked directly, according to Apple's documentation)

    // Managing Toolbars
    unsafe fn toolbar(self) -> id /* NSToolbar */;
    unsafe fn setToolbar_(self, toolbar: id /* NSToolbar */);
    unsafe fn runToolbarCustomizationPalette(self, sender: id);

    // TODO: Managing Attached Windows
    // TODO: Managing Window Buffers
    // TODO: Managing Default Buttons
    // TODO: Managing Field Editors
    // TODO: Managing the Window Menu
    // TODO: Managing Cursor Rectangles

    // Managing Title Bars
    unsafe fn standardWindowButton_(self, windowButtonKind: NSWindowButton) -> id;

    // Managing Window Tabs
    unsafe fn allowsAutomaticWindowTabbing(_: Self) -> BOOL;
    unsafe fn setAllowsAutomaticWindowTabbing_(_: Self, allowsAutomaticWindowTabbing: BOOL);
    unsafe fn tabbingIdentifier(self) -> id;
    // unsafe fn tabbingMode(self) -> NSWindowTabbingMode;
    // unsafe fn setTabbingMode_(self, tabbingMode: NSWindowTabbingMode);
    // unsafe fn addTabbedWindow_ordered_(self, window: id, ordering_mode: NSWindowOrderingMode);
    unsafe fn toggleTabBar_(self, sender: id);

    // TODO: Managing Tooltips
    // TODO: Handling Events

    // Managing Responders
    unsafe fn initialFirstResponder(self) -> id;
    unsafe fn firstResponder(self) -> id;
    unsafe fn setInitialFirstResponder_(self, responder: id);
    unsafe fn makeFirstResponder_(self, responder: id) -> BOOL;

    // TODO: Managing the Key View Loop

    // Handling Keyboard Events
    unsafe fn keyDown_(self, event: id);

    // Handling Mouse Events
    unsafe fn acceptsMouseMovedEvents(self) -> BOOL;
    unsafe fn ignoresMouseEvents(self) -> BOOL;
    unsafe fn setIgnoresMouseEvents_(self, ignoreMouseEvents: BOOL);
    unsafe fn mouseLocationOutsideOfEventStream(self) -> NSPoint;
    unsafe fn setAcceptsMouseMovedEvents_(self, acceptMouseMovedEvents: BOOL);
    unsafe fn windowNumberAtPoint_belowWindowWithWindowNumber_(
        self,
        point: NSPoint,
        windowNumber: NSInteger,
    ) -> NSInteger;

    // TODO: Handling Window Restoration
    // TODO: Bracketing Drawing Operations
    // TODO: Drawing Windows
    // TODO: Window Animation
    // TODO: Updating Windows
    // TODO: Dragging Items

    // Converting Coordinates
    unsafe fn backingScaleFactor(self) -> CGFloat;
    //  unsafe fn backingAlignedRect_options_(
    //      self,
    //      rect: NSRect,
    //      options: NSAlignmentOptions,
    //  ) -> NSRect;
    unsafe fn convertRectFromBacking_(self, rect: NSRect) -> NSRect;
    unsafe fn convertRectToBacking_(self, rect: NSRect) -> NSRect;
    unsafe fn convertRectToScreen_(self, rect: NSRect) -> NSRect;
    unsafe fn convertRectFromScreen_(self, rect: NSRect) -> NSRect;

    // Accessing Edited Status
    unsafe fn setDocumentEdited_(self, documentEdited: BOOL);

    // Managing Titles
    unsafe fn title(self) -> id;
    unsafe fn setTitle_(self, title: id);
    unsafe fn setTitleWithRepresentedFilename_(self, filePath: id);
    unsafe fn setTitleVisibility_(self, visibility: NSWindowTitleVisibility);
    unsafe fn setTitlebarAppearsTransparent_(self, transparent: BOOL);
    unsafe fn representedFilename(self) -> id;
    unsafe fn setRepresentedFilename_(self, filePath: id);
    unsafe fn representedURL(self) -> id;
    unsafe fn setRepresentedURL_(self, representedURL: id);

    // Accessing Screen Information
    unsafe fn screen(self) -> id;
    unsafe fn deepestScreen(self) -> id;
    unsafe fn displaysWhenScreenProfileChanges(self) -> BOOL;
    unsafe fn setDisplaysWhenScreenProfileChanges_(self, displaysWhenScreenProfileChanges: BOOL);

    // Moving Windows
    unsafe fn setMovableByWindowBackground_(self, movableByWindowBackground: BOOL);
    unsafe fn setMovable_(self, movable: BOOL);
    unsafe fn center(self);

    // Closing Windows
    unsafe fn performClose_(self, sender: id);
    unsafe fn close(self);
    unsafe fn setReleasedWhenClosed_(self, releasedWhenClosed: BOOL);

    // Minimizing Windows
    unsafe fn performMiniaturize_(self, sender: id);
    unsafe fn miniaturize_(self, sender: id);
    unsafe fn deminiaturize_(self, sender: id);
    unsafe fn miniwindowImage(self) -> id;
    unsafe fn setMiniwindowImage_(self, miniwindowImage: id);
    unsafe fn miniwindowTitle(self) -> id;
    unsafe fn setMiniwindowTitle_(self, miniwindowTitle: id);

    // TODO: Getting the Dock Tile
    // TODO: Printing Windows
    // TODO: Providing Services
    // TODO: Working with Carbon
    // TODO: Triggering Constraint-Based Layout
    // TODO: Debugging Constraint-Based Layout
    // TODO: Constraint-Based Layouts
}

impl NSWindow for id {
    // Creating Windows

    unsafe fn initWithContentRect_styleMask_backing_defer_(
        self,
        rect: NSRect,
        style: NSWindowStyleMask,
        backing: NSBackingStoreType,
        defer: BOOL,
    ) -> id {
        msg_send![self, initWithContentRect:rect
                                  styleMask:style.bits
                                    backing:backing as NSUInteger
                                      defer:defer]
    }

    unsafe fn initWithContentRect_styleMask_backing_defer_screen_(
        self,
        rect: NSRect,
        style: NSWindowStyleMask,
        backing: NSBackingStoreType,
        defer: BOOL,
        screen: id,
    ) -> id {
        msg_send![self, initWithContentRect:rect
                                  styleMask:style.bits
                                    backing:backing as NSUInteger
                                      defer:defer
                                     screen:screen]
    }

    // Configuring Windows

    unsafe fn styleMask(self) -> NSWindowStyleMask {
        NSWindowStyleMask::from_bits_truncate(msg_send![self, styleMask])
    }

    unsafe fn setStyleMask_(self, styleMask: NSWindowStyleMask) {
        msg_send![self, setStyleMask:styleMask.bits]
    }

    unsafe fn toggleFullScreen_(self, sender: id) {
        msg_send![self, toggleFullScreen: sender]
    }

    unsafe fn worksWhenModal(self) -> BOOL {
        msg_send![self, worksWhenModal]
    }

    unsafe fn alphaValue(self) -> CGFloat {
        msg_send![self, alphaValue]
    }

    unsafe fn setAlphaValue_(self, windowAlpha: CGFloat) {
        msg_send![self, setAlphaValue: windowAlpha]
    }

    unsafe fn backgroundColor(self) -> id {
        msg_send![self, backgroundColor]
    }

    unsafe fn setBackgroundColor_(self, color: id) {
        msg_send![self, setBackgroundColor: color]
    }

    unsafe fn colorSpace(self) -> id {
        msg_send![self, colorSpace]
    }

    unsafe fn setColorSpace_(self, colorSpace: id) {
        msg_send![self, setColorSpace: colorSpace]
    }

    unsafe fn contentView(self) -> id {
        msg_send![self, contentView]
    }

    unsafe fn setContentView_(self, view: id) {
        msg_send![self, setContentView: view]
    }

    unsafe fn canHide(self) -> BOOL {
        msg_send![self, canHide]
    }

    unsafe fn setCanHide_(self, canHide: BOOL) {
        msg_send![self, setCanHide: canHide]
    }

    unsafe fn hidesOnDeactivate(self) -> BOOL {
        msg_send![self, hidesOnDeactivate]
    }

    unsafe fn setHidesOnDeactivate_(self, hideOnDeactivate: BOOL) {
        msg_send![self, setHidesOnDeactivate: hideOnDeactivate]
    }

    // unsafe fn collectionBehavior(self) -> NSWindowCollectionBehavior {
    //     msg_send![self, collectionBehavior]
    // }
    //
    //     // unsafe fn setCollectionBehavior_(self, collectionBehavior: NSWindowCollectionBehavior) {
    //     msg_send![self, setCollectionBehavior: collectionBehavior]
    // }

    unsafe fn setOpaque_(self, opaque: BOOL) {
        msg_send![self, setOpaque: opaque]
    }

    unsafe fn hasShadow(self) -> BOOL {
        msg_send![self, hasShadow]
    }

    unsafe fn setHasShadow_(self, hasShadow: BOOL) {
        msg_send![self, setHasShadow: hasShadow]
    }

    unsafe fn invalidateShadow(self) {
        msg_send![self, invalidateShadow]
    }

    //  unsafe fn autorecalculatesContentBorderThicknessForEdge_(self, edge: NSRectEdge) -> BOOL {
    //      msg_send![self, autorecalculatesContentBorderThicknessForEdge: edge]
    //  }
    //
    //  unsafe fn setAutorecalculatesContentBorderThickness_forEdge_(
    //      self,
    //      autorecalculateContentBorderThickness: BOOL,
    //      edge: NSRectEdge,
    //  ) -> BOOL {
    //      msg_send![self, setAutorecalculatesContentBorderThickness:
    //                      autorecalculateContentBorderThickness forEdge:edge]
    //  }
    //
    //  unsafe fn contentBorderThicknessForEdge_(self, edge: NSRectEdge) -> CGFloat {
    //      msg_send![self, contentBorderThicknessForEdge: edge]
    //  }
    //
    //  unsafe fn setContentBorderThickness_forEdge_(self, borderThickness: CGFloat, edge: NSRectEdge) {
    //      msg_send![self, setContentBorderThickness:borderThickness forEdge:edge]
    //  }

    unsafe fn delegate(self) -> id {
        msg_send![self, delegate]
    }

    unsafe fn setDelegate_(self, delegate: id) {
        msg_send![self, setDelegate: delegate]
    }

    unsafe fn preventsApplicationTerminationWhenModal(self) -> BOOL {
        msg_send![self, preventsApplicationTerminationWhenModal]
    }

    unsafe fn setPreventsApplicationTerminationWhenModal_(self, flag: BOOL) {
        msg_send![self, setPreventsApplicationTerminationWhenModal: flag]
    }

    // TODO: Accessing Window Information

    // Getting Layout Information

    unsafe fn contentRectForFrameRect_styleMask_(
        self,
        windowFrame: NSRect,
        windowStyle: NSWindowStyleMask,
    ) -> NSRect {
        msg_send![self, contentRectForFrameRect:windowFrame styleMask:windowStyle.bits]
    }

    unsafe fn frameRectForContentRect_styleMask_(
        self,
        windowContentRect: NSRect,
        windowStyle: NSWindowStyleMask,
    ) -> NSRect {
        msg_send![self, frameRectForContentRect:windowContentRect styleMask:windowStyle.bits]
    }

    unsafe fn minFrameWidthWithTitle_styleMask_(
        self,
        windowTitle: id,
        windowStyle: NSWindowStyleMask,
    ) -> CGFloat {
        msg_send![self, minFrameWidthWithTitle:windowTitle styleMask:windowStyle.bits]
    }

    unsafe fn contentRectForFrameRect_(self, windowFrame: NSRect) -> NSRect {
        msg_send![self, contentRectForFrameRect: windowFrame]
    }

    unsafe fn frameRectForContentRect_(self, windowContent: NSRect) -> NSRect {
        msg_send![self, frameRectForContentRect: windowContent]
    }

    // Managing Windows

    unsafe fn drawers(self) -> id {
        msg_send![self, drawers]
    }

    unsafe fn windowController(self) -> id {
        msg_send![self, windowController]
    }

    unsafe fn setWindowController_(self, windowController: id) {
        msg_send![self, setWindowController: windowController]
    }

    // TODO: Managing Sheets

    // Sizing Windows

    unsafe fn frame(self) -> NSRect {
        msg_send![self, frame]
    }

    unsafe fn setFrameOrigin_(self, point: NSPoint) {
        msg_send![self, setFrameOrigin: point]
    }

    unsafe fn setFrameTopLeftPoint_(self, point: NSPoint) {
        msg_send![self, setFrameTopLeftPoint: point]
    }

    unsafe fn constrainFrameRect_toScreen_(self, frameRect: NSRect, screen: id) {
        msg_send![self, constrainFrameRect:frameRect toScreen:screen]
    }

    unsafe fn cascadeTopLeftFromPoint_(self, topLeft: NSPoint) -> NSPoint {
        msg_send![self, cascadeTopLeftFromPoint: topLeft]
    }

    unsafe fn setFrame_display_(self, windowFrame: NSRect, display: BOOL) {
        msg_send![self, setFrame:windowFrame display:display]
    }

    unsafe fn setFrame_displayViews_(self, windowFrame: NSRect, display: BOOL) {
        msg_send![self, setFrame:windowFrame displayViews:display]
    }

    unsafe fn aspectRatio(self) -> NSSize {
        msg_send![self, aspectRatio]
    }

    unsafe fn setAspectRatio_(self, aspectRatio: NSSize) {
        msg_send![self, setAspectRatio: aspectRatio]
    }

    unsafe fn minSize(self) -> NSSize {
        msg_send![self, minSize]
    }

    unsafe fn setMinSize_(self, minSize: NSSize) {
        msg_send![self, setMinSize: minSize]
    }

    unsafe fn maxSize(self) -> NSSize {
        msg_send![self, maxSize]
    }

    unsafe fn setMaxSize_(self, maxSize: NSSize) {
        msg_send![self, setMaxSize: maxSize]
    }

    unsafe fn performZoom_(self, sender: id) {
        msg_send![self, performZoom: sender]
    }

    unsafe fn zoom_(self, sender: id) {
        msg_send![self, zoom: sender]
    }

    unsafe fn resizeFlags(self) -> NSInteger {
        msg_send![self, resizeFlags]
    }

    unsafe fn showsResizeIndicator(self) -> BOOL {
        msg_send![self, showsResizeIndicator]
    }

    unsafe fn setShowsResizeIndicator_(self, showsResizeIndicator: BOOL) {
        msg_send![self, setShowsResizeIndicator: showsResizeIndicator]
    }

    unsafe fn resizeIncrements(self) -> NSSize {
        msg_send![self, resizeIncrements]
    }

    unsafe fn setResizeIncrements_(self, resizeIncrements: NSSize) {
        msg_send![self, setResizeIncrements: resizeIncrements]
    }

    unsafe fn preservesContentDuringLiveResize(self) -> BOOL {
        msg_send![self, preservesContentDuringLiveResize]
    }

    unsafe fn setPreservesContentDuringLiveResize_(self, preservesContentDuringLiveResize: BOOL) {
        msg_send![
            self,
            setPreservesContentDuringLiveResize: preservesContentDuringLiveResize
        ]
    }

    unsafe fn inLiveResize(self) -> BOOL {
        msg_send![self, inLiveResize]
    }

    // Sizing Content

    unsafe fn contentAspectRatio(self) -> NSSize {
        msg_send![self, contentAspectRatio]
    }

    unsafe fn setContentAspectRatio_(self, contentAspectRatio: NSSize) {
        msg_send![self, setContentAspectRatio: contentAspectRatio]
    }

    unsafe fn contentMinSize(self) -> NSSize {
        msg_send![self, contentMinSize]
    }

    unsafe fn setContentMinSize_(self, contentMinSize: NSSize) {
        msg_send![self, setContentMinSize: contentMinSize]
    }

    unsafe fn contentSize(self) -> NSSize {
        msg_send![self, contentSize]
    }

    unsafe fn setContentSize_(self, contentSize: NSSize) {
        msg_send![self, setContentSize: contentSize]
    }

    unsafe fn contentMaxSize(self) -> NSSize {
        msg_send![self, contentMaxSize]
    }

    unsafe fn setContentMaxSize_(self, contentMaxSize: NSSize) {
        msg_send![self, setContentMaxSize: contentMaxSize]
    }

    unsafe fn contentResizeIncrements(self) -> NSSize {
        msg_send![self, contentResizeIncrements]
    }

    unsafe fn setContentResizeIncrements_(self, contentResizeIncrements: NSSize) {
        msg_send![self, setContentResizeIncrements: contentResizeIncrements]
    }

    // Managing Window Visibility and Occlusion State

    unsafe fn isVisible(self) -> BOOL {
        msg_send![self, isVisible]
    }

    // unsafe fn occlusionState(self) -> NSWindowOcclusionState {
    //     msg_send![self, occlusionState]
    // }

    // Managing Window Layers

    unsafe fn orderOut_(self, sender: id) {
        msg_send![self, orderOut: sender]
    }

    unsafe fn orderBack_(self, sender: id) {
        msg_send![self, orderBack: sender]
    }

    unsafe fn orderFront_(self, sender: id) {
        msg_send![self, orderFront: sender]
    }

    unsafe fn orderFrontRegardless(self) {
        msg_send![self, orderFrontRegardless]
    }

    // unsafe fn orderFrontWindow_relativeTo_(
    //     self,
    //     ordering_mode: NSWindowOrderingMode,
    //     other_window_number: NSInteger,
    // ) {
    //     msg_send![self, orderWindow:ordering_mode relativeTo:other_window_number]
    // }

    unsafe fn level(self) -> NSInteger {
        msg_send![self, level]
    }

    unsafe fn setLevel_(self, level: NSInteger) {
        msg_send![self, setLevel: level]
    }

    // Managing Key Status

    unsafe fn isKeyWindow(self) -> BOOL {
        msg_send![self, isKeyWindow]
    }

    unsafe fn canBecomeKeyWindow(self) -> BOOL {
        msg_send![self, canBecomeKeyWindow]
    }

    unsafe fn makeKeyWindow(self) {
        msg_send![self, makeKeyWindow]
    }

    unsafe fn makeKeyAndOrderFront_(self, sender: id) {
        msg_send![self, makeKeyAndOrderFront: sender]
    }

    // Managing Main Status

    unsafe fn canBecomeMainWindow(self) -> BOOL {
        msg_send![self, canBecomeMainWindow]
    }

    unsafe fn makeMainWindow(self) {
        msg_send![self, makeMainWindow]
    }

    // Managing Toolbars

    unsafe fn toolbar(self) -> id /* NSToolbar */ {
        msg_send![self, toolbar]
    }

    unsafe fn setToolbar_(self, toolbar: id /* NSToolbar */) {
        msg_send![self, setToolbar: toolbar]
    }

    unsafe fn runToolbarCustomizationPalette(self, sender: id) {
        msg_send![self, runToolbarCustomizationPalette: sender]
    }

    // TODO: Managing Attached Windows
    // TODO: Managing Window Buffers
    // TODO: Managing Default Buttons
    // TODO: Managing Field Editors
    // TODO: Managing the Window Menu
    // TODO: Managing Cursor Rectangles

    // Managing Title Bars

    unsafe fn standardWindowButton_(self, windowButtonKind: NSWindowButton) -> id {
        msg_send![self, standardWindowButton: windowButtonKind]
    }

    // Managing Window Tabs
    unsafe fn allowsAutomaticWindowTabbing(_: Self) -> BOOL {
        msg_send![class!(NSWindow), allowsAutomaticWindowTabbing]
    }

    unsafe fn setAllowsAutomaticWindowTabbing_(_: Self, allowsAutomaticWindowTabbing: BOOL) {
        msg_send![
            class!(NSWindow),
            setAllowsAutomaticWindowTabbing: allowsAutomaticWindowTabbing
        ]
    }

    unsafe fn tabbingIdentifier(self) -> id {
        msg_send![self, tabbingIdentifier]
    }

    //  unsafe fn tabbingMode(self) -> NSWindowTabbingMode {
    //      msg_send!(self, tabbingMode)
    //  }
    //
    //  unsafe fn setTabbingMode_(self, tabbingMode: NSWindowTabbingMode) {
    //      msg_send![self, setTabbingMode: tabbingMode]
    //  }
    //
    //  unsafe fn addTabbedWindow_ordered_(self, window: id, ordering_mode: NSWindowOrderingMode) {
    //      msg_send![self, addTabbedWindow:window ordered: ordering_mode]
    //  }

    unsafe fn toggleTabBar_(self, sender: id) {
        msg_send![self, toggleTabBar: sender]
    }
    // TODO: Managing Tooltips
    // TODO: Handling Events

    // Managing Responders

    unsafe fn initialFirstResponder(self) -> id {
        msg_send![self, initialFirstResponder]
    }

    unsafe fn firstResponder(self) -> id {
        msg_send![self, firstResponder]
    }

    unsafe fn setInitialFirstResponder_(self, responder: id) {
        msg_send![self, setInitialFirstResponder: responder]
    }

    unsafe fn makeFirstResponder_(self, responder: id) -> BOOL {
        msg_send![self, makeFirstResponder: responder]
    }

    // TODO: Managing the Key View Loop

    // Handling Keyboard Events

    unsafe fn keyDown_(self, event: id) {
        msg_send![self, keyDown: event]
    }

    // Handling Mouse Events

    unsafe fn acceptsMouseMovedEvents(self) -> BOOL {
        msg_send![self, acceptsMouseMovedEvents]
    }

    unsafe fn ignoresMouseEvents(self) -> BOOL {
        msg_send![self, ignoresMouseEvents]
    }

    unsafe fn setIgnoresMouseEvents_(self, ignoreMouseEvents: BOOL) {
        msg_send![self, setIgnoresMouseEvents: ignoreMouseEvents]
    }

    unsafe fn mouseLocationOutsideOfEventStream(self) -> NSPoint {
        msg_send![self, mouseLocationOutsideOfEventStream]
    }

    unsafe fn setAcceptsMouseMovedEvents_(self, acceptMouseMovedEvents: BOOL) {
        msg_send![self, setAcceptsMouseMovedEvents: acceptMouseMovedEvents]
    }

    unsafe fn windowNumberAtPoint_belowWindowWithWindowNumber_(
        self,
        point: NSPoint,
        windowNumber: NSInteger,
    ) -> NSInteger {
        msg_send![self, windowNumberAtPoint:point belowWindowWithWindowNumber:windowNumber]
    }

    // Converting Coordinates

    unsafe fn backingScaleFactor(self) -> CGFloat {
        msg_send![self, backingScaleFactor]
    }

    //  unsafe fn backingAlignedRect_options_(
    //      self,
    //      rect: NSRect,
    //      options: NSAlignmentOptions,
    //  ) -> NSRect {
    //      msg_send![self, backingAlignedRect:rect options:options]
    //  }

    unsafe fn convertRectFromBacking_(self, rect: NSRect) -> NSRect {
        msg_send![self, convertRectFromBacking: rect]
    }

    unsafe fn convertRectToBacking_(self, rect: NSRect) -> NSRect {
        msg_send![self, convertRectToBacking: rect]
    }

    unsafe fn convertRectToScreen_(self, rect: NSRect) -> NSRect {
        msg_send![self, convertRectToScreen: rect]
    }

    unsafe fn convertRectFromScreen_(self, rect: NSRect) -> NSRect {
        msg_send![self, convertRectFromScreen: rect]
    }

    // Accessing Edited Status

    unsafe fn setDocumentEdited_(self, documentEdited: BOOL) {
        msg_send![self, setDocumentEdited: documentEdited]
    }

    // Managing Titles

    unsafe fn title(self) -> id {
        msg_send![self, title]
    }

    unsafe fn setTitle_(self, title: id) {
        msg_send![self, setTitle: title]
    }

    unsafe fn setTitleWithRepresentedFilename_(self, filePath: id) {
        msg_send![self, setTitleWithRepresentedFilename: filePath]
    }

    unsafe fn setTitleVisibility_(self, visibility: NSWindowTitleVisibility) {
        msg_send![self, setTitleVisibility: visibility]
    }

    unsafe fn setTitlebarAppearsTransparent_(self, transparent: BOOL) {
        msg_send![self, setTitlebarAppearsTransparent: transparent]
    }

    unsafe fn representedFilename(self) -> id {
        msg_send![self, representedFilename]
    }

    unsafe fn setRepresentedFilename_(self, filePath: id) {
        msg_send![self, setRepresentedFilename: filePath]
    }

    unsafe fn representedURL(self) -> id {
        msg_send![self, representedURL]
    }

    unsafe fn setRepresentedURL_(self, representedURL: id) {
        msg_send![self, setRepresentedURL: representedURL]
    }

    // Accessing Screen Information

    unsafe fn screen(self) -> id {
        msg_send![self, screen]
    }

    unsafe fn deepestScreen(self) -> id {
        msg_send![self, deepestScreen]
    }

    unsafe fn displaysWhenScreenProfileChanges(self) -> BOOL {
        msg_send![self, displaysWhenScreenProfileChanges]
    }

    unsafe fn setDisplaysWhenScreenProfileChanges_(self, displaysWhenScreenProfileChanges: BOOL) {
        msg_send![
            self,
            setDisplaysWhenScreenProfileChanges: displaysWhenScreenProfileChanges
        ]
    }

    // Moving Windows

    unsafe fn setMovableByWindowBackground_(self, movableByWindowBackground: BOOL) {
        msg_send![
            self,
            setMovableByWindowBackground: movableByWindowBackground
        ]
    }

    unsafe fn setMovable_(self, movable: BOOL) {
        msg_send![self, setMovable: movable]
    }

    unsafe fn center(self) {
        msg_send![self, center]
    }

    // Closing Windows

    unsafe fn performClose_(self, sender: id) {
        msg_send![self, performClose: sender]
    }

    unsafe fn close(self) {
        msg_send![self, close]
    }

    unsafe fn setReleasedWhenClosed_(self, releasedWhenClosed: BOOL) {
        msg_send![self, setReleasedWhenClosed: releasedWhenClosed]
    }

    // Minimizing Windows

    unsafe fn performMiniaturize_(self, sender: id) {
        msg_send![self, performMiniaturize: sender]
    }

    unsafe fn miniaturize_(self, sender: id) {
        msg_send![self, miniaturize: sender]
    }

    unsafe fn deminiaturize_(self, sender: id) {
        msg_send![self, deminiaturize: sender]
    }

    unsafe fn miniwindowImage(self) -> id {
        msg_send![self, miniwindowImage]
    }

    unsafe fn setMiniwindowImage_(self, miniwindowImage: id) {
        msg_send![self, setMiniwindowImage: miniwindowImage]
    }

    unsafe fn miniwindowTitle(self) -> id {
        msg_send![self, miniwindowTitle]
    }

    unsafe fn setMiniwindowTitle_(self, miniwindowTitle: id) {
        msg_send![self, setMiniwindowTitle: miniwindowTitle]
    }

    // TODO: Getting the Dock Tile
    // TODO: Printing Windows
    // TODO: Providing Services
    // TODO: Working with Carbon
    // TODO: Triggering Constraint-Based Layout
    // TODO: Debugging Constraint-Based Layout
    // TODO: Constraint-Based Layouts
}

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]

pub enum NSBackingStoreType {
    NSBackingStoreRetained = 0,
    NSBackingStoreNonretained = 1,
    NSBackingStoreBuffered = 2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]

pub enum NSWindowTitleVisibility {
    NSWindowTitleVisible = 0,
    NSWindowTitleHidden = 1,
}

pub trait NSPasteboard: Sized {
    unsafe fn generalPasteboard(_: Self) -> id {
        msg_send![class!(NSPasteboard), generalPasteboard]
    }

    unsafe fn pasteboardByFilteringData_ofType(_: Self, data: id, _type: id) -> id {
        msg_send![class!(NSPasteboard), pasteboardByFilteringData:data ofType:_type]
    }

    unsafe fn pasteboardByFilteringFile(_: Self, file: id) -> id {
        msg_send![class!(NSPasteboard), pasteboardByFilteringFile: file]
    }

    unsafe fn pasteboardByFilteringTypesInPasteboard(_: Self, pboard: id) -> id {
        msg_send![
            class!(NSPasteboard),
            pasteboardByFilteringTypesInPasteboard: pboard
        ]
    }

    unsafe fn pasteboardWithName(_: Self, name: id) -> id {
        msg_send![class!(NSPasteboard), pasteboardWithName: name]
    }

    unsafe fn pasteboardWithUniqueName(_: Self) -> id {
        msg_send![class!(NSPasteboard), pasteboardWithUniqueName]
    }

    unsafe fn releaseGlobally(self);

    unsafe fn clearContents(self) -> NSInteger;
    unsafe fn writeObjects(self, objects: id) -> BOOL;
    unsafe fn setData_forType(self, data: id, dataType: id) -> BOOL;
    unsafe fn setPropertyList_forType(self, plist: id, dataType: id) -> BOOL;
    unsafe fn setString_forType(self, string: id, dataType: id) -> BOOL;

    unsafe fn readObjectsForClasses_options(self, classArray: id, options: id) -> id;
    unsafe fn pasteboardItems(self) -> id;
    unsafe fn indexOfPasteboardItem(self, pasteboardItem: id) -> NSInteger;
    unsafe fn dataForType(self, dataType: id) -> id;
    unsafe fn propertyListForType(self, dataType: id) -> id;
    unsafe fn stringForType(self, dataType: id) -> id;

    unsafe fn availableTypeFromArray(self, types: id) -> id;
    unsafe fn canReadItemWithDataConformingToTypes(self, types: id) -> BOOL;
    unsafe fn canReadObjectForClasses_options(self, classArray: id, options: id) -> BOOL;
    unsafe fn types(self) -> id;
    unsafe fn typesFilterableTo(_: Self, _type: id) -> id {
        msg_send![class!(NSPasteboard), typesFilterableTo: _type]
    }

    unsafe fn name(self) -> id;
    unsafe fn changeCount(self) -> NSInteger;

    unsafe fn declareTypes_owner(self, newTypes: id, newOwner: id) -> NSInteger;
    unsafe fn addTypes_owner(self, newTypes: id, newOwner: id) -> NSInteger;
    unsafe fn writeFileContents(self, filename: id) -> BOOL;
    unsafe fn writeFileWrapper(self, wrapper: id) -> BOOL;

    unsafe fn readFileContentsType_toFile(self, _type: id, filename: id) -> id;
    unsafe fn readFileWrapper(self) -> id;
}

impl NSPasteboard for id {
    unsafe fn releaseGlobally(self) {
        msg_send![self, releaseGlobally]
    }

    unsafe fn clearContents(self) -> NSInteger {
        msg_send![self, clearContents]
    }

    unsafe fn writeObjects(self, objects: id) -> BOOL {
        msg_send![self, writeObjects: objects]
    }

    unsafe fn setData_forType(self, data: id, dataType: id) -> BOOL {
        msg_send![self, setData:data forType:dataType]
    }

    unsafe fn setPropertyList_forType(self, plist: id, dataType: id) -> BOOL {
        msg_send![self, setPropertyList:plist forType:dataType]
    }

    unsafe fn setString_forType(self, string: id, dataType: id) -> BOOL {
        msg_send![self, setString:string forType:dataType]
    }

    unsafe fn readObjectsForClasses_options(self, classArray: id, options: id) -> id {
        msg_send![self, readObjectsForClasses:classArray options:options]
    }

    unsafe fn pasteboardItems(self) -> id {
        msg_send![self, pasteboardItems]
    }

    unsafe fn indexOfPasteboardItem(self, pasteboardItem: id) -> NSInteger {
        msg_send![self, indexOfPasteboardItem: pasteboardItem]
    }

    unsafe fn dataForType(self, dataType: id) -> id {
        msg_send![self, dataForType: dataType]
    }

    unsafe fn propertyListForType(self, dataType: id) -> id {
        msg_send![self, propertyListForType: dataType]
    }

    unsafe fn stringForType(self, dataType: id) -> id {
        msg_send![self, stringForType: dataType]
    }

    unsafe fn availableTypeFromArray(self, types: id) -> id {
        msg_send![self, availableTypeFromArray: types]
    }

    unsafe fn canReadItemWithDataConformingToTypes(self, types: id) -> BOOL {
        msg_send![self, canReadItemWithDataConformingToTypes: types]
    }

    unsafe fn canReadObjectForClasses_options(self, classArray: id, options: id) -> BOOL {
        msg_send![self, canReadObjectForClasses:classArray options:options]
    }

    unsafe fn types(self) -> id {
        msg_send![self, types]
    }

    unsafe fn name(self) -> id {
        msg_send![self, name]
    }

    unsafe fn changeCount(self) -> NSInteger {
        msg_send![self, changeCount]
    }

    unsafe fn declareTypes_owner(self, newTypes: id, newOwner: id) -> NSInteger {
        msg_send![self, declareTypes:newTypes owner:newOwner]
    }

    unsafe fn addTypes_owner(self, newTypes: id, newOwner: id) -> NSInteger {
        msg_send![self, addTypes:newTypes owner:newOwner]
    }

    unsafe fn writeFileContents(self, filename: id) -> BOOL {
        msg_send![self, writeFileContents: filename]
    }

    unsafe fn writeFileWrapper(self, wrapper: id) -> BOOL {
        msg_send![self, writeFileWrapper: wrapper]
    }

    unsafe fn readFileContentsType_toFile(self, _type: id, filename: id) -> id {
        msg_send![self, readFileContentsType:_type toFile:filename]
    }

    unsafe fn readFileWrapper(self) -> id {
        msg_send![self, readFileWrapper]
    }
}

pub trait NSFastEnumeration: Sized {
    unsafe fn iter(self) -> NSFastIterator;
}

impl NSFastEnumeration for id {
    unsafe fn iter(self) -> NSFastIterator {
        NSFastIterator {
            state: NSFastEnumerationState {
                state: 0,
                items_ptr: ptr::null_mut(),
                mutations_ptr: ptr::null_mut(),
                extra: [0; 5],
            },
            buffer: [nil; NS_FAST_ENUM_BUF_SIZE],
            mut_val: None,
            len: 0,
            idx: 0,
            object: self,
        }
    }
}

const NS_FAST_ENUM_BUF_SIZE: usize = 16;

#[repr(C)]
struct NSFastEnumerationState {
    pub state: libc::c_ulong,
    pub items_ptr: *mut id,
    pub mutations_ptr: *mut libc::c_ulong,
    pub extra: [libc::c_ulong; 5],
}

pub struct NSFastIterator {
    state: NSFastEnumerationState,
    buffer: [id; NS_FAST_ENUM_BUF_SIZE],
    mut_val: Option<libc::c_ulong>,
    len: usize,
    idx: usize,
    object: id,
}

impl Iterator for NSFastIterator {
    type Item = id;

    fn next(&mut self) -> Option<id> {
        if self.idx >= self.len {
            self.len = unsafe {
                msg_send![self.object, countByEnumeratingWithState:&mut self.state objects:self.buffer.as_mut_ptr() count:NS_FAST_ENUM_BUF_SIZE]
            };
            self.idx = 0;
        }

        let new_mut = unsafe { *self.state.mutations_ptr };

        if let Some(old_mut) = self.mut_val {
            assert!(
                old_mut == new_mut,
                "The collection was mutated while being enumerated"
            );
        }

        if self.idx < self.len {
            let object = unsafe { *self.state.items_ptr.offset(self.idx as isize) };
            self.mut_val = Some(new_mut);
            self.idx += 1;
            Some(object)
        } else {
            None
        }
    }
}

pub trait NSColor: Sized {
    unsafe fn clearColor(_: Self) -> id;
    unsafe fn colorWithRed_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id;
    unsafe fn colorWithSRGBRed_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id;
    unsafe fn colorWithDeviceRed_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id;
    unsafe fn colorWithDisplayP3Red_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id;
    unsafe fn colorWithCalibratedRed_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id;

    unsafe fn colorUsingColorSpace_(self, color_space: id) -> id;

    unsafe fn alphaComponent(self) -> CGFloat;
    unsafe fn whiteComponent(self) -> CGFloat;
    unsafe fn redComponent(self) -> CGFloat;
    unsafe fn greenComponent(self) -> CGFloat;
    unsafe fn blueComponent(self) -> CGFloat;
    unsafe fn cyanComponent(self) -> CGFloat;
    unsafe fn magentaComponent(self) -> CGFloat;
    unsafe fn yellowComponent(self) -> CGFloat;
    unsafe fn blackComponent(self) -> CGFloat;
    unsafe fn hueComponent(self) -> CGFloat;
    unsafe fn saturationComponent(self) -> CGFloat;
    unsafe fn brightnessComponent(self) -> CGFloat;
}

impl NSColor for id {
    unsafe fn clearColor(_: Self) -> id {
        msg_send![class!(NSColor), clearColor]
    }
    unsafe fn colorWithRed_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id {
        msg_send![class!(NSColor), colorWithRed:r green:g blue:b alpha:a]
    }
    unsafe fn colorWithSRGBRed_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id {
        msg_send![class!(NSColor), colorWithSRGBRed:r green:g blue:b alpha:a]
    }
    unsafe fn colorWithDeviceRed_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id {
        msg_send![class!(NSColor), colorWithDeviceRed:r green:g blue:b alpha:a]
    }
    unsafe fn colorWithDisplayP3Red_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id {
        msg_send![class!(NSColor), colorWithDisplayP3Red:r green:g blue:b alpha:a]
    }
    unsafe fn colorWithCalibratedRed_green_blue_alpha_(
        _: Self,
        r: CGFloat,
        g: CGFloat,
        b: CGFloat,
        a: CGFloat,
    ) -> id {
        msg_send![class!(NSColor), colorWithCalibratedRed:r green:g blue:b alpha:a]
    }

    unsafe fn colorUsingColorSpace_(self, color_space: id) -> id {
        msg_send![self, colorUsingColorSpace: color_space]
    }

    unsafe fn alphaComponent(self) -> CGFloat {
        msg_send![self, alphaComponent]
    }
    unsafe fn whiteComponent(self) -> CGFloat {
        msg_send![self, whiteComponent]
    }
    unsafe fn redComponent(self) -> CGFloat {
        msg_send![self, redComponent]
    }
    unsafe fn greenComponent(self) -> CGFloat {
        msg_send![self, greenComponent]
    }
    unsafe fn blueComponent(self) -> CGFloat {
        msg_send![self, blueComponent]
    }
    unsafe fn cyanComponent(self) -> CGFloat {
        msg_send![self, cyanComponent]
    }
    unsafe fn magentaComponent(self) -> CGFloat {
        msg_send![self, magentaComponent]
    }
    unsafe fn yellowComponent(self) -> CGFloat {
        msg_send![self, yellowComponent]
    }
    unsafe fn blackComponent(self) -> CGFloat {
        msg_send![self, blackComponent]
    }
    unsafe fn hueComponent(self) -> CGFloat {
        msg_send![self, hueComponent]
    }
    unsafe fn saturationComponent(self) -> CGFloat {
        msg_send![self, saturationComponent]
    }
    unsafe fn brightnessComponent(self) -> CGFloat {
        msg_send![self, brightnessComponent]
    }
}

#[repr(u64)]
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum NSWindowButton {
    NSWindowCloseButton = 0,
    NSWindowMiniaturizeButton = 1,
    NSWindowZoomButton = 2,
    NSWindowToolbarButton = 3,
    NSWindowDocumentIconButton = 4,
    NSWindowDocumentVersionsButton = 6,
    NSWindowFullScreenButton = 7,
}

pub trait NSArray: Sized {
    unsafe fn array(_: Self) -> id {
        msg_send![class!(NSArray), array]
    }

    unsafe fn arrayWithObjects(_: Self, objects: &[id]) -> id {
        msg_send![class!(NSArray), arrayWithObjects:objects.as_ptr()
                                    count:objects.len()]
    }

    unsafe fn arrayWithObject(_: Self, object: id) -> id {
        msg_send![class!(NSArray), arrayWithObject: object]
    }

    unsafe fn init(self) -> id;

    unsafe fn count(self) -> NSUInteger;

    unsafe fn arrayByAddingObjectFromArray(self, object: id) -> id;
    unsafe fn arrayByAddingObjectsFromArray(self, objects: id) -> id;
    unsafe fn objectAtIndex(self, index: NSUInteger) -> id;
}

impl NSArray for id {
    unsafe fn init(self) -> id {
        msg_send![self, init]
    }

    unsafe fn count(self) -> NSUInteger {
        msg_send![self, count]
    }

    unsafe fn arrayByAddingObjectFromArray(self, object: id) -> id {
        msg_send![self, arrayByAddingObjectFromArray: object]
    }

    unsafe fn arrayByAddingObjectsFromArray(self, objects: id) -> id {
        msg_send![self, arrayByAddingObjectsFromArray: objects]
    }

    unsafe fn objectAtIndex(self, index: NSUInteger) -> id {
        msg_send![self, objectAtIndex: index]
    }
}

pub trait NSImage: Sized {
    unsafe fn alloc(_: Self) -> id {
        msg_send![class!(NSImage), alloc]
    }

    unsafe fn initByReferencingFile_(self, file_name: id /* (NSString *) */) -> id;
    /*
    unsafe fn initWithContentsOfFile_(self, file_name: id /* (NSString *) */) -> id;
    unsafe fn initWithData_(self, data: id /* (NSData *) */) -> id;
    unsafe fn initWithDataIgnoringOrientation_(self, data: id /* (NSData *) */) -> id;
    unsafe fn initWithPasteboard_(self, pasteboard: id /* (NSPasteboard *) */) -> id;
    unsafe fn initWithSize_flipped_drawingHandler_(self, size: NSSize,
                                                   drawingHandlerShouldBeCalledWithFlippedContext: BOOL,
                                                   drawingHandler: *mut Block<(NSRect,), BOOL>);
    unsafe fn initWithSize_(self, aSize: NSSize) -> id;

    unsafe fn imageNamed_(_: Self, name: id /* (NSString *) */) -> id {
        msg_send![class!(NSImage), imageNamed:name]
    }

    unsafe fn name(self) -> id /* (NSString *) */;
    unsafe fn setName_(self, name: id /* (NSString *) */) -> BOOL;

    unsafe fn size(self) -> NSSize;
    unsafe fn template(self) -> BOOL;

    unsafe fn canInitWithPasteboard_(self, pasteboard: id /* (NSPasteboard *) */) -> BOOL;
    unsafe fn imageTypes(self) -> id /* (NSArray<NSString *> ) */;
    unsafe fn imageUnfilteredTypes(self) -> id /* (NSArray<NSString *> ) */;

    unsafe fn addRepresentation_(self, imageRep: id /* (NSImageRep *) */);
    unsafe fn addRepresentations_(self, imageReps: id /* (NSArray<NSImageRep *> *) */);
    unsafe fn representations(self) -> id /* (NSArray<NSImageRep *> *) */;
    unsafe fn removeRepresentation_(self, imageRep: id /* (NSImageRep *) */);
    unsafe fn bestRepresentationForRect_context_hints_(self, rect: NSRect,
                                                       referenceContext: id /* (NSGraphicsContext *) */,
                                                       hints: id /* (NSDictionary<NSString *, id> *) */)
                                                       -> id /* (NSImageRep *) */;
    unsafe fn prefersColorMatch(self) -> BOOL;
    unsafe fn usesEPSOnResolutionMismatch(self) -> BOOL;
    unsafe fn matchesOnMultipleResolution(self) -> BOOL;

    unsafe fn drawInRect_(self, rect: NSRect);
    unsafe fn drawAtPoint_fromRect_operation_fraction_(self, point: NSPoint, srcRect: NSRect,
                                                       op: NSCompositingOperation, delta: CGFloat);
    unsafe fn drawInRect_fromRect_operation_fraction_(self, dstRect: NSRect, srcRect: NSRect,
                                                      op: NSCompositingOperation, delta: CGFloat);
    unsafe fn drawInRect_fromRect_operation_fraction_respectFlipped_hints_(self, dstSpacePortionRect: NSRect,
        srcSpacePortionRect: NSRect, op: NSCompositingOperation, delta: CGFloat, respectContextIsFlipped: BOOL,
        hints: id /* (NSDictionary<NSString *, id> *) */);
    unsafe fn drawRepresentation_inRect_(self, imageRep: id /* (NSImageRep *) */, dstRect: NSRect);

    unsafe fn isValid(self) -> BOOL;
    unsafe fn backgroundColor(self) -> id /* (NSColor *) */;

    unsafe fn lockFocus(self);
    unsafe fn lockFocusFlipped_(self, flipped: BOOL);
    unsafe fn unlockFocus(self);

    unsafe fn alignmentRect(self) -> NSRect;

    unsafe fn cacheMode(self) -> NSImageCacheMode;
    unsafe fn recache(self);

    unsafe fn delegate(self) -> id /* (id<NSImageDelegate *> *) */;

    unsafe fn TIFFRepresentation(self) -> id /* (NSData *) */;
    unsafe fn TIFFRepresentationUsingCompression_factor_(self, comp: NSTIFFCompression, aFloat: f32)
                                                         -> id /* (NSData *) */;

    unsafe fn cancelIncrementalLoad(self);

    unsafe fn hitTestRect_withImageDestinationRect_context_hints_flipped_(self, testRectDestSpace: NSRect,
        imageRectDestSpace: NSRect, referenceContext: id /* (NSGraphicsContext *) */,
        hints: id /* (NSDictionary<NSString *, id> *) */, flipped: BOOL) -> BOOL;

    unsafe fn accessibilityDescription(self) -> id /* (NSString *) */;

    unsafe fn layerContentsForContentsScale_(self, layerContentsScale: CGFloat) -> id /* (id) */;
    unsafe fn recommendedLayerContentsScale_(self, preferredContentsScale: CGFloat) -> CGFloat;

    unsafe fn matchesOnlyOnBestFittingAxis(self) -> BOOL;
    */
}

impl NSImage for id {
    unsafe fn initByReferencingFile_(self, file_name: id /* (NSString *) */) -> id {
        msg_send![self, initByReferencingFile: file_name]
    }

    /*
    unsafe fn initWithContentsOfFile_(self, file_name: id /* (NSString *) */) -> id {
        msg_send![self, initWithContentsOfFile: file_name]
    }

    unsafe fn initWithData_(self, data: id /* (NSData *) */) -> id {
        msg_send![self, initWithData: data]
    }

    unsafe fn initWithDataIgnoringOrientation_(self, data: id /* (NSData *) */) -> id {
        msg_send![self, initWithDataIgnoringOrientation: data]
    }

    unsafe fn initWithPasteboard_(self, pasteboard: id /* (NSPasteboard *) */) -> id {
        msg_send![self, initWithPasteboard: pasteboard]
    }

    unsafe fn initWithSize_flipped_drawingHandler_(
        self,
        size: NSSize,
        drawingHandlerShouldBeCalledWithFlippedContext: BOOL,
        drawingHandler: *mut Block<(NSRect,), BOOL>,
    ) {
        msg_send![self, initWithSize:size
                             flipped:drawingHandlerShouldBeCalledWithFlippedContext
                      drawingHandler:drawingHandler]
    }

    unsafe fn initWithSize_(self, aSize: NSSize) -> id {
        msg_send![self, initWithSize: aSize]
    }

    unsafe fn name(self) -> id /* (NSString *) */ {
        msg_send![self, name]
    }

    unsafe fn setName_(self, name: id /* (NSString *) */) -> BOOL {
        msg_send![self, setName: name]
    }

    unsafe fn size(self) -> NSSize {
        msg_send![self, size]
    }

    unsafe fn template(self) -> BOOL {
        msg_send![self, template]
    }

    unsafe fn canInitWithPasteboard_(self, pasteboard: id /* (NSPasteboard *) */) -> BOOL {
        msg_send![self, canInitWithPasteboard: pasteboard]
    }

    unsafe fn imageTypes(self) -> id /* (NSArray<NSString *> ) */ {
        msg_send![self, imageTypes]
    }

    unsafe fn imageUnfilteredTypes(self) -> id /* (NSArray<NSString *> ) */ {
        msg_send![self, imageUnfilteredTypes]
    }

    unsafe fn addRepresentation_(self, imageRep: id /* (NSImageRep *) */) {
        msg_send![self, addRepresentation: imageRep]
    }

    unsafe fn addRepresentations_(self, imageReps: id /* (NSArray<NSImageRep *> *) */) {
        msg_send![self, addRepresentations: imageReps]
    }

    unsafe fn representations(self) -> id /* (NSArray<NSImageRep *> *) */ {
        msg_send![self, representations]
    }

    unsafe fn removeRepresentation_(self, imageRep: id /* (NSImageRep *) */) {
        msg_send![self, removeRepresentation: imageRep]
    }

    unsafe fn bestRepresentationForRect_context_hints_(
        self,
        rect: NSRect,
        referenceContext: id, /* (NSGraphicsContext *) */
        hints: id,            /* (NSDictionary<NSString *, id> *) */
    ) -> id /* (NSImageRep *) */ {
        msg_send![self, bestRepresentationForRect:rect context:referenceContext hints:hints]
    }

    unsafe fn prefersColorMatch(self) -> BOOL {
        msg_send![self, prefersColorMatch]
    }

    unsafe fn usesEPSOnResolutionMismatch(self) -> BOOL {
        msg_send![self, usesEPSOnResolutionMismatch]
    }

    unsafe fn matchesOnMultipleResolution(self) -> BOOL {
        msg_send![self, matchesOnMultipleResolution]
    }

    unsafe fn drawInRect_(self, rect: NSRect) {
        msg_send![self, drawInRect: rect]
    }

    unsafe fn drawAtPoint_fromRect_operation_fraction_(
        self,
        point: NSPoint,
        srcRect: NSRect,
        op: NSCompositingOperation,
        delta: CGFloat,
    ) {
        msg_send![self, drawAtPoint:point fromRect:srcRect operation:op fraction:delta]
    }

    unsafe fn drawInRect_fromRect_operation_fraction_(
        self,
        dstRect: NSRect,
        srcRect: NSRect,
        op: NSCompositingOperation,
        delta: CGFloat,
    ) {
        msg_send![self, drawInRect:dstRect fromRect:srcRect operation:op fraction:delta]
    }

    unsafe fn drawInRect_fromRect_operation_fraction_respectFlipped_hints_(
        self,
        dstSpacePortionRect: NSRect,
        srcSpacePortionRect: NSRect,
        op: NSCompositingOperation,
        delta: CGFloat,
        respectContextIsFlipped: BOOL,
        hints: id, /* (NSDictionary<NSString *, id> *) */
    ) {
        msg_send![self, drawInRect:dstSpacePortionRect
                          fromRect:srcSpacePortionRect
                         operation:op
                          fraction:delta
                    respectFlipped:respectContextIsFlipped
                             hints:hints]
    }

    unsafe fn drawRepresentation_inRect_(
        self,
        imageRep: id, /* (NSImageRep *) */
        dstRect: NSRect,
    ) {
        msg_send![self, drawRepresentation:imageRep inRect:dstRect]
    }

    unsafe fn isValid(self) -> BOOL {
        msg_send![self, isValid]
    }

    unsafe fn backgroundColor(self) -> id /* (NSColor *) */ {
        msg_send![self, backgroundColor]
    }

    unsafe fn lockFocus(self) {
        msg_send![self, lockFocus]
    }

    unsafe fn lockFocusFlipped_(self, flipped: BOOL) {
        msg_send![self, lockFocusFlipped: flipped]
    }

    unsafe fn unlockFocus(self) {
        msg_send![self, unlockFocus]
    }

    unsafe fn alignmentRect(self) -> NSRect {
        msg_send![self, alignmentRect]
    }

    unsafe fn cacheMode(self) -> NSImageCacheMode {
        msg_send![self, cacheMode]
    }

    unsafe fn recache(self) {
        msg_send![self, recache]
    }

    unsafe fn delegate(self) -> id /* (id<NSImageDelegate *> *) */ {
        msg_send![self, delegate]
    }

    unsafe fn TIFFRepresentation(self) -> id /* (NSData *) */ {
        msg_send![self, TIFFRepresentation]
    }

    unsafe fn TIFFRepresentationUsingCompression_factor_(
        self,
        comp: NSTIFFCompression,
        aFloat: f32,
    ) -> id /* (NSData *) */ {
        msg_send![self, TIFFRepresentationUsingCompression:comp factor:aFloat]
    }

    unsafe fn cancelIncrementalLoad(self) {
        msg_send![self, cancelIncrementalLoad]
    }

    unsafe fn hitTestRect_withImageDestinationRect_context_hints_flipped_(
        self,
        testRectDestSpace: NSRect,
        imageRectDestSpace: NSRect,
        referenceContext: id, /* (NSGraphicsContext *) */
        hints: id,            /* (NSDictionary<NSString *, id> *) */
        flipped: BOOL,
    ) -> BOOL {
        msg_send![self, hitTestRect:testRectDestSpace
           withImageDestinationRect:imageRectDestSpace
                            context:referenceContext
                              hints:hints
                            flipped:flipped]
    }

    unsafe fn accessibilityDescription(self) -> id /* (NSString *) */ {
        msg_send![self, accessibilityDescription]
    }

    unsafe fn layerContentsForContentsScale_(self, layerContentsScale: CGFloat) -> id /* (id) */ {
        msg_send![self, layerContentsForContentsScale: layerContentsScale]
    }

    unsafe fn recommendedLayerContentsScale_(self, preferredContentsScale: CGFloat) -> CGFloat {
        msg_send![self, recommendedLayerContentsScale: preferredContentsScale]
    }

    unsafe fn matchesOnlyOnBestFittingAxis(self) -> BOOL {
        msg_send![self, matchesOnlyOnBestFittingAxis]
    }
    */
}

pub trait NSDictionary: Sized {
    // unsafe fn dictionary(_: Self) -> id {
    //     msg_send![class!(NSDictionary), dictionary]
    // }

    unsafe fn dictionaryWithContentsOfFile_(_: Self, path: id) -> id {
        msg_send![class!(NSDictionary), dictionaryWithContentsOfFile: path]
    }

    /*
    unsafe fn dictionaryWithContentsOfURL_(_: Self, aURL: id) -> id {
        msg_send![class!(NSDictionary), dictionaryWithContentsOfURL: aURL]
    }

    unsafe fn dictionaryWithDictionary_(_: Self, otherDictionary: id) -> id {
        msg_send![
            class!(NSDictionary),
            dictionaryWithDictionary: otherDictionary
        ]
    }

    unsafe fn dictionaryWithObject_forKey_(_: Self, anObject: id, aKey: id) -> id {
        msg_send![class!(NSDictionary), dictionaryWithObject:anObject forKey:aKey]
    }

    unsafe fn dictionaryWithObjects_forKeys_(_: Self, objects: id, keys: id) -> id {
        msg_send![class!(NSDictionary), dictionaryWithObjects:objects forKeys:keys]
    }

    unsafe fn dictionaryWithObjects_forKeys_count_(
        _: Self,
        objects: *const id,
        keys: *const id,
        count: NSUInteger,
    ) -> id {
        msg_send![class!(NSDictionary), dictionaryWithObjects:objects forKeys:keys count:count]
    }

    unsafe fn dictionaryWithObjectsAndKeys_(_: Self, firstObject: id) -> id {
        msg_send![
            class!(NSDictionary),
            dictionaryWithObjectsAndKeys: firstObject
        ]
    }

    unsafe fn init(self) -> id;
    unsafe fn initWithContentsOfFile_(self, path: id) -> id;
    unsafe fn initWithContentsOfURL_(self, aURL: id) -> id;
    unsafe fn initWithDictionary_(self, otherDicitonary: id) -> id;
    unsafe fn initWithDictionary_copyItems_(self, otherDicitonary: id, flag: BOOL) -> id;
    unsafe fn initWithObjects_forKeys_(self, objects: id, keys: id) -> id;
    unsafe fn initWithObjects_forKeys_count_(self, objects: id, keys: id, count: NSUInteger) -> id;
    unsafe fn initWithObjectsAndKeys_(self, firstObject: id) -> id;

    unsafe fn sharedKeySetForKeys_(_: Self, keys: id) -> id {
        msg_send![class!(NSDictionary), sharedKeySetForKeys: keys]
    }

    unsafe fn count(self) -> NSUInteger;

    unsafe fn isEqualToDictionary_(self, otherDictionary: id) -> BOOL;

    unsafe fn allKeys(self) -> id;
    unsafe fn allKeysForObject_(self, anObject: id) -> id;
    unsafe fn allValues(self) -> id;
    unsafe fn objectForKey_(self, aKey: id) -> id;
    unsafe fn objectForKeyedSubscript_(self, key: id) -> id;
    unsafe fn objectsForKeys_notFoundMarker_(self, keys: id, anObject: id) -> id;
    */
    unsafe fn valueForKey_(self, key: id) -> id;

    /*
        unsafe fn keyEnumerator(self) -> id;
        unsafe fn objectEnumerator(self) -> id;
        unsafe fn enumerateKeysAndObjectsUsingBlock_(self, block: *mut Block<(id, id, *mut BOOL), ()>);
        unsafe fn enumerateKeysAndObjectsWithOptions_usingBlock_(
            self,
            opts: NSEnumerationOptions,
            block: *mut Block<(id, id, *mut BOOL), ()>,
        );

        unsafe fn keysSortedByValueUsingSelector_(self, comparator: SEL) -> id;
        unsafe fn keysSortedByValueUsingComparator_(self, cmptr: NSComparator) -> id;
        unsafe fn keysSortedByValueWithOptions_usingComparator_(
            self,
            opts: NSEnumerationOptions,
            cmptr: NSComparator,
        ) -> id;

        unsafe fn keysOfEntriesPassingTest_(
            self,
            predicate: *mut Block<(id, id, *mut BOOL), BOOL>,
        ) -> id;
        unsafe fn keysOfEntriesWithOptions_PassingTest_(
            self,
            opts: NSEnumerationOptions,
            predicate: *mut Block<(id, id, *mut BOOL), BOOL>,
        ) -> id;

        unsafe fn writeToFile_atomically_(self, path: id, flag: BOOL) -> BOOL;
        unsafe fn writeToURL_atomically_(self, aURL: id, flag: BOOL) -> BOOL;

        unsafe fn fileCreationDate(self) -> id;
        unsafe fn fileExtensionHidden(self) -> BOOL;
        unsafe fn fileGroupOwnerAccountID(self) -> id;
        unsafe fn fileGroupOwnerAccountName(self) -> id;
        unsafe fn fileIsAppendOnly(self) -> BOOL;
        unsafe fn fileIsImmutable(self) -> BOOL;
        unsafe fn fileModificationDate(self) -> id;
        unsafe fn fileOwnerAccountID(self) -> id;
        unsafe fn fileOwnerAccountName(self) -> id;
        unsafe fn filePosixPermissions(self) -> NSUInteger;
        unsafe fn fileSize(self) -> libc::c_ulonglong;
        unsafe fn fileSystemFileNumber(self) -> NSUInteger;
        unsafe fn fileSystemNumber(self) -> NSInteger;
        unsafe fn fileType(self) -> id;

        unsafe fn description(self) -> id;
        unsafe fn descriptionInStringsFileFormat(self) -> id;
        unsafe fn descriptionWithLocale_(self, locale: id) -> id;
        unsafe fn descriptionWithLocale_indent_(self, locale: id, indent: NSUInteger) -> id;
    }
    */
}

impl NSDictionary for id {
    /*
     unsafe fn init(self) -> id {
         msg_send![self, init]
     }

    unsafe fn initWithContentsOfFile_(self, path: id) -> id {
        msg_send![self, initWithContentsOfFile: path]
    }


    unsafe fn initWithContentsOfURL_(self, aURL: id) -> id {
        msg_send![self, initWithContentsOfURL: aURL]
    }

    unsafe fn initWithDictionary_(self, otherDictionary: id) -> id {
        msg_send![self, initWithDictionary: otherDictionary]
    }

    unsafe fn initWithDictionary_copyItems_(self, otherDictionary: id, flag: BOOL) -> id {
        msg_send![self, initWithDictionary:otherDictionary copyItems:flag]
    }

    unsafe fn initWithObjects_forKeys_(self, objects: id, keys: id) -> id {
        msg_send![self, initWithObjects:objects forKeys:keys]
    }

    unsafe fn initWithObjects_forKeys_count_(self, objects: id, keys: id, count: NSUInteger) -> id {
        msg_send![self, initWithObjects:objects forKeys:keys count:count]
    }

    unsafe fn initWithObjectsAndKeys_(self, firstObject: id) -> id {
        msg_send![self, initWithObjectsAndKeys: firstObject]
    }

    unsafe fn count(self) -> NSUInteger {
        msg_send![self, count]
    }

    unsafe fn isEqualToDictionary_(self, otherDictionary: id) -> BOOL {
        msg_send![self, isEqualToDictionary: otherDictionary]
    }

    unsafe fn allKeys(self) -> id {
        msg_send![self, allKeys]
    }

    unsafe fn allKeysForObject_(self, anObject: id) -> id {
        msg_send![self, allKeysForObject: anObject]
    }

    unsafe fn allValues(self) -> id {
        msg_send![self, allValues]
    }

    unsafe fn objectForKey_(self, aKey: id) -> id {
        msg_send![self, objectForKey: aKey]
    }

    unsafe fn objectForKeyedSubscript_(self, key: id) -> id {
        msg_send![self, objectForKeyedSubscript: key]
    }

    unsafe fn objectsForKeys_notFoundMarker_(self, keys: id, anObject: id) -> id {
        msg_send![self, objectsForKeys:keys notFoundMarker:anObject]
    }
    */

    unsafe fn valueForKey_(self, key: id) -> id {
        msg_send![self, valueForKey: key]
    }

    /*

    unsafe fn keyEnumerator(self) -> id {
        msg_send![self, keyEnumerator]
    }

    unsafe fn objectEnumerator(self) -> id {
        msg_send![self, objectEnumerator]
    }

    unsafe fn enumerateKeysAndObjectsUsingBlock_(self, block: *mut Block<(id, id, *mut BOOL), ()>) {
        msg_send![self, enumerateKeysAndObjectsUsingBlock: block]
    }

    unsafe fn enumerateKeysAndObjectsWithOptions_usingBlock_(
        self,
        opts: NSEnumerationOptions,
        block: *mut Block<(id, id, *mut BOOL), ()>,
    ) {
        msg_send![self, enumerateKeysAndObjectsWithOptions:opts usingBlock:block]
    }

    unsafe fn keysSortedByValueUsingSelector_(self, comparator: SEL) -> id {
        msg_send![self, keysSortedByValueUsingSelector: comparator]
    }

    unsafe fn keysSortedByValueUsingComparator_(self, cmptr: NSComparator) -> id {
        msg_send![self, keysSortedByValueUsingComparator: cmptr]
    }

    unsafe fn keysSortedByValueWithOptions_usingComparator_(
        self,
        opts: NSEnumerationOptions,
        cmptr: NSComparator,
    ) -> id {
        let rv: id = msg_send![self, keysSortedByValueWithOptions:opts usingComparator:cmptr];
        rv
    }

    unsafe fn keysOfEntriesPassingTest_(
        self,
        predicate: *mut Block<(id, id, *mut BOOL), BOOL>,
    ) -> id {
        msg_send![self, keysOfEntriesPassingTest: predicate]
    }

    unsafe fn keysOfEntriesWithOptions_PassingTest_(
        self,
        opts: NSEnumerationOptions,
        predicate: *mut Block<(id, id, *mut BOOL), BOOL>,
    ) -> id {
        msg_send![self, keysOfEntriesWithOptions:opts PassingTest:predicate]
    }

    unsafe fn writeToFile_atomically_(self, path: id, flag: BOOL) -> BOOL {
        msg_send![self, writeToFile:path atomically:flag]
    }

    unsafe fn writeToURL_atomically_(self, aURL: id, flag: BOOL) -> BOOL {
        msg_send![self, writeToURL:aURL atomically:flag]
    }

    unsafe fn fileCreationDate(self) -> id {
        msg_send![self, fileCreationDate]
    }

    unsafe fn fileExtensionHidden(self) -> BOOL {
        msg_send![self, fileExtensionHidden]
    }

    unsafe fn fileGroupOwnerAccountID(self) -> id {
        msg_send![self, fileGroupOwnerAccountID]
    }

    unsafe fn fileGroupOwnerAccountName(self) -> id {
        msg_send![self, fileGroupOwnerAccountName]
    }

    unsafe fn fileIsAppendOnly(self) -> BOOL {
        msg_send![self, fileIsAppendOnly]
    }

    unsafe fn fileIsImmutable(self) -> BOOL {
        msg_send![self, fileIsImmutable]
    }

    unsafe fn fileModificationDate(self) -> id {
        msg_send![self, fileModificationDate]
    }

    unsafe fn fileOwnerAccountID(self) -> id {
        msg_send![self, fileOwnerAccountID]
    }

    unsafe fn fileOwnerAccountName(self) -> id {
        msg_send![self, fileOwnerAccountName]
    }

    unsafe fn filePosixPermissions(self) -> NSUInteger {
        msg_send![self, filePosixPermissions]
    }

    unsafe fn fileSize(self) -> libc::c_ulonglong {
        msg_send![self, fileSize]
    }

    unsafe fn fileSystemFileNumber(self) -> NSUInteger {
        msg_send![self, fileSystemFileNumber]
    }

    unsafe fn fileSystemNumber(self) -> NSInteger {
        msg_send![self, fileSystemNumber]
    }

    unsafe fn fileType(self) -> id {
        msg_send![self, fileType]
    }

    unsafe fn description(self) -> id {
        msg_send![self, description]
    }

    unsafe fn descriptionInStringsFileFormat(self) -> id {
        msg_send![self, descriptionInStringsFileFormat]
    }

    unsafe fn descriptionWithLocale_(self, locale: id) -> id {
        msg_send![self, descriptionWithLocale: locale]
    }

    unsafe fn descriptionWithLocale_indent_(self, locale: id, indent: NSUInteger) -> id {
        msg_send![self, descriptionWithLocale:locale indent:indent]
    }
    */
}

bitflags! {
    pub struct NSEnumerationOptions: libc::c_ulonglong {
        const NSEnumerationConcurrent = 1 << 0;
        const NSEnumerationReverse = 1 << 1;
    }
}

bitflags! {
    pub struct NSEventPhase: NSUInteger {
       const NSEventPhaseNone        = 0;
       const NSEventPhaseBegan       = 0x1 << 0;
       const NSEventPhaseStationary  = 0x1 << 1;
       const NSEventPhaseChanged     = 0x1 << 2;
       const NSEventPhaseEnded       = 0x1 << 3;
       const NSEventPhaseCancelled   = 0x1 << 4;
       const NSEventPhaseMayBegin    = 0x1 << 5;
    }
}

#[link(name = "AppKit", kind = "framework")]
extern "C" {
    pub static NSFilenamesPboardType: id;

    pub static NSAppKitVersionNumber: f64;
}

pub const NSAppKitVersionNumber10_12: f64 = 1504.0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGPoint {
    pub x: CGFloat,
    pub y: CGFloat,
}

impl CGPoint {
    // #[inline]
    // pub fn new(x: CGFloat, y: CGFloat) -> CGPoint {
    //     CGPoint { x, y }
    // }

    // #[inline]
    // pub fn apply_transform(&self, t: &CGAffineTransform) -> CGPoint {
    //     unsafe { ffi::CGPointApplyAffineTransform(*self, *t) }
    // }
}
