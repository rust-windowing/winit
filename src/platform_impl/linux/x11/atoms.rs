//! Atom management.

use x11rb::{
    atom_manager, cookie,
    errors::{ConnectionError, ReplyError},
    protocol::xproto::{self, ConnectionExt as _},
    xcb_ffi::XCBConnection,
};

// Note: text/uri-list is a special case and is handled separately.

macro_rules! make_atom_manager {
    (
        $($name: ident),*
    ) => {
        /// The various atoms used within `Winit`.
        #[allow(non_camel_case_types)]
        pub(crate) enum AtomType {
            TextUriList,
            $(
                $name,
            )*
        }

        /// A collection of atoms used within `Winit`.
        pub(crate) struct Atoms {
            /// The textual atom list.
            some_atoms: SomeAtoms,

            /// `text/uri-list`.
            text_uri_list: xproto::Atom,
        }

        /// The cookie for the `Atoms` structure.
        pub(crate) struct AtomsCookie<'a> {
            /// The textual atom list.
            some_atoms: SomeAtomsCookie<'a, XCBConnection>,

            /// `text/uri-list`.
            text_uri_list: cookie::Cookie<'a, XCBConnection, xproto::InternAtomReply>,
        }

        impl Atoms {
            /// Create a new `Atoms` structure.
            pub(crate) fn request(conn: &XCBConnection) -> Result<AtomsCookie<'_>, ConnectionError> {
                let some_atoms = SomeAtoms::new(conn)?;
                let text_uri_list = conn.intern_atom(true, b"text/uri-list")?;
                Ok(AtomsCookie {
                    some_atoms,
                    text_uri_list,
                })
            }
        }

        impl AtomsCookie<'_> {
            /// Finish the creation of the `Atoms` structure.
            pub(crate) fn reply(self) -> Result<Atoms, ReplyError> {
                let some_atoms = self.some_atoms.reply()?;
                let text_uri_list = self.text_uri_list.reply()?.atom;
                Ok(Atoms {
                    some_atoms,
                    text_uri_list,
                })
            }
        }

        atom_manager! {
            /// A collection of atoms used within `Winit`.
            SomeAtoms : SomeAtomsCookie {
                $(
                    $name,
                )*
            }
        }

        impl std::ops::Index<AtomType> for Atoms {
            type Output = xproto::Atom;

            fn index(&self, atom: AtomType) -> &Self::Output {
                match atom {
                    AtomType::TextUriList => &self.text_uri_list,
                    $(
                        AtomType::$name => &self.some_atoms.$name,
                    )*
                }
            }
        }
    };
}

make_atom_manager! {
    // Window type hints.
    _NET_WM_WINDOW_TYPE,
    _NET_WM_WINDOW_TYPE_DESKTOP,
    _NET_WM_WINDOW_TYPE_DOCK,
    _NET_WM_WINDOW_TYPE_TOOLBAR,
    _NET_WM_WINDOW_TYPE_MENU,
    _NET_WM_WINDOW_TYPE_UTILITY,
    _NET_WM_WINDOW_TYPE_SPLASH,
    _NET_WM_WINDOW_TYPE_DIALOG,
    _NET_WM_WINDOW_TYPE_DROPDOWN_MENU,
    _NET_WM_WINDOW_TYPE_POPUP_MENU,
    _NET_WM_WINDOW_TYPE_TOOLTIP,
    _NET_WM_WINDOW_TYPE_NOTIFICATION,
    _NET_WM_WINDOW_TYPE_COMBO,
    _NET_WM_WINDOW_TYPE_DND,
    _NET_WM_WINDOW_TYPE_NORMAL,

    // Other _NET_WM hints
    _NET_WM_MOVERESIZE,
    _NET_WM_NAME,
    _NET_WM_ICON,
    _NET_WM_PID,
    _NET_WM_PING,
    _NET_WM_STATE,
    _NET_WM_STATE_ABOVE,
    _NET_WM_STATE_BELOW,
    _NET_WM_STATE_FULLSCREEN,
    _NET_WM_STATE_HIDDEN,
    _NET_WM_STATE_MAXIMIZED_HORZ,
    _NET_WM_STATE_MAXIMIZED_VERT,

    // DND atoms.
    XdndAware,
    XdndEnter,
    XdndLeave,
    XdndDrop,
    XdndPosition,
    XdndStatus,
    XdndActionPrivate,
    XdndSelection,
    XdndFinished,
    XdndTypeList,

    // _NET hints.
    _NET_ACTIVE_WINDOW,
    _NET_FRAME_EXTENTS,
    _NET_CLIENT_LIST,
    _NET_SUPPORTED,
    _NET_SUPPORTING_WM_CHECK,

    // Misc WM hints.
    WM_CHANGE_STATE,
    WM_CLIENT_MACHINE,
    WM_DELETE_WINDOW,
    WM_PROTOCOLS,
    WM_STATE,

    // Other misc atoms.
    _MOTIF_WM_HINTS,
    _GTK_THEME_VARIANT,
    CARD32,
    UTF8_STRING,
    None
}

pub(crate) use AtomType::*;

/// Prevent the `None` atom from shadowing `Option::None`.
pub use std::option::Option::None;
