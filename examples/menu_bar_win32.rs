extern crate winit;

#[cfg(not(target_os = "windows"))]
fn main() {
    println!("This example only works on Windows!");
}

#[cfg(target_os = "windows")]
const ID_FILE_NEW: u16         = 9001;
#[cfg(target_os = "windows")]
const ID_QUIT_APPLICATION: u16 = 9002;

#[cfg(target_os = "windows")]
fn main() {
    use winit::os::windows::WindowBuilderExt;
    let mut events_loop = winit::EventsLoop::new();

    let _window = winit::WindowBuilder::new()
        .with_title("A fantastic window!")
        .with_create_callback(win32_create_callback)
        .build(&events_loop)
        .unwrap();

    events_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent { event,  .. } => {
                match event {
                    winit::WindowEvent::CloseRequested => winit::ControlFlow::Break,
                    winit::WindowEvent::Command(command) => {
                        match command {
                            ID_FILE_NEW => {
                                println!("New File button clicked!");
                                winit::ControlFlow::Continue
                            },
                            ID_QUIT_APPLICATION => {
                                println!("Quit Application button clicked!");
                                winit::ControlFlow::Break
                            },
                            _ => winit::ControlFlow::Continue,
                        }
                    },
                    _ => winit::ControlFlow::Continue,
                }
            },
            _ => winit::ControlFlow::Continue,
        }
    });
}

#[cfg(target_os = "windows")]
fn win32_create_callback(hwnd: winit::winapi::shared::windef::HWND) {

    // Encode a Rust `&str` as a Vec<u16> compatible with the Win32 API
    fn str_to_wide_vec_u16(input: &str) -> Vec<u16> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStrExt;
        let mut s: Vec<u16> = OsString::from(input).as_os_str().encode_wide().into_iter().collect();
        s.push(0);
        s
    }

    use winit::winapi::um::winuser::{SetMenu, CreateMenu, AppendMenuW, DestroyMenu, MF_STRING, MF_POPUP};

    let menu_bar = unsafe { CreateMenu() };
    if menu_bar.is_null() {
        println!("CreateMenu failed!");
        return;
    }

    let popup_menu_1 = unsafe { CreateMenu() };
    if popup_menu_1.is_null() {
        println!("CreateMenu failed!");
        unsafe { DestroyMenu(menu_bar) };
        return;
    }

    let application_top_level_menu_str = str_to_wide_vec_u16("&Application");
    let file_new_str = str_to_wide_vec_u16("New &File");
    let quit_application_str = str_to_wide_vec_u16("&Quit");

    // In order to keep this example simple, we don't check for errors here...

    // Append the "New File" to the popup menu
    unsafe { AppendMenuW(popup_menu_1, MF_STRING, ID_FILE_NEW as usize, file_new_str.as_ptr()) };

    // Append the "Quit" to the popup menu
    unsafe { AppendMenuW(popup_menu_1, MF_STRING, ID_QUIT_APPLICATION as usize, quit_application_str.as_ptr()) };

    // Append the popup menu to the menu bar under the text "Application"
    unsafe { AppendMenuW(menu_bar, MF_POPUP, popup_menu_1 as usize, application_top_level_menu_str.as_ptr()) };

    // Add the menu bar to the window
    if unsafe { SetMenu(hwnd, menu_bar) } == 0 { // <- window locks up here
        println!("SetMenu failed!");
        unsafe { DestroyMenu(popup_menu_1) };
        unsafe { DestroyMenu(menu_bar) };
        return;
    }

    // You can store the resources in your structure here if you want to
    // (ex. for pushing new items to the menu).
    unsafe { DestroyMenu(popup_menu_1) };
    unsafe { DestroyMenu(menu_bar) };
}
