use cocoa::appkit::{NSApp, NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use cocoa::base::{id, nil, selector};
use cocoa::foundation::{NSAutoreleasePool, NSString};
use objc::{
    declare::ClassDecl,
    rc::autoreleasepool,
    runtime::{Class, Object, Sel},
};
use std::sync::Once;

use crate::menu::{Menu, MenuItem};

static BLOCK_PTR: &'static str = "winitMenuItemBlockPtr";

struct KeyEquivalent<'a> {
    key: &'a str,
    masks: Option<NSEventModifierFlags>,
}

struct Action(Box<dyn Fn() + 'static>);

pub fn initialize(menu: Vec<Menu>) {
    autoreleasepool(|| unsafe {
        let menubar = NSMenu::new(nil).autorelease();

        for menu in menu {
            // create our menu
            let menu_item = NSMenuItem::new(nil).autorelease();
            menubar.addItem_(menu_item);
            // prepare our submenu tree
            let menu_title = NSString::alloc(nil).init_str(&menu.title);
            let menu_object = NSMenu::alloc(nil).initWithTitle_(menu_title).autorelease();

            // create menu
            for item in &menu.items {
                let item_obj: *mut Object = match item {
                    MenuItem::Custom(custom_menu) => {
                        let title = NSString::alloc(nil).init_str(custom_menu.name.as_str());
                        make_menu_item(title, None, None)
                    }
                    MenuItem::Separator => NSMenuItem::separatorItem(nil),
                    MenuItem::About(app_name) => {
                        let title = format!("About {}", app_name);
                        let title_alloc = NSString::alloc(nil).init_str(title.as_str());
                        make_menu_item(
                            title_alloc,
                            Some(selector("orderFrontStandardAboutPanel:")),
                            None,
                        )
                    }
                    MenuItem::CloseWindow => {
                        let title = NSString::alloc(nil).init_str("Close Window");
                        make_menu_item(
                            title,
                            Some(selector("performClose:")),
                            Some(KeyEquivalent {
                                key: "w",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::Quit => {
                        let title = NSString::alloc(nil).init_str("Quit");
                        make_menu_item(
                            title,
                            Some(selector("terminate:")),
                            Some(KeyEquivalent {
                                key: "q",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::Hide => {
                        let title = NSString::alloc(nil).init_str("Hide");
                        make_menu_item(
                            title,
                            Some(selector("hide:")),
                            Some(KeyEquivalent {
                                key: "h",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::HideOthers => {
                        let title = NSString::alloc(nil).init_str("Hide Others");
                        make_menu_item(
                            title,
                            Some(selector("hideOtherApplications:")),
                            Some(KeyEquivalent {
                                key: "h",
                                masks: Some(
                                    NSEventModifierFlags::NSAlternateKeyMask
                                        | NSEventModifierFlags::NSCommandKeyMask,
                                ),
                            }),
                        )
                    }
                    MenuItem::ShowAll => {
                        let title = NSString::alloc(nil).init_str("Show All");
                        make_menu_item(title, Some(selector("unhideAllApplications:")), None)
                    }
                    MenuItem::EnterFullScreen => {
                        let title = NSString::alloc(nil).init_str("Enter Full Screen");
                        make_menu_item(
                            title,
                            Some(selector("toggleFullScreen:")),
                            Some(KeyEquivalent {
                                key: "h",
                                masks: Some(
                                    NSEventModifierFlags::NSCommandKeyMask
                                        | NSEventModifierFlags::NSControlKeyMask,
                                ),
                            }),
                        )
                    }
                    MenuItem::Minimize => {
                        let title = NSString::alloc(nil).init_str("Minimize");
                        make_menu_item(
                            title,
                            Some(selector("performMiniaturize:")),
                            Some(KeyEquivalent {
                                key: "m",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::Zoom => {
                        let title = NSString::alloc(nil).init_str("Zoom");
                        make_menu_item(title, Some(selector("performZoom:")), None)
                    }
                    MenuItem::Copy => {
                        let title = NSString::alloc(nil).init_str("Copy");
                        make_menu_item(
                            title,
                            Some(selector("copy:")),
                            Some(KeyEquivalent {
                                key: "c",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::Cut => {
                        let title = NSString::alloc(nil).init_str("Cut");
                        make_menu_item(
                            title,
                            Some(selector("cut:")),
                            Some(KeyEquivalent {
                                key: "x",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::Paste => {
                        let title = NSString::alloc(nil).init_str("Paste");
                        make_menu_item(
                            title,
                            Some(selector("paste:")),
                            Some(KeyEquivalent {
                                key: "v",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::Undo => {
                        let title = NSString::alloc(nil).init_str("Undo");
                        make_menu_item(
                            title,
                            Some(selector("undo:")),
                            Some(KeyEquivalent {
                                key: "z",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::Redo => {
                        let title = NSString::alloc(nil).init_str("Redo");
                        make_menu_item(
                            title,
                            Some(selector("redo:")),
                            Some(KeyEquivalent {
                                key: "Z",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::SelectAll => {
                        let title = NSString::alloc(nil).init_str("Select All");
                        make_menu_item(
                            title,
                            Some(selector("selectAll:")),
                            Some(KeyEquivalent {
                                key: "a",
                                masks: None,
                            }),
                        )
                    }
                    MenuItem::Services => {
                        let title = NSString::alloc(nil).init_str("Services");
                        let item = make_menu_item(title, None, None);
                        let app_class = class!(NSApplication);
                        let app: id = msg_send![app_class, sharedApplication];
                        let services: id = msg_send![app, servicesMenu];
                        let _: () = msg_send![&*item, setSubmenu: services];
                        item
                    }
                };

                menu_object.addItem_(item_obj);
            }

            menu_item.setSubmenu_(menu_object);
        }

        // Set the menu as main menu for the app
        let app = NSApp();
        app.setMainMenu_(menubar);
    });
}

fn make_menu_item(
    title: *mut Object,
    selector: Option<Sel>,
    key_equivalent: Option<KeyEquivalent<'_>>,
) -> *mut Object {
    unsafe {
        let (key, masks) = match key_equivalent {
            Some(ke) => (NSString::alloc(nil).init_str(ke.key), ke.masks),
            None => (NSString::alloc(nil).init_str(""), None),
        };
        // if no selector defined, that mean it's a custom
        // menu so fire our handler
        let selector = match selector {
            Some(selector) => selector,
            None => sel!(fireBlockAction:),
        };

        // allocate our item to our class
        let alloc: id = msg_send![make_menu_item_class(), alloc];
        let item =
            NSMenuItem::alloc(alloc).initWithTitle_action_keyEquivalent_(title, selector, key);

        if let Some(masks) = masks {
            item.setKeyEquivalentModifierMask_(masks)
        }

        item
    }
}

fn make_menu_item_class() -> *const Class {
    static mut APP_CLASS: *const Class = 0 as *const Class;
    static INIT: Once = Once::new();

    INIT.call_once(|| unsafe {
        let superclass = class!(NSMenuItem);
        let mut decl = ClassDecl::new("WinitMenuItem", superclass).unwrap();
        decl.add_ivar::<usize>(BLOCK_PTR);

        decl.add_method(
            sel!(dealloc),
            dealloc_custom_menuitem as extern "C" fn(&Object, _),
        );
        decl.add_method(
            sel!(fireBlockAction:),
            fire_custom_menu_click as extern "C" fn(&Object, _, id),
        );

        APP_CLASS = decl.register();
    });

    unsafe { APP_CLASS }
}

extern "C" fn fire_custom_menu_click(this: &Object, _: Sel, _item: id) {
    println!("CLICK");
}

extern "C" fn dealloc_custom_menuitem(this: &Object, _: Sel) {
    println!("DEALOC");
    unsafe {
        let ptr: usize = *this.get_ivar(BLOCK_PTR);
        let obj = ptr as *mut Action;
        println!("Action {:?}", obj);

        if !obj.is_null() {
            let _handler = Box::from_raw(obj);
        }

        //let _: () = msg_send![this, setTarget:nil];
        let _: () = msg_send![super(this, class!(NSMenuItem)), dealloc];
    }
}
