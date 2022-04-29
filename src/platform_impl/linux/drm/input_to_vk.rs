use crate::event::VirtualKeyCode;

pub const CHAR_MAPPINGS: [Option<VirtualKeyCode>; 234] = [
    Some(VirtualKeyCode::Space),
    Some(VirtualKeyCode::Escape),
    Some(VirtualKeyCode::Key1),
    Some(VirtualKeyCode::Key2),
    Some(VirtualKeyCode::Key3),
    Some(VirtualKeyCode::Key4),
    Some(VirtualKeyCode::Key5),
    Some(VirtualKeyCode::Key6),
    Some(VirtualKeyCode::Key7),
    Some(VirtualKeyCode::Key8),
    Some(VirtualKeyCode::Key9),
    Some(VirtualKeyCode::Key0),
    Some(VirtualKeyCode::Minus),
    Some(VirtualKeyCode::Equals),
    Some(VirtualKeyCode::Back),
    Some(VirtualKeyCode::Tab),
    Some(VirtualKeyCode::Q),
    Some(VirtualKeyCode::W),
    Some(VirtualKeyCode::E),
    Some(VirtualKeyCode::R),
    Some(VirtualKeyCode::T),
    Some(VirtualKeyCode::Y),
    Some(VirtualKeyCode::U),
    Some(VirtualKeyCode::I),
    Some(VirtualKeyCode::O),
    Some(VirtualKeyCode::P),
    Some(VirtualKeyCode::LBracket),
    Some(VirtualKeyCode::RBracket),
    Some(VirtualKeyCode::Return),
    Some(VirtualKeyCode::LControl),
    Some(VirtualKeyCode::A),
    Some(VirtualKeyCode::S),
    Some(VirtualKeyCode::D),
    Some(VirtualKeyCode::F),
    Some(VirtualKeyCode::G),
    Some(VirtualKeyCode::H),
    Some(VirtualKeyCode::J),
    Some(VirtualKeyCode::K),
    Some(VirtualKeyCode::L),
    Some(VirtualKeyCode::Semicolon),
    Some(VirtualKeyCode::Apostrophe),
    Some(VirtualKeyCode::Grave),
    Some(VirtualKeyCode::LShift),
    Some(VirtualKeyCode::Backslash),
    Some(VirtualKeyCode::Z),
    Some(VirtualKeyCode::X),
    Some(VirtualKeyCode::C),
    Some(VirtualKeyCode::V),
    Some(VirtualKeyCode::B),
    Some(VirtualKeyCode::N),
    Some(VirtualKeyCode::M),
    Some(VirtualKeyCode::Comma),
    Some(VirtualKeyCode::Period),
    Some(VirtualKeyCode::Slash),
    Some(VirtualKeyCode::RShift),
    Some(VirtualKeyCode::Asterisk),
    Some(VirtualKeyCode::LAlt),
    Some(VirtualKeyCode::Space),
    Some(VirtualKeyCode::Capital),
    Some(VirtualKeyCode::F1),
    Some(VirtualKeyCode::F2),
    Some(VirtualKeyCode::F3),
    Some(VirtualKeyCode::F4),
    Some(VirtualKeyCode::F5),
    Some(VirtualKeyCode::F6),
    Some(VirtualKeyCode::F7),
    Some(VirtualKeyCode::F8),
    Some(VirtualKeyCode::F9),
    Some(VirtualKeyCode::F10),
    Some(VirtualKeyCode::Numlock),
    Some(VirtualKeyCode::Scroll),
    Some(VirtualKeyCode::Numpad7),
    Some(VirtualKeyCode::Numpad8),
    Some(VirtualKeyCode::Numpad9),
    Some(VirtualKeyCode::NumpadSubtract),
    Some(VirtualKeyCode::Numpad4),
    Some(VirtualKeyCode::Numpad5),
    Some(VirtualKeyCode::Numpad6),
    Some(VirtualKeyCode::NumpadEquals),
    Some(VirtualKeyCode::Numpad1),
    Some(VirtualKeyCode::Numpad2),
    Some(VirtualKeyCode::Numpad3),
    Some(VirtualKeyCode::Numpad0),
    Some(VirtualKeyCode::NumpadDecimal),
    None, // NULL KEY
    None, // KEY_ZENKAKUHANKAKU
    Some(VirtualKeyCode::OEM102),
    Some(VirtualKeyCode::F11),
    Some(VirtualKeyCode::F12),
    None, // Rollover key
    Some(VirtualKeyCode::Kana),
    None, // KEY_HIRAGANA
    None, // KEY_HENKAN
    None, // KEY_KATAKANAHIRAGANA
    None, // KEY_MUHENKAN
    None, // KEY_KPJPCOMMA
    None, // KEY_KPENTER
    Some(VirtualKeyCode::RControl),
    None, // KEY_KPSLASH
    Some(VirtualKeyCode::Sysrq),
    Some(VirtualKeyCode::RAlt),
    None, // KEY_LINEFEED
    Some(VirtualKeyCode::Home),
    Some(VirtualKeyCode::Up),
    Some(VirtualKeyCode::PageUp),
    Some(VirtualKeyCode::Left),
    Some(VirtualKeyCode::Right),
    Some(VirtualKeyCode::End),
    Some(VirtualKeyCode::Down),
    Some(VirtualKeyCode::PageDown),
    Some(VirtualKeyCode::Insert),
    Some(VirtualKeyCode::Delete),
    None, // KEY_MACRO
    Some(VirtualKeyCode::Mute),
    Some(VirtualKeyCode::VolumeDown),
    Some(VirtualKeyCode::VolumeUp),
    Some(VirtualKeyCode::Power),
    None, // KEY_KPEQUAL
    None, // KEY_KPPLUSMINUS
    Some(VirtualKeyCode::Pause),
    None, // KEY_SCALE
    None, // KEY_KPCOMMA
    None, // KEY_HANGEUL
    None, // KEY_HANJA
    Some(VirtualKeyCode::Yen),
    Some(VirtualKeyCode::LWin),
    Some(VirtualKeyCode::RWin),
    Some(VirtualKeyCode::Compose),
    Some(VirtualKeyCode::Stop),
    None, // KEY_AGAIN
    None, // KEY_PROPS
    None, // KEY_UNDO
    None, // KEY_FRONT
    Some(VirtualKeyCode::Copy),
    None, // KEY_OPEN
    Some(VirtualKeyCode::Paste),
    None, // KEY_FIND
    Some(VirtualKeyCode::Cut),
    None, // KEY_HELP
    None, // KEY_MENU
    Some(VirtualKeyCode::Calculator),
    None, // KEY_SETUP
    Some(VirtualKeyCode::Sleep),
    Some(VirtualKeyCode::Wake),
    None, // KEY_FILE
    None, // KEY_SENDFILE
    None, // KEY_DELETEFILE
    None, // KEY_XFER
    None, // KEY_PROG1
    None, // KEY_PROG1
    None, // KEY_PROG2
    None, // KEY_WWW
    None, // KEY_MSDOS
    None, // KEY_COFFEE
    None, // KEY_ROTATE_DISPLAY
    None, // KEY_CYCLEWINDOWS
    Some(VirtualKeyCode::Mail),
    None, // KEY_BOOKMARKS
    Some(VirtualKeyCode::MyComputer),
    Some(VirtualKeyCode::NavigateBackward),
    Some(VirtualKeyCode::NavigateForward),
    None, // KEY_CLOSECD
    None, // KEY_EJECTCD
    None, // KEY_EJECTCLOSECD
    Some(VirtualKeyCode::NextTrack),
    Some(VirtualKeyCode::PlayPause),
    Some(VirtualKeyCode::PrevTrack),
    None, //  KEY_STOPCD
    Some(VirtualKeyCode::MediaStop),
    None, // KEY_RECORD
    None, // KEY_REWIND
    None, // KEY_PHONE
    None, // KEY_ISO
    None, // KEY_CONFIG
    Some(VirtualKeyCode::WebHome),
    Some(VirtualKeyCode::WebRefresh),
    None, // KEY_EXIT
    None, // KEY_MOVE
    None, // KEY_EDIT
    None, // KEY_SCROLLUP
    None, // KEY_SCROLLDOWN
    None, // KEY_KPLEFTPAREN
    None, // KEY_KPRIGHTPAREN
    None, // KEY_NEW
    None, // KEY_REDO
    Some(VirtualKeyCode::F13),
    Some(VirtualKeyCode::F14),
    Some(VirtualKeyCode::F15),
    Some(VirtualKeyCode::F16),
    Some(VirtualKeyCode::F17),
    Some(VirtualKeyCode::F18),
    Some(VirtualKeyCode::F19),
    Some(VirtualKeyCode::F20),
    Some(VirtualKeyCode::F21),
    Some(VirtualKeyCode::F22),
    Some(VirtualKeyCode::F23),
    Some(VirtualKeyCode::F24),
    None, // KEY_PLAYCD
    None, // KEY_PAUSECD
    None, // KEY_PROG3
    None, // KEY_PROG4
    None, // KEY_DASHBOARD
    None, // KEY_SUSPEND
    None, // KEY_CLOSE
    None, // KEY_PLAY
    None, // KEY_FASTFORWARD
    None, // KEY_BASSBOOST
    None, // KEY_PRINT
    None, // KEY_HP
    None, // KEY_CAMERA
    None, // KEY_SOUND
    None, // KEY_QUESTION
    None, // KEY_EMAIL
    None, // KEY_CHAT
    Some(VirtualKeyCode::WebSearch),
    None, // KEY_CONNECT
    None, // KEY_FINANCE
    None, // KEY_SPORT
    None, // KEY_SHOP
    None, // KEY_ALTERASE
    None, // KEY_CANCEL
    None, // KEY_BRIGHTNESSDOWN
    None, // KEY_BRIGHTNESSUP
    Some(VirtualKeyCode::MediaSelect),
    None, // KEY_SWITCHVIDEOMODE
    None, // KEY_KBDILLUMTOGGLE
    None, // KEY_KBDILLUMDOWN
    None, // KEY_KBDILLUMUP
    None, // KEY_SEND
    None, // KEY_REPLY
    None, // KEY_FORWARDMAIL
    None, // KEY_SAVE
    None, // KEY_DOCUMENTS
    None, // KEY_BATTERY
];
