use cocoa::appkit::{
    NSApp, NSApplication, NSApplicationActivationPolicyRegular, NSEventModifierFlags, NSMenu,
    NSMenuItem,
};
use cocoa::base::{nil, selector};
use cocoa::foundation::{NSAutoreleasePool, NSProcessInfo, NSString};
use objc::runtime::{Object, Sel};

struct KeyEquivalent<'a> {
    key: &'a str,
    masks: Option<NSEventModifierFlags>,
}

pub fn initialize() {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);

        let app = NSApp();
        app.setActivationPolicy_(NSApplicationActivationPolicyRegular);

        let menubar = NSMenu::new(nil).autorelease();
        let app_menu_item = NSMenuItem::new(nil).autorelease();
        menubar.addItem_(app_menu_item);
        app.setMainMenu_(menubar);

        let app_menu = NSMenu::new(nil).autorelease();
        let process_name = NSProcessInfo::processInfo(nil).processName();

        // About menu item
        let about_item_prefix = NSString::alloc(nil).init_str("About ");
        let about_item_title = about_item_prefix.stringByAppendingString_(process_name);
        let about_item = menu_item(
            about_item_title,
            selector("orderFrontStandardAboutPanel:"),
            None,
        )
        .autorelease();

        // Seperator menu item
        let sep_first = NSMenuItem::separatorItem(nil).autorelease();

        // Hide application menu item
        let hide_item_prefix = NSString::alloc(nil).init_str("Hide ");
        let hide_item_title = hide_item_prefix.stringByAppendingString_(process_name);
        let hide_item = menu_item(
            hide_item_title,
            selector("hide:"),
            Some(KeyEquivalent {
                key: "h",
                masks: None,
            }),
        )
        .autorelease();

        // Hide other applications menu item
        let hide_others_item_title = NSString::alloc(nil).init_str("Hide Others");
        let hide_others_item = menu_item(
            hide_others_item_title,
            selector("hideOtherApplications:"),
            Some(KeyEquivalent {
                key: "h",
                masks: Some(
                    NSEventModifierFlags::NSAlternateKeyMask
                        | NSEventModifierFlags::NSCommandKeyMask,
                ),
            }),
        )
        .autorelease();

        // Show applications menu item
        let show_all_item_title = NSString::alloc(nil).init_str("Show All");
        let show_all_item = menu_item(
            show_all_item_title,
            selector("unhideAllApplications:"),
            None,
        )
        .autorelease();

        // Seperator menu item
        let sep = NSMenuItem::separatorItem(nil).autorelease();

        // Quit application menu item
        let quit_item_prefix = NSString::alloc(nil).init_str("Quit ");
        let quit_item_title = quit_item_prefix.stringByAppendingString_(process_name);
        let quit_item = menu_item(
            quit_item_title,
            selector("terminate:"),
            Some(KeyEquivalent {
                key: "q",
                masks: None,
            }),
        )
        .autorelease();

        app_menu.addItem_(about_item);
        app_menu.addItem_(sep_first);
        app_menu.addItem_(hide_item);
        app_menu.addItem_(hide_others_item);
        app_menu.addItem_(show_all_item);
        app_menu.addItem_(sep);
        app_menu.addItem_(quit_item);
        app_menu_item.setSubmenu_(app_menu);
    }
}

fn menu_item(
    title: *mut Object,
    selector: Sel,
    key_equivalent: Option<KeyEquivalent<'_>>,
) -> *mut Object {
    unsafe {
        let (key, masks) = match key_equivalent {
            Some(ke) => (NSString::alloc(nil).init_str(ke.key), ke.masks),
            None => (NSString::alloc(nil).init_str(""), None),
        };
        let item = NSMenuItem::alloc(nil).initWithTitle_action_keyEquivalent_(title, selector, key);
        match masks {
            Some(masks) => item.setKeyEquivalentModifierMask_(masks),
            _ => {}
        }

        item
    }
}
