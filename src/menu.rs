#[derive(Debug, Clone)]
pub struct Menu {
    pub title: String,
    pub items: Vec<MenuItem>,
}

impl Menu {
    pub fn new(title: &str, items: Vec<MenuItem>) -> Self {
        Self {
            title: String::from(title),
            items,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CustomMenu {
    pub id: String,
    pub name: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone)]
pub enum MenuItem {
    /// A custom MenuItem. This type functions as a builder, so you can customize it easier.
    /// You can (and should) create this variant via the `new(title)` method, but if you need to do
    /// something crazier, then wrap it in this and you can hook into the Cacao menu system
    /// accordingly.
    Custom(CustomMenu),

    /// Shows a standard "About" item,  which will bring up the necessary window when clicked
    /// (include a `credits.html` in your App to make use of here). The argument baked in here
    /// should be your app name.
    About(String),

    /// A standard "hide the app" menu item.
    Hide,

    /// A standard "Services" menu item.
    Services,

    /// A "hide all other windows" menu item.
    HideOthers,

    /// A menu item to show all the windows for this app.
    ShowAll,

    /// Close the current window.
    CloseWindow,

    /// A "quit this app" menu icon.
    Quit,

    /// A menu item for enabling copying (often text) from responders.
    Copy,

    /// A menu item for enabling cutting (often text) from responders.
    Cut,

    /// An "undo" menu item; particularly useful for supporting the cut/copy/paste/undo lifecycle
    /// of events.
    Undo,

    /// An "redo" menu item; particularly useful for supporting the cut/copy/paste/undo lifecycle
    /// of events.
    Redo,

    /// A menu item for selecting all (often text) from responders.
    SelectAll,

    /// A menu item for pasting (often text) into responders.
    Paste,

    /// A standard "enter full screen" item.
    EnterFullScreen,

    /// An item for minimizing the window with the standard system controls.
    Minimize,

    /// An item for instructing the app to zoom. Your app must react to this with necessary window
    /// lifecycle events.
    Zoom,

    /// Represents a Separator. It's useful nonetheless for
    /// separating out pieces of the `NSMenu` structure.
    Separator,
}

impl MenuItem {
    pub fn new(unique_menu_id: String, title: String) -> Self {
        MenuItem::Custom(CustomMenu {
            id: unique_menu_id,
            key: None,
            name: title,
        })
    }

    pub fn key(mut self, key: &str) -> Self {
        if let MenuItem::Custom(ref mut custom_menu) = self {
            custom_menu.key = Some(key.to_string());
        }
        self
    }
}
