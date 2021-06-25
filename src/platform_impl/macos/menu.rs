use super::util::IdRef;
use cocoa::appkit::{NSApp, NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use cocoa::base::{nil, selector};
use cocoa::foundation::{NSProcessInfo, NSString};
use objc::{
    rc::autoreleasepool,
    runtime::{Object, Sel},
};

struct KeyEquivalent<'a> {
    key: &'a str,
    masks: Option<NSEventModifierFlags>,
}

pub fn initialize() {
    autoreleasepool(|| unsafe {
        let menubar = IdRef::new(NSMenu::new(nil));
        let app_menu_item = IdRef::new(NSMenuItem::new(nil));
        menubar.addItem_(*app_menu_item);
        let app = NSApp();
        app.setMainMenu_(*menubar);

        let app_menu = NSMenu::new(nil);
        let process_name = NSProcessInfo::processInfo(nil).processName();

        // About menu item
        let about_item_prefix = NSString::alloc(nil).init_str("About ");
        let about_item_title = about_item_prefix.stringByAppendingString_(process_name);
        let about_item = menu_item(
            about_item_title,
            selector("orderFrontStandardAboutPanel:"),
            None,
        );

        // Seperator menu item
        let sep_first = NSMenuItem::separatorItem(nil);

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
        );

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
        );

        // Show applications menu item
        let show_all_item_title = NSString::alloc(nil).init_str("Show All");
        let show_all_item = menu_item(
            show_all_item_title,
            selector("unhideAllApplications:"),
            None,
        );

        // Seperator menu item
        let sep = NSMenuItem::separatorItem(nil);

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
        );

        app_menu.addItem_(about_item);
        app_menu.addItem_(sep_first);
        app_menu.addItem_(hide_item);
        app_menu.addItem_(hide_others_item);
        app_menu.addItem_(show_all_item);
        app_menu.addItem_(sep);
        app_menu.addItem_(quit_item);
        app_menu_item.setSubmenu_(app_menu);
    });
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
        if let Some(masks) = masks {
            item.setKeyEquivalentModifierMask_(masks)
        }

        item
    }
}
