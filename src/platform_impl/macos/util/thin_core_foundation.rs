#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]
#![allow(unused)]

use std::os::raw::{c_int, c_uint, c_void};

#[repr(C)]
pub struct __CFString(c_void);

pub type CFStringRef = *const __CFString;

pub type Boolean = u8;
pub type mach_port_t = c_uint;
pub type CFAllocatorRef = *const c_void;
pub type CFNullRef = *const c_void;
pub type CFTypeRef = *const c_void;
pub type OSStatus = i32;
pub type SInt32 = c_int;
pub type CFTypeID = usize;
pub type CFOptionFlags = usize;
pub type CFHashCode = usize;
pub type CFIndex = isize;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFRange {
    pub location: CFIndex,
    pub length: CFIndex,
}

// for back-compat
impl CFRange {
    pub fn init(location: CFIndex, length: CFIndex) -> CFRange {
        CFRange { location, length }
    }
}

extern "C" {
    /*
     * CFBase.h
     */

    /* CFAllocator Reference */

    pub static kCFAllocatorDefault: CFAllocatorRef;
    pub static kCFAllocatorSystemDefault: CFAllocatorRef;
    pub static kCFAllocatorMalloc: CFAllocatorRef;
    pub static kCFAllocatorMallocZone: CFAllocatorRef;
    pub static kCFAllocatorNull: CFAllocatorRef;
    pub static kCFAllocatorUseContext: CFAllocatorRef;

    pub fn CFAllocatorCreate(
        allocator: CFAllocatorRef,
        context: *mut CFAllocatorContext,
    ) -> CFAllocatorRef;
    pub fn CFAllocatorAllocate(
        allocator: CFAllocatorRef,
        size: CFIndex,
        hint: CFOptionFlags,
    ) -> *mut c_void;
    pub fn CFAllocatorDeallocate(allocator: CFAllocatorRef, ptr: *mut c_void);
    pub fn CFAllocatorGetPreferredSizeForSize(
        allocator: CFAllocatorRef,
        size: CFIndex,
        hint: CFOptionFlags,
    ) -> CFIndex;
    pub fn CFAllocatorReallocate(
        allocator: CFAllocatorRef,
        ptr: *mut c_void,
        newsize: CFIndex,
        hint: CFOptionFlags,
    ) -> *mut c_void;
    pub fn CFAllocatorGetDefault() -> CFAllocatorRef;
    pub fn CFAllocatorSetDefault(allocator: CFAllocatorRef);
    pub fn CFAllocatorGetContext(allocator: CFAllocatorRef, context: *mut CFAllocatorContext);
    pub fn CFAllocatorGetTypeID() -> CFTypeID;

    /* CFNull Reference */

    pub static kCFNull: CFNullRef;

    /* CFType Reference */

    //fn CFCopyTypeIDDescription
    //fn CFGetAllocator
    pub fn CFCopyDescription(cf: CFTypeRef) -> CFStringRef;
    pub fn CFEqual(cf1: CFTypeRef, cf2: CFTypeRef) -> Boolean;
    pub fn CFGetRetainCount(cf: CFTypeRef) -> CFIndex;
    pub fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;
    pub fn CFHash(cf: CFTypeRef) -> CFHashCode;
    //fn CFMakeCollectable
    pub fn CFRelease(cf: CFTypeRef);
    pub fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    pub fn CFShow(obj: CFTypeRef);

    /* Base Utilities Reference */
    // N.B. Some things missing here.
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct CFAllocatorContext {
    pub version: CFIndex,
    pub info: *mut c_void,
    pub retain: Option<CFAllocatorRetainCallBack>,
    pub release: Option<CFAllocatorReleaseCallBack>,
    pub copyDescription: Option<CFAllocatorCopyDescriptionCallBack>,
    pub allocate: Option<CFAllocatorAllocateCallBack>,
    pub reallocate: Option<CFAllocatorReallocateCallBack>,
    pub deallocate: Option<CFAllocatorDeallocateCallBack>,
    pub preferredSize: Option<CFAllocatorPreferredSizeCallBack>,
}

pub type CFAllocatorRetainCallBack = extern "C" fn(info: *mut c_void) -> *mut c_void;
pub type CFAllocatorReleaseCallBack = extern "C" fn(info: *mut c_void);
pub type CFAllocatorCopyDescriptionCallBack = extern "C" fn(info: *mut c_void) -> CFStringRef;
pub type CFAllocatorAllocateCallBack =
    extern "C" fn(allocSize: CFIndex, hint: CFOptionFlags, info: *mut c_void) -> *mut c_void;
pub type CFAllocatorReallocateCallBack = extern "C" fn(
    ptr: *mut c_void,
    newsize: CFIndex,
    hint: CFOptionFlags,
    info: *mut c_void,
) -> *mut c_void;
pub type CFAllocatorDeallocateCallBack = extern "C" fn(ptr: *mut c_void, info: *mut c_void);
pub type CFAllocatorPreferredSizeCallBack =
    extern "C" fn(size: CFIndex, hint: CFOptionFlags, info: *mut c_void) -> CFIndex;

pub mod array {
    // Copyright 2013-2015 The Servo Project Developers. See the COPYRIGHT
    // file at the top-level directory of this distribution.
    //
    // Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
    // http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
    // <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
    // option. This file may not be copied, modified, or distributed
    // except according to those terms.

    use std::os::raw::c_void;

    use super::CFStringRef;
    use super::{Boolean, CFAllocatorRef, CFIndex, CFRange, CFTypeID};

    pub type CFArrayRetainCallBack =
        extern "C" fn(allocator: CFAllocatorRef, value: *const c_void) -> *const c_void;
    pub type CFArrayReleaseCallBack =
        extern "C" fn(allocator: CFAllocatorRef, value: *const c_void);
    pub type CFArrayCopyDescriptionCallBack = extern "C" fn(value: *const c_void) -> CFStringRef;
    pub type CFArrayEqualCallBack =
        extern "C" fn(value1: *const c_void, value2: *const c_void) -> Boolean;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CFArrayCallBacks {
        pub version: CFIndex,
        pub retain: CFArrayRetainCallBack,
        pub release: CFArrayReleaseCallBack,
        pub copyDescription: CFArrayCopyDescriptionCallBack,
        pub equal: CFArrayEqualCallBack,
    }

    #[repr(C)]
    pub struct __CFArray(c_void);

    pub type CFArrayRef = *const __CFArray;

    extern "C" {
        /*
         * CFArray.h
         */
        pub static kCFTypeArrayCallBacks: CFArrayCallBacks;

        pub fn CFArrayCreate(
            allocator: CFAllocatorRef,
            values: *const *const c_void,
            numValues: CFIndex,
            callBacks: *const CFArrayCallBacks,
        ) -> CFArrayRef;
        pub fn CFArrayCreateCopy(allocator: CFAllocatorRef, theArray: CFArrayRef) -> CFArrayRef;

        // CFArrayBSearchValues
        // CFArrayContainsValue
        pub fn CFArrayGetCount(theArray: CFArrayRef) -> CFIndex;
        // CFArrayGetCountOfValue
        // CFArrayGetFirstIndexOfValue
        // CFArrayGetLastIndexOfValue
        pub fn CFArrayGetValues(theArray: CFArrayRef, range: CFRange, values: *mut *const c_void);
        pub fn CFArrayGetValueAtIndex(theArray: CFArrayRef, idx: CFIndex) -> *const c_void;
        // CFArrayApplyFunction
        pub fn CFArrayGetTypeID() -> CFTypeID;
    }
}

pub mod dictionary {
    // Copyright 2013-2015 The Servo Project Developers. See the COPYRIGHT
    // file at the top-level directory of this distribution.
    //
    // Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
    // http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
    // <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
    // option. This file may not be copied, modified, or distributed
    // except according to those terms.

    use std::os::raw::c_void;

    use super::CFStringRef;
    use super::{Boolean, CFAllocatorRef, CFHashCode, CFIndex, CFTypeID};

    pub type CFDictionaryApplierFunction =
        extern "C" fn(key: *const c_void, value: *const c_void, context: *mut c_void);

    pub type CFDictionaryRetainCallBack =
        extern "C" fn(allocator: CFAllocatorRef, value: *const c_void) -> *const c_void;
    pub type CFDictionaryReleaseCallBack =
        extern "C" fn(allocator: CFAllocatorRef, value: *const c_void);
    pub type CFDictionaryCopyDescriptionCallBack =
        extern "C" fn(value: *const c_void) -> CFStringRef;
    pub type CFDictionaryEqualCallBack =
        extern "C" fn(value1: *const c_void, value2: *const c_void) -> Boolean;
    pub type CFDictionaryHashCallBack = extern "C" fn(value: *const c_void) -> CFHashCode;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CFDictionaryKeyCallBacks {
        pub version: CFIndex,
        pub retain: CFDictionaryRetainCallBack,
        pub release: CFDictionaryReleaseCallBack,
        pub copyDescription: CFDictionaryCopyDescriptionCallBack,
        pub equal: CFDictionaryEqualCallBack,
        pub hash: CFDictionaryHashCallBack,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CFDictionaryValueCallBacks {
        pub version: CFIndex,
        pub retain: CFDictionaryRetainCallBack,
        pub release: CFDictionaryReleaseCallBack,
        pub copyDescription: CFDictionaryCopyDescriptionCallBack,
        pub equal: CFDictionaryEqualCallBack,
    }

    #[repr(C)]
    pub struct __CFDictionary(c_void);

    pub type CFDictionaryRef = *const __CFDictionary;
    pub type CFMutableDictionaryRef = *mut __CFDictionary;

    extern "C" {
        /*
         * CFDictionary.h
         */

        pub static kCFTypeDictionaryKeyCallBacks: CFDictionaryKeyCallBacks;
        pub static kCFTypeDictionaryValueCallBacks: CFDictionaryValueCallBacks;

        pub fn CFDictionaryContainsKey(theDict: CFDictionaryRef, key: *const c_void) -> Boolean;
        pub fn CFDictionaryCreate(
            allocator: CFAllocatorRef,
            keys: *const *const c_void,
            values: *const *const c_void,
            numValues: CFIndex,
            keyCallBacks: *const CFDictionaryKeyCallBacks,
            valueCallBacks: *const CFDictionaryValueCallBacks,
        ) -> CFDictionaryRef;
        pub fn CFDictionaryGetCount(theDict: CFDictionaryRef) -> CFIndex;
        pub fn CFDictionaryGetTypeID() -> CFTypeID;
        pub fn CFDictionaryGetValueIfPresent(
            theDict: CFDictionaryRef,
            key: *const c_void,
            value: *mut *const c_void,
        ) -> Boolean;
        pub fn CFDictionaryApplyFunction(
            theDict: CFDictionaryRef,
            applier: CFDictionaryApplierFunction,
            context: *mut c_void,
        );
        pub fn CFDictionaryGetKeysAndValues(
            theDict: CFDictionaryRef,
            keys: *mut *const c_void,
            values: *mut *const c_void,
        );

        pub fn CFDictionaryCreateMutable(
            allocator: CFAllocatorRef,
            capacity: CFIndex,
            keyCallbacks: *const CFDictionaryKeyCallBacks,
            valueCallbacks: *const CFDictionaryValueCallBacks,
        ) -> CFMutableDictionaryRef;
        pub fn CFDictionaryCreateMutableCopy(
            allocator: CFAllocatorRef,
            capacity: CFIndex,
            theDict: CFDictionaryRef,
        ) -> CFMutableDictionaryRef;
        pub fn CFDictionaryAddValue(
            theDict: CFMutableDictionaryRef,
            key: *const c_void,
            value: *const c_void,
        );
        pub fn CFDictionarySetValue(
            theDict: CFMutableDictionaryRef,
            key: *const c_void,
            value: *const c_void,
        );
        pub fn CFDictionaryReplaceValue(
            theDict: CFMutableDictionaryRef,
            key: *const c_void,
            value: *const c_void,
        );
        pub fn CFDictionaryRemoveValue(theDict: CFMutableDictionaryRef, key: *const c_void);
        pub fn CFDictionaryRemoveAllValues(theDict: CFMutableDictionaryRef);
    }
}

pub mod string {
    // Copyright 2013-2015 The Servo Project Developers. See the COPYRIGHT
    // file at the top-level directory of this distribution.
    //
    // Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
    // http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
    // <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
    // option. This file may not be copied, modified, or distributed
    // except according to those terms.

    use std::os::raw::{c_char, c_ushort, c_void};

    use super::{Boolean, CFAllocatorRef, CFIndex, CFOptionFlags, CFRange, CFTypeID};

    pub type UniChar = c_ushort;

    // CFString.h

    pub type CFStringCompareFlags = CFOptionFlags;
    //static kCFCompareCaseInsensitive: CFStringCompareFlags = 1;
    //static kCFCompareBackwards: CFStringCompareFlags = 4;
    //static kCFCompareAnchored: CFStringCompareFlags = 8;
    //static kCFCompareNonliteral: CFStringCompareFlags = 16;
    //static kCFCompareLocalized: CFStringCompareFlags = 32;
    //static kCFCompareNumerically: CFStringCompareFlags = 64;
    //static kCFCompareDiacriticInsensitive: CFStringCompareFlags = 128;
    //static kCFCompareWidthInsensitive: CFStringCompareFlags = 256;
    //static kCFCompareForcedOrdering: CFStringCompareFlags = 512;

    pub type CFStringEncoding = u32;

    // macOS built-in encodings.

    //static kCFStringEncodingMacRoman: CFStringEncoding = 0;
    //static kCFStringEncodingWindowsLatin1: CFStringEncoding = 0x0500;
    //static kCFStringEncodingISOLatin1: CFStringEncoding = 0x0201;
    //static kCFStringEncodingNextStepLatin: CFStringEncoding = 0x0B01;
    //static kCFStringEncodingASCII: CFStringEncoding = 0x0600;
    //static kCFStringEncodingUnicode: CFStringEncoding = 0x0100;
    pub static kCFStringEncodingUTF8: CFStringEncoding = 0x08000100;
    //static kCFStringEncodingNonLossyASCII: CFStringEncoding = 0x0BFF;

    //static kCFStringEncodingUTF16: CFStringEncoding = 0x0100;
    //static kCFStringEncodingUTF16BE: CFStringEncoding = 0x10000100;
    //static kCFStringEncodingUTF16LE: CFStringEncoding = 0x14000100;
    //static kCFStringEncodingUTF32: CFStringEncoding = 0x0c000100;
    //static kCFStringEncodingUTF32BE: CFStringEncoding = 0x18000100;
    //static kCFStringEncodingUTF32LE: CFStringEncoding = 0x1c000100;

    // CFStringEncodingExt.h

    pub type CFStringEncodings = CFIndex;

    // External encodings, except those defined above.
    // Defined above: kCFStringEncodingMacRoman = 0
    //static kCFStringEncodingMacJapanese: CFStringEncoding = 1;
    //static kCFStringEncodingMacChineseTrad: CFStringEncoding = 2;
    //static kCFStringEncodingMacKorean: CFStringEncoding = 3;
    //static kCFStringEncodingMacArabic: CFStringEncoding = 4;
    //static kCFStringEncodingMacHebrew: CFStringEncoding = 5;
    //static kCFStringEncodingMacGreek: CFStringEncoding = 6;
    //static kCFStringEncodingMacCyrillic: CFStringEncoding = 7;
    //static kCFStringEncodingMacDevanagari: CFStringEncoding = 9;
    //static kCFStringEncodingMacGurmukhi: CFStringEncoding = 10;
    //static kCFStringEncodingMacGujarati: CFStringEncoding = 11;
    //static kCFStringEncodingMacOriya: CFStringEncoding = 12;
    //static kCFStringEncodingMacBengali: CFStringEncoding = 13;
    //static kCFStringEncodingMacTamil: CFStringEncoding = 14;
    //static kCFStringEncodingMacTelugu: CFStringEncoding = 15;
    //static kCFStringEncodingMacKannada: CFStringEncoding = 16;
    //static kCFStringEncodingMacMalayalam: CFStringEncoding = 17;
    //static kCFStringEncodingMacSinhalese: CFStringEncoding = 18;
    //static kCFStringEncodingMacBurmese: CFStringEncoding = 19;
    //static kCFStringEncodingMacKhmer: CFStringEncoding = 20;
    //static kCFStringEncodingMacThai: CFStringEncoding = 21;
    //static kCFStringEncodingMacLaotian: CFStringEncoding = 22;
    //static kCFStringEncodingMacGeorgian: CFStringEncoding = 23;
    //static kCFStringEncodingMacArmenian: CFStringEncoding = 24;
    //static kCFStringEncodingMacChineseSimp: CFStringEncoding = 25;
    //static kCFStringEncodingMacTibetan: CFStringEncoding = 26;
    //static kCFStringEncodingMacMongolian: CFStringEncoding = 27;
    //static kCFStringEncodingMacEthiopic: CFStringEncoding = 28;
    //static kCFStringEncodingMacCentralEurRoman: CFStringEncoding = 29;
    //static kCFStringEncodingMacVietnamese: CFStringEncoding = 30;
    //static kCFStringEncodingMacExtArabic: CFStringEncoding = 31;
    //static kCFStringEncodingMacSymbol: CFStringEncoding = 33;
    //static kCFStringEncodingMacDingbats: CFStringEncoding = 34;
    //static kCFStringEncodingMacTurkish: CFStringEncoding = 35;
    //static kCFStringEncodingMacCroatian: CFStringEncoding = 36;
    //static kCFStringEncodingMacIcelandic: CFStringEncoding = 37;
    //static kCFStringEncodingMacRomanian: CFStringEncoding = 38;
    //static kCFStringEncodingMacCeltic: CFStringEncoding = 39;
    //static kCFStringEncodingMacGaelic: CFStringEncoding = 40;
    //static kCFStringEncodingMacFarsi: CFStringEncoding = 0x8C;
    //static kCFStringEncodingMacUkrainian: CFStringEncoding = 0x98;
    //static kCFStringEncodingMacInuit: CFStringEncoding = 0xEC;
    //static kCFStringEncodingMacVT100: CFStringEncoding = 0xFC;
    //static kCFStringEncodingMacHFS: CFStringEncoding = 0xFF;
    // Defined above: kCFStringEncodingISOLatin1 = 0x0201
    //static kCFStringEncodingISOLatin2: CFStringEncoding = 0x0202;
    //static kCFStringEncodingISOLatin3: CFStringEncoding = 0x0203;
    //static kCFStringEncodingISOLatin4: CFStringEncoding = 0x0204;
    //static kCFStringEncodingISOLatinCyrillic: CFStringEncoding = 0x0205;
    //static kCFStringEncodingISOLatinArabic: CFStringEncoding = 0x0206;
    //static kCFStringEncodingISOLatinGreek: CFStringEncoding = 0x0207;
    //static kCFStringEncodingISOLatinHebrew: CFStringEncoding = 0x0208;
    //static kCFStringEncodingISOLatin5: CFStringEncoding = 0x0209;
    //static kCFStringEncodingISOLatin6: CFStringEncoding = 0x020A;
    //static kCFStringEncodingISOLatinThai: CFStringEncoding = 0x020B;
    //static kCFStringEncodingISOLatin7: CFStringEncoding = 0x020D;
    //static kCFStringEncodingISOLatin8: CFStringEncoding = 0x020E;
    //static kCFStringEncodingISOLatin9: CFStringEncoding = 0x020F;
    //static kCFStringEncodingISOLatin10: CFStringEncoding = 0x0210;
    //static kCFStringEncodingDOSLatinUS: CFStringEncoding = 0x0400;
    //static kCFStringEncodingDOSGreek: CFStringEncoding = 0x0405;
    //static kCFStringEncodingDOSBalticRim: CFStringEncoding = 0x0406;
    //static kCFStringEncodingDOSLatin1: CFStringEncoding = 0x0410;
    //static kCFStringEncodingDOSGreek1: CFStringEncoding = 0x0411;
    //static kCFStringEncodingDOSLatin2: CFStringEncoding = 0x0412;
    //static kCFStringEncodingDOSCyrillic: CFStringEncoding = 0x0413;
    //static kCFStringEncodingDOSTurkish: CFStringEncoding = 0x0414;
    //static kCFStringEncodingDOSPortuguese: CFStringEncoding = 0x0415;
    //static kCFStringEncodingDOSIcelandic: CFStringEncoding = 0x0416;
    //static kCFStringEncodingDOSHebrew: CFStringEncoding = 0x0417;
    //static kCFStringEncodingDOSCanadianFrench: CFStringEncoding = 0x0418;
    //static kCFStringEncodingDOSArabic: CFStringEncoding = 0x0419;
    //static kCFStringEncodingDOSNordic: CFStringEncoding = 0x041A;
    //static kCFStringEncodingDOSRussian: CFStringEncoding = 0x041B;
    //static kCFStringEncodingDOSGreek2: CFStringEncoding = 0x041C;
    //static kCFStringEncodingDOSThai: CFStringEncoding = 0x041D;
    //static kCFStringEncodingDOSJapanese: CFStringEncoding = 0x0420;
    //static kCFStringEncodingDOSChineseSimplif: CFStringEncoding = 0x0421;
    //static kCFStringEncodingDOSKorean: CFStringEncoding = 0x0422;
    //static kCFStringEncodingDOSChineseTrad: CFStringEncoding = 0x0423;
    // Defined above: kCFStringEncodingWindowsLatin1 = 0x0500
    //static kCFStringEncodingWindowsLatin2: CFStringEncoding = 0x0501;
    //static kCFStringEncodingWindowsCyrillic: CFStringEncoding = 0x0502;
    //static kCFStringEncodingWindowsGreek: CFStringEncoding = 0x0503;
    //static kCFStringEncodingWindowsLatin5: CFStringEncoding = 0x0504;
    //static kCFStringEncodingWindowsHebrew: CFStringEncoding = 0x0505;
    //static kCFStringEncodingWindowsArabic: CFStringEncoding = 0x0506;
    //static kCFStringEncodingWindowsBalticRim: CFStringEncoding = 0x0507;
    //static kCFStringEncodingWindowsVietnamese: CFStringEncoding = 0x0508;
    //static kCFStringEncodingWindowsKoreanJohab: CFStringEncoding = 0x0510;
    // Defined above: kCFStringEncodingASCII = 0x0600
    //static kCFStringEncodingANSEL: CFStringEncoding = 0x0601;
    //static kCFStringEncodingJIS_X0201_76: CFStringEncoding = 0x0620;
    //static kCFStringEncodingJIS_X0208_83: CFStringEncoding = 0x0621;
    //static kCFStringEncodingJIS_X0208_90: CFStringEncoding = 0x0622;
    //static kCFStringEncodingJIS_X0212_90: CFStringEncoding = 0x0623;
    //static kCFStringEncodingJIS_C6226_78: CFStringEncoding = 0x0624;
    //static kCFStringEncodingShiftJIS_X0213: CFStringEncoding = 0x0628;
    //static kCFStringEncodingShiftJIS_X0213_MenKuTen: CFStringEncoding = 0x0629;
    //static kCFStringEncodingGB_2312_80: CFStringEncoding = 0x0630;
    //static kCFStringEncodingGBK_95: CFStringEncoding = 0x0631;
    //static kCFStringEncodingGB_18030_2000: CFStringEncoding = 0x0632;
    //static kCFStringEncodingKSC_5601_87: CFStringEncoding = 0x0640;
    //static kCFStringEncodingKSC_5601_92_Johab: CFStringEncoding = 0x0641;
    //static kCFStringEncodingCNS_11643_92_P1: CFStringEncoding = 0x0651;
    //static kCFStringEncodingCNS_11643_92_P2: CFStringEncoding = 0x0652;
    //static kCFStringEncodingCNS_11643_92_P3: CFStringEncoding = 0x0653;
    //static kCFStringEncodingISO_2022_JP: CFStringEncoding = 0x0820;
    //static kCFStringEncodingISO_2022_JP_2: CFStringEncoding = 0x0821;
    //static kCFStringEncodingISO_2022_JP_1: CFStringEncoding = 0x0822;
    //static kCFStringEncodingISO_2022_JP_3: CFStringEncoding = 0x0823;
    //static kCFStringEncodingISO_2022_CN: CFStringEncoding = 0x0830;
    //static kCFStringEncodingISO_2022_CN_EXT: CFStringEncoding = 0x0831;
    //static kCFStringEncodingISO_2022_KR: CFStringEncoding = 0x0840;
    //static kCFStringEncodingEUC_JP: CFStringEncoding = 0x0920;
    //static kCFStringEncodingEUC_CN: CFStringEncoding = 0x0930;
    //static kCFStringEncodingEUC_TW: CFStringEncoding = 0x0931;
    //static kCFStringEncodingEUC_KR: CFStringEncoding = 0x0940;
    //static kCFStringEncodingShiftJIS: CFStringEncoding = 0x0A01;
    //static kCFStringEncodingKOI8_R: CFStringEncoding = 0x0A02;
    //static kCFStringEncodingBig5: CFStringEncoding = 0x0A03;
    //static kCFStringEncodingMacRomanLatin1: CFStringEncoding = 0x0A04;
    //static kCFStringEncodingHZ_GB_2312: CFStringEncoding = 0x0A05;
    //static kCFStringEncodingBig5_HKSCS_1999: CFStringEncoding = 0x0A06;
    //static kCFStringEncodingVISCII: CFStringEncoding = 0x0A07;
    //static kCFStringEncodingKOI8_U: CFStringEncoding = 0x0A08;
    //static kCFStringEncodingBig5_E: CFStringEncoding = 0x0A09;
    // Defined above: kCFStringEncodingNextStepLatin = 0x0B01
    //static kCFStringEncodingNextStepJapanese: CFStringEncoding = 0x0B02;
    //static kCFStringEncodingEBCDIC_US: CFStringEncoding = 0x0C01;
    //static kCFStringEncodingEBCDIC_CP037: CFStringEncoding = 0x0C02;
    //static kCFStringEncodingUTF7: CFStringEncoding = 0x04000100;
    //static kCFStringEncodingUTF7_IMAP: CFStringEncoding = 0x0A10;
    //static kCFStringEncodingShiftJIS_X0213_00: CFStringEncoding = 0x0628; /* Deprecated */
    #[repr(C)]
    pub struct __CFString(c_void);

    pub type CFStringRef = *const __CFString;

    extern "C" {
        /*
         * CFString.h
         */

        // N.B. organized according to "Functions by task" in docs

        /* Creating a CFString */
        //fn CFSTR
        //fn CFStringCreateArrayBySeparatingStrings
        //fn CFStringCreateByCombiningStrings
        //fn CFStringCreateCopy
        //fn CFStringCreateFromExternalRepresentation
        pub fn CFStringCreateWithBytes(
            alloc: CFAllocatorRef,
            bytes: *const u8,
            numBytes: CFIndex,
            encoding: CFStringEncoding,
            isExternalRepresentation: Boolean,
        ) -> CFStringRef;
        pub fn CFStringCreateWithBytesNoCopy(
            alloc: CFAllocatorRef,
            bytes: *const u8,
            numBytes: CFIndex,
            encoding: CFStringEncoding,
            isExternalRepresentation: Boolean,
            contentsDeallocator: CFAllocatorRef,
        ) -> CFStringRef;
        //fn CFStringCreateWithCharacters
        pub fn CFStringCreateWithCharactersNoCopy(
            alloc: CFAllocatorRef,
            chars: *const UniChar,
            numChars: CFIndex,
            contentsDeallocator: CFAllocatorRef,
        ) -> CFStringRef;
        pub fn CFStringCreateWithCString(
            alloc: CFAllocatorRef,
            cStr: *const c_char,
            encoding: CFStringEncoding,
        ) -> CFStringRef;
        //fn CFStringCreateWithCStringNoCopy
        //fn CFStringCreateWithFormat
        //fn CFStringCreateWithFormatAndArguments
        //fn CFStringCreateWithPascalString
        //fn CFStringCreateWithPascalStringNoCopy
        //fn CFStringCreateWithSubstring

        /* Searching Strings */
        //fn CFStringCreateArrayWithFindResults
        //fn CFStringFind
        //fn CFStringFindCharacterFromSet
        //fn CFStringFindWithOptions
        //fn CFStringFindWithOptionsAndLocale
        //fn CFStringGetLineBounds

        /* Comparing Strings */
        //fn CFStringCompare
        //fn CFStringCompareWithOptions
        //fn CFStringCompareWithOptionsAndLocale
        //fn CFStringHasPrefix
        //fn CFStringHasSuffix

        /* Accessing Characters */
        //fn CFStringCreateExternalRepresentation
        pub fn CFStringGetBytes(
            theString: CFStringRef,
            range: CFRange,
            encoding: CFStringEncoding,
            lossByte: u8,
            isExternalRepresentation: Boolean,
            buffer: *mut u8,
            maxBufLen: CFIndex,
            usedBufLen: *mut CFIndex,
        ) -> CFIndex;
        //fn CFStringGetCharacterAtIndex
        //fn CFStringGetCharacters
        //fn CFStringGetCharactersPtr
        //fn CFStringGetCharacterFromInlineBuffer
        pub fn CFStringGetCString(
            theString: CFStringRef,
            buffer: *mut c_char,
            bufferSize: CFIndex,
            encoding: CFStringEncoding,
        ) -> Boolean;
        pub fn CFStringGetCStringPtr(
            theString: CFStringRef,
            encoding: CFStringEncoding,
        ) -> *const c_char;
        pub fn CFStringGetLength(theString: CFStringRef) -> CFIndex;
        //fn CFStringGetPascalString
        //fn CFStringGetPascalStringPtr
        //fn CFStringGetRangeOfComposedCharactersAtIndex
        //fn CFStringInitInlineBuffer

        /* Working With Hyphenation */
        //fn CFStringGetHyphenationLocationBeforeIndex
        //fn CFStringIsHyphenationAvailableForLocale

        /* Working With Encodings */
        //fn CFStringConvertEncodingToIANACharSetName
        //fn CFStringConvertEncodingToNSStringEncoding
        //fn CFStringConvertEncodingToWindowsCodepage
        //fn CFStringConvertIANACharSetNameToEncoding
        //fn CFStringConvertNSStringEncodingToEncoding
        //fn CFStringConvertWindowsCodepageToEncoding
        //fn CFStringGetFastestEncoding
        //fn CFStringGetListOfAvailableEncodings
        //fn CFStringGetMaximumSizeForEncoding
        //fn CFStringGetMostCompatibleMacStringEncoding
        //fn CFStringGetNameOfEncoding
        //fn CFStringGetSmallestEncoding
        //fn CFStringGetSystemEncoding
        //fn CFStringIsEncodingAvailable

        /* Getting Numeric Values */
        //fn CFStringGetDoubleValue
        //fn CFStringGetIntValue

        /* Getting String Properties */
        //fn CFShowStr
        pub fn CFStringGetTypeID() -> CFTypeID;

        /* String File System Representations */
        //fn CFStringCreateWithFileSystemRepresentation
        //fn CFStringGetFileSystemRepresentation
        //fn CFStringGetMaximumSizeOfFileSystemRepresentation

        /* Getting Paragraph Bounds */
        //fn CFStringGetParagraphBounds

        /* Managing Surrogates */
        //fn CFStringGetLongCharacterForSurrogatePair
        //fn CFStringGetSurrogatePairForLongCharacter
        //fn CFStringIsSurrogateHighCharacter
        //fn CFStringIsSurrogateLowCharacter
    }
}

pub mod uuid {
    // Copyright 2013-2015 The Servo Project Developers. See the COPYRIGHT
    // file at the top-level directory of this distribution.
    //
    // Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
    // http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
    // <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
    // option. This file may not be copied, modified, or distributed
    // except according to those terms.

    use std::os::raw::c_void;

    use super::{CFAllocatorRef, CFTypeID};

    #[repr(C)]
    pub struct __CFUUID(c_void);

    pub type CFUUIDRef = *const __CFUUID;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct CFUUIDBytes {
        pub byte0: u8,
        pub byte1: u8,
        pub byte2: u8,
        pub byte3: u8,
        pub byte4: u8,
        pub byte5: u8,
        pub byte6: u8,
        pub byte7: u8,
        pub byte8: u8,
        pub byte9: u8,
        pub byte10: u8,
        pub byte11: u8,
        pub byte12: u8,
        pub byte13: u8,
        pub byte14: u8,
        pub byte15: u8,
    }

    extern "C" {
        /*
         * CFUUID.h
         */
        pub fn CFUUIDCreate(allocator: CFAllocatorRef) -> CFUUIDRef;
        pub fn CFUUIDCreateFromUUIDBytes(
            allocator: CFAllocatorRef,
            bytes: CFUUIDBytes,
        ) -> CFUUIDRef;
        pub fn CFUUIDGetUUIDBytes(uuid: CFUUIDRef) -> CFUUIDBytes;

        pub fn CFUUIDGetTypeID() -> CFTypeID;
    }
}

/*
extern crate libc;

#[cfg(feature = "with-chrono")]
extern crate chrono;

pub unsafe trait ConcreteCFType: TCFType {}

/// Declare a Rust type that wraps an underlying CoreFoundation type.
///
/// This will provide an implementation of `Drop` using [`CFRelease`].
/// The type must have an implementation of the [`TCFType`] trait, usually
/// provided using the [`impl_TCFType`] macro.
///
/// ```
/// #[macro_use] extern crate core_foundation;
/// // Make sure that the `TCFType` trait is in scope.
/// use core_foundation::base::{CFTypeID, TCFType};
///
/// extern "C" {
///     // We need a function that returns the `CFTypeID`.
///     pub fn ShrubberyGetTypeID() -> CFTypeID;
/// }
///
/// pub struct __Shrubbery {}
/// // The ref type must be a pointer to the underlying struct.
/// pub type ShrubberyRef = *const __Shrubbery;
///
/// declare_TCFType!(Shrubbery, ShrubberyRef);
/// impl_TCFType!(Shrubbery, ShrubberyRef, ShrubberyGetTypeID);
/// # fn main() {}
/// ```
///
/// [`CFRelease`]: https://developer.apple.com/documentation/corefoundation/1521153-cfrelease
/// [`TCFType`]: base/trait.TCFType.html
/// [`impl_TCFType`]: macro.impl_TCFType.html
#[macro_export]
macro_rules! declare_TCFType {
    (
        $(#[$doc:meta])*
        $ty:ident, $raw:ident
    ) => {
        $(#[$doc])*
        pub struct $ty($raw);

        impl Drop for $ty {
            fn drop(&mut self) {
                unsafe { $crate::base::CFRelease(self.as_CFTypeRef()) }
            }
        }
    }
}

/// Provide an implementation of the [`TCFType`] trait for the Rust
/// wrapper type around an underlying CoreFoundation type.
///
/// See [`declare_TCFType`] for details.
///
/// [`declare_TCFType`]: macro.declare_TCFType.html
/// [`TCFType`]: base/trait.TCFType.html
#[macro_export]
macro_rules! impl_TCFType {
    ($ty:ident, $ty_ref:ident, $ty_id:ident) => {
        impl_TCFType!($ty<>, $ty_ref, $ty_id);
        unsafe impl $crate::ConcreteCFType for $ty { }
    };

    ($ty:ident<$($p:ident $(: $bound:path)*),*>, $ty_ref:ident, $ty_id:ident) => {
        impl<$($p $(: $bound)*),*> $crate::base::TCFType for $ty<$($p),*> {
            type Ref = $ty_ref;

            #[inline]
            fn as_concrete_TypeRef(&self) -> $ty_ref {
                self.0
            }

            #[inline]
            unsafe fn wrap_under_get_rule(reference: $ty_ref) -> Self {
                assert!(!reference.is_null(), "Attempted to create a NULL object.");
                let reference = $crate::base::CFRetain(reference as *const ::std::os::raw::c_void) as $ty_ref;
                $crate::base::TCFType::wrap_under_create_rule(reference)
            }

            #[inline]
            fn as_CFTypeRef(&self) -> $crate::base::CFTypeRef {
                self.as_concrete_TypeRef() as $crate::base::CFTypeRef
            }

            #[inline]
            unsafe fn wrap_under_create_rule(reference: $ty_ref) -> Self {
                assert!(!reference.is_null(), "Attempted to create a NULL object.");
                // we need one PhantomData for each type parameter so call ourselves
                // again with @Phantom $p to produce that
                $ty(reference $(, impl_TCFType!(@Phantom $p))*)
            }

            #[inline]
            fn type_id() -> $crate::base::CFTypeID {
                unsafe {
                    $ty_id()
                }
            }
        }

        impl Clone for $ty {
            #[inline]
            fn clone(&self) -> $ty {
                unsafe {
                    $ty::wrap_under_get_rule(self.0)
                }
            }
        }

        impl PartialEq for $ty {
            #[inline]
            fn eq(&self, other: &$ty) -> bool {
                self.as_CFType().eq(&other.as_CFType())
            }
        }

        impl Eq for $ty { }

        unsafe impl<'a> $crate::base::ToVoid<$ty> for &'a $ty {
            fn to_void(&self) -> *const ::std::os::raw::c_void {
                use $crate::base::TCFTypeRef;
                self.as_concrete_TypeRef().as_void_ptr()
            }
        }

        unsafe impl $crate::base::ToVoid<$ty> for $ty {
            fn to_void(&self) -> *const ::std::os::raw::c_void {
                use $crate::base::TCFTypeRef;
                self.as_concrete_TypeRef().as_void_ptr()
            }
        }

        unsafe impl $crate::base::ToVoid<$ty> for $ty_ref {
            fn to_void(&self) -> *const ::std::os::raw::c_void {
                use $crate::base::TCFTypeRef;
                self.as_void_ptr()
            }
        }

    };

    (@Phantom $x:ident) => { ::std::marker::PhantomData };
}

/// Implement `std::fmt::Debug` for the given type.
///
/// This will invoke the implementation of `Debug` for [`CFType`]
/// which invokes [`CFCopyDescription`].
///
/// The type must have an implementation of the [`TCFType`] trait, usually
/// provided using the [`impl_TCFType`] macro.
///
/// [`CFType`]: base/struct.CFType.html#impl-Debug
/// [`CFCopyDescription`]: https://developer.apple.com/documentation/corefoundation/1521252-cfcopydescription?language=objc
/// [`TCFType`]: base/trait.TCFType.html
/// [`impl_TCFType`]: macro.impl_TCFType.html
#[macro_export]
macro_rules! impl_CFTypeDescription {
    ($ty:ident) => {
        // it's fine to use an empty <> list
        impl_CFTypeDescription!($ty<>);
    };
    ($ty:ident<$($p:ident $(: $bound:path)*),*>) => {
        impl<$($p $(: $bound)*),*> ::std::fmt::Debug for $ty<$($p),*> {
            fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                self.as_CFType().fmt(f)
            }
        }
    }
}

#[macro_export]
macro_rules! impl_CFComparison {
    ($ty:ident, $compare:ident) => {
        impl PartialOrd for $ty {
            #[inline]
            fn partial_cmp(&self, other: &$ty) -> Option<::std::cmp::Ordering> {
                unsafe {
                    Some(
                        $compare(
                            self.as_concrete_TypeRef(),
                            other.as_concrete_TypeRef(),
                            ::std::ptr::null_mut(),
                        )
                        .into(),
                    )
                }
            }
        }

        impl Ord for $ty {
            #[inline]
            fn cmp(&self, other: &$ty) -> ::std::cmp::Ordering {
                self.partial_cmp(other).unwrap()
            }
        }
    };
}

/// All Core Foundation types implement this trait. The associated type `Ref` specifies the
/// associated Core Foundation type: e.g. for `CFType` this is `CFTypeRef`; for `CFArray` this is
/// `CFArrayRef`.
///
/// Most structs that implement this trait will do so via the [`impl_TCFType`] macro.
///
/// [`impl_TCFType`]: ../macro.impl_TCFType.html
pub trait TCFType {
    /// The reference type wrapped inside this type.
    type Ref: TCFTypeRef;

    /// Returns the object as its concrete TypeRef.
    fn as_concrete_TypeRef(&self) -> Self::Ref;

    /// Returns an instance of the object, wrapping the underlying `CFTypeRef` subclass. Use this
    /// when following Core Foundation's "Create Rule". The reference count is *not* bumped.
    unsafe fn wrap_under_create_rule(obj: Self::Ref) -> Self;

    /// Returns the type ID for this class.
    fn type_id() -> CFTypeID;

    /// Returns the object as a wrapped `CFType`. The reference count is incremented by one.
    #[inline]
    fn as_CFType(&self) -> CFType {
        unsafe { TCFType::wrap_under_get_rule(self.as_CFTypeRef()) }
    }

    /// Returns the object as a wrapped `CFType`. Consumes self and avoids changing the reference
    /// count.
    #[inline]
    fn into_CFType(self) -> CFType
    where
        Self: Sized,
    {
        let reference = self.as_CFTypeRef();
        mem::forget(self);
        unsafe { TCFType::wrap_under_create_rule(reference) }
    }

    /// Returns the object as a raw `CFTypeRef`. The reference count is not adjusted.
    fn as_CFTypeRef(&self) -> CFTypeRef;

    /// Returns an instance of the object, wrapping the underlying `CFTypeRef` subclass. Use this
    /// when following Core Foundation's "Get Rule". The reference count *is* bumped.
    unsafe fn wrap_under_get_rule(reference: Self::Ref) -> Self;

    /// Returns the reference count of the object. It is unwise to do anything other than test
    /// whether the return value of this method is greater than zero.
    #[inline]
    fn retain_count(&self) -> CFIndex {
        unsafe { CFGetRetainCount(self.as_CFTypeRef()) }
    }

    /// Returns the type ID of this object.
    #[inline]
    fn type_of(&self) -> CFTypeID {
        unsafe { CFGetTypeID(self.as_CFTypeRef()) }
    }

    /// Writes a debugging version of this object on standard error.
    fn show(&self) {
        unsafe { CFShow(self.as_CFTypeRef()) }
    }

    /// Returns true if this value is an instance of another type.
    #[inline]
    fn instance_of<OtherCFType: TCFType>(&self) -> bool {
        self.type_of() == OtherCFType::type_id()
    }
}

impl TCFType for CFType {
    type Ref = CFTypeRef;

    #[inline]
    fn as_concrete_TypeRef(&self) -> CFTypeRef {
        self.0
    }

    #[inline]
    unsafe fn wrap_under_get_rule(reference: CFTypeRef) -> CFType {
        assert!(!reference.is_null(), "Attempted to create a NULL object.");
        let reference: CFTypeRef = CFRetain(reference);
        TCFType::wrap_under_create_rule(reference)
    }

    #[inline]
    fn as_CFTypeRef(&self) -> CFTypeRef {
        self.as_concrete_TypeRef()
    }

    #[inline]
    unsafe fn wrap_under_create_rule(obj: CFTypeRef) -> CFType {
        assert!(!obj.is_null(), "Attempted to create a NULL object.");
        CFType(obj)
    }

    #[inline]
    fn type_id() -> CFTypeID {
        // FIXME(pcwalton): Is this right?
        0
    }
}
*/

#[repr(C)]
pub struct __CFMachPort(c_void);
pub type CFMachPortRef = *const __CFMachPort;
