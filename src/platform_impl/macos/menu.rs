use icrate::AppKit::{
    NSEventModifierFlagCommand, NSEventModifierFlagOption, NSEventModifierFlags, NSMenu, NSMenuItem,
};
use icrate::Foundation::{ns_string, MainThreadMarker, NSProcessInfo, NSString};
use objc2::rc::Id;
use objc2::runtime::Sel;
use objc2::sel;

use super::appkit::NSApp;

struct KeyEquivalent<'a> {
    key: &'a NSString,
    masks: Option<NSEventModifierFlags>,
}

pub fn initialize(mtm: MainThreadMarker) {
    let menubar = NSMenu::new(mtm);
    let app_menu_item = NSMenuItem::new(mtm);
    menubar.addItem(&app_menu_item);

    let app_menu = NSMenu::new(mtm);
    let process_name = NSProcessInfo::processInfo().processName();

    // About menu item
    let about_item_title = ns_string!("About ").stringByAppendingString(&process_name);
    let about_item = menu_item(
        mtm,
        &about_item_title,
        Some(sel!(orderFrontStandardAboutPanel:)),
        None,
    );

    // Services menu item
    let services_menu = NSMenu::new(mtm);
    let services_item = menu_item(mtm, ns_string!("Services"), None, None);
    services_item.setSubmenu(Some(&services_menu));

    // Seperator menu item
    let sep_first = NSMenuItem::separatorItem(mtm);

    // Hide application menu item
    let hide_item_title = ns_string!("Hide ").stringByAppendingString(&process_name);
    let hide_item = menu_item(
        mtm,
        &hide_item_title,
        Some(sel!(hide:)),
        Some(KeyEquivalent {
            key: ns_string!("h"),
            masks: None,
        }),
    );

    // Hide other applications menu item
    let hide_others_item_title = ns_string!("Hide Others");
    let hide_others_item = menu_item(
        mtm,
        hide_others_item_title,
        Some(sel!(hideOtherApplications:)),
        Some(KeyEquivalent {
            key: ns_string!("h"),
            masks: Some(NSEventModifierFlagOption | NSEventModifierFlagCommand),
        }),
    );

    // Show applications menu item
    let show_all_item_title = ns_string!("Show All");
    let show_all_item = menu_item(
        mtm,
        show_all_item_title,
        Some(sel!(unhideAllApplications:)),
        None,
    );

    // Seperator menu item
    let sep = NSMenuItem::separatorItem(mtm);

    // Quit application menu item
    let quit_item_title = ns_string!("Quit ").stringByAppendingString(&process_name);
    let quit_item = menu_item(
        mtm,
        &quit_item_title,
        Some(sel!(terminate:)),
        Some(KeyEquivalent {
            key: ns_string!("q"),
            masks: None,
        }),
    );

    app_menu.addItem(&about_item);
    app_menu.addItem(&sep_first);
    app_menu.addItem(&services_item);
    app_menu.addItem(&hide_item);
    app_menu.addItem(&hide_others_item);
    app_menu.addItem(&show_all_item);
    app_menu.addItem(&sep);
    app_menu.addItem(&quit_item);
    app_menu_item.setSubmenu(Some(&app_menu));

    let app = NSApp();
    app.setServicesMenu(&services_menu);
    app.setMainMenu(&menubar);
}

fn menu_item(
    mtm: MainThreadMarker,
    title: &NSString,
    selector: Option<Sel>,
    key_equivalent: Option<KeyEquivalent<'_>>,
) -> Id<NSMenuItem> {
    let (key, masks) = match key_equivalent {
        Some(ke) => (ke.key, ke.masks),
        None => (ns_string!(""), None),
    };
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(mtm.alloc(), title, selector, key)
    };
    if let Some(masks) = masks {
        item.setKeyEquivalentModifierMask(masks)
    }

    item
}
