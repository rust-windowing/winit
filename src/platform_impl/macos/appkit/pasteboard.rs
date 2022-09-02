use objc2::foundation::NSString;

pub type NSPasteboardType = NSString;

extern "C" {
    pub static NSFilenamesPboardType: &'static NSPasteboardType;
}
