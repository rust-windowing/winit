//! Windows mirroring probe without a GUI toolkit. Space toggles WS_EX_LAYOUTRTL.
//! Green marks the actual cursor; red marks winit's reported position.
//! GDI drawing explicitly uses LTR coordinates, as an unmirrored renderer would.

#![allow(clippy::disallowed_macros)] // This diagnostic example only runs on Windows.

#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
fn main() {
    windows::run();
}

#[cfg(target_os = "windows")]
mod windows {
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    use dpi::PhysicalPosition;
    use rwh_06::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    use winit::application::ApplicationHandler;
    use winit::event::{ElementState, WindowEvent};
    use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
    use winit::keyboard::Key;
    use winit::window::{Window, WindowAttributes, WindowId};

    #[derive(Default)]
    struct App {
        window: Option<Box<dyn Window>>,
        reported: PhysicalPosition<f64>,
        step: usize,
        next: Option<Instant>,
        expected: VecDeque<PhysicalPosition<f64>>,
        verified: usize,
        saved_cursor: Option<POINT>,
    }

    fn hwnd(window: &dyn Window) -> HWND {
        let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() else {
            unreachable!()
        };
        handle.hwnd.get() as HWND
    }

    fn toggle(window: &dyn Window) {
        unsafe {
            let hwnd = hwnd(window);
            SetWindowLongW(
                hwnd,
                GWL_EXSTYLE,
                GetWindowLongW(hwnd, GWL_EXSTYLE) ^ WS_EX_LAYOUTRTL as i32,
            );
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
        window.request_redraw();
    }

    // Map a RECT so Windows normalizes its edges even in a mirrored window.
    fn screen_rect(hwnd: HWND) -> RECT {
        unsafe {
            let mut rect = RECT::default();
            assert_ne!(GetClientRect(hwnd, &mut rect), 0);
            MapWindowPoints(hwnd, std::ptr::null_mut(), &mut rect as *mut RECT as *mut POINT, 2);
            rect
        }
    }

    impl Drop for App {
        fn drop(&mut self) {
            if let Some(cursor) = self.saved_cursor {
                unsafe {
                    SetCursorPos(cursor.x, cursor.y);
                }
            }
        }
    }

    impl ApplicationHandler for App {
        fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
            self.window = Some(
                event_loop
                    .create_window(
                        WindowAttributes::default()
                            .with_title("RTL probe: Space toggles; green = OS cursor, red = winit")
                            .with_surface_size(dpi::PhysicalSize::new(640, 360)),
                    )
                    .unwrap(),
            );
            if std::env::args().any(|arg| arg == "--verify") {
                let mut cursor = POINT::default();
                unsafe {
                    GetCursorPos(&mut cursor);
                }
                self.saved_cursor = Some(cursor);
                self.next = Some(Instant::now() + Duration::from_millis(300));
            }
        }

        fn window_event(
            &mut self,
            event_loop: &dyn ActiveEventLoop,
            _: WindowId,
            event: WindowEvent,
        ) {
            let window = self.window.as_ref().unwrap();
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed =>
                {
                    if event.logical_key == Key::Character(" ".into()) {
                        toggle(&**window);
                    }
                },
                WindowEvent::PointerMoved { position, .. }
                | WindowEvent::PointerButton { position, .. } => {
                    if let Some(expected) = self.expected.pop_front() {
                        assert_eq!(
                            position, expected,
                            "injected native mouse coordinates must match the rendered surface"
                        );
                        self.verified += 1;
                    }
                    self.reported = position;
                    let hwnd = hwnd(&**window);
                    let rect = screen_rect(hwnd);
                    let mut cursor = POINT::default();
                    unsafe {
                        GetCursorPos(&mut cursor);
                    }
                    println!(
                        "rtl={} width={} actual=({}, {}) reported=({}, {})",
                        unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } & WS_EX_LAYOUTRTL as i32 != 0,
                        rect.right - rect.left,
                        cursor.x - rect.left,
                        cursor.y - rect.top,
                        position.x,
                        position.y
                    );
                    window.request_redraw();
                },
                WindowEvent::RedrawRequested => unsafe {
                    let hwnd = hwnd(&**window);
                    let dc = GetDC(hwnd);
                    let layout = SetLayout(dc, 0);
                    let mut rect = RECT::default();
                    GetClientRect(hwnd, &mut rect);
                    let background = CreateSolidBrush(0x00202020);
                    FillRect(dc, &rect, background);
                    DeleteObject(background);
                    let mut cursor = POINT::default();
                    GetCursorPos(&mut cursor);
                    let origin = screen_rect(hwnd);
                    let text: Vec<u16> = format!(
                        "OS: ({}, {})  winit: ({:.1}, {:.1})  [Space: toggle RTL]",
                        cursor.x - origin.left,
                        cursor.y - origin.top,
                        self.reported.x,
                        self.reported.y
                    )
                    .encode_utf16()
                    .collect();
                    SetTextColor(dc, 0x00ffffff);
                    SetBkMode(dc, TRANSPARENT as i32);
                    TextOutW(dc, 12, 12, text.as_ptr(), text.len() as i32);
                    for (x, y, color) in [
                        (self.reported.x as i32, self.reported.y as i32, 0x000000ff),
                        (cursor.x - origin.left, cursor.y - origin.top, 0x0000ff00),
                    ] {
                        let brush = CreateSolidBrush(color);
                        FillRect(
                            dc,
                            &RECT { left: x - 4, top: y - 4, right: x + 5, bottom: y + 5 },
                            brush,
                        );
                        DeleteObject(brush);
                    }
                    SetLayout(dc, layout);
                    ReleaseDC(hwnd, dc);
                },
                _ => (),
            }
        }

        fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
            let Some(next) = self.next else {
                return;
            };
            if Instant::now() >= next {
                assert_eq!(
                    self.verified,
                    self.step * 10,
                    "all injected events must be delivered before the next step"
                );
                let window = self.window.as_ref().unwrap();
                if self.step == 6 {
                    assert_eq!(self.verified, 60);
                    if let Some(cursor) = self.saved_cursor {
                        unsafe {
                            SetCursorPos(cursor.x, cursor.y);
                        }
                    }
                    println!("PASS: 60 injected move/drag/button events across LTR -> RTL -> LTR");
                    self.next = None;
                    event_loop.exit();
                    return;
                }
                if self.step == 2 || self.step == 4 {
                    toggle(&**window);
                }
                if self.step == 3 {
                    window.set_resizable(false);
                    window.set_resizable(true);
                    let _ = window.request_surface_size(dpi::PhysicalSize::new(801, 420).into());
                    assert_ne!(
                        unsafe { GetWindowLongW(hwnd(&**window), GWL_EXSTYLE) }
                            & WS_EX_LAYOUTRTL as i32,
                        0,
                        "unrelated winit attributes must preserve native mirroring"
                    );
                }
                unsafe {
                    SetForegroundWindow(hwnd(&**window));
                    BringWindowToTop(hwnd(&**window));
                }
                let rect = screen_rect(hwnd(&**window));
                unsafe {
                    let mut titlebar = TITLEBARINFOEX {
                        cbSize: std::mem::size_of::<TITLEBARINFOEX>() as u32,
                        ..Default::default()
                    };
                    SendMessageW(
                        hwnd(&**window),
                        WM_GETTITLEBARINFOEX,
                        0,
                        &mut titlebar as *mut _ as isize,
                    );
                    let rtl =
                        GetWindowLongW(hwnd(&**window), GWL_EXSTYLE) & WS_EX_LAYOUTRTL as i32 != 0;
                    for (index, name) in [(2, "minimize"), (3, "maximize"), (5, "close")] {
                        let button = titlebar.rgrect[index];
                        println!(
                            "native {name} button screen rect=({}, {}, {}, {})",
                            button.left, button.top, button.right, button.bottom
                        );
                        assert!(
                            button.right > button.left,
                            "native button geometry must be available"
                        );
                        if rtl {
                            assert!(button.right < (rect.left + rect.right) / 2);
                        } else {
                            assert!(button.left > (rect.left + rect.right) / 2);
                        }
                    }
                }
                let x = if self.step % 2 == 0 { 80 } else { 520 };
                unsafe {
                    assert_ne!(SetCursorPos(rect.left + x, rect.top + 100), 0);
                    println!(
                        "step={} client screen rect=({}, {}, {}, {})",
                        self.step, rect.left, rect.top, rect.right, rect.bottom
                    );
                }
                unsafe {
                    let hwnd = hwnd(&**window);
                    let mut mapped = POINT { x: rect.left + x, y: rect.top + 100 };
                    let mut legacy = mapped;
                    ScreenToClient(hwnd, &mut legacy);
                    MapWindowPoints(std::ptr::null_mut(), hwnd, &mut mapped, 1);
                    let mut origin = POINT::default();
                    ClientToScreen(hwnd, &mut origin);
                    println!(
                        "native=({}, {}) ScreenToClient=({}, {}) ClientToScreen(0)=({}, {})",
                        mapped.x, mapped.y, legacy.x, legacy.y, origin.x, origin.y
                    );
                    let packed = ((mapped.y as u32 & 0xffff) << 16) | (mapped.x as u32 & 0xffff);
                    self.expected.extend(
                        [x, x, x + 20, x + 20, x, x, x, x, x, x]
                            .map(|x| PhysicalPosition::new(x as f64, 100.)),
                    );
                    SendMessageW(hwnd, WM_MOUSEMOVE, 0, packed as isize);
                    SendMessageW(hwnd, WM_LBUTTONDOWN, 1, packed as isize);
                    let mut dragged = POINT { x: rect.left + x + 20, y: rect.top + 100 };
                    MapWindowPoints(std::ptr::null_mut(), hwnd, &mut dragged, 1);
                    let drag_packed =
                        ((dragged.y as u32 & 0xffff) << 16) | (dragged.x as u32 & 0xffff);
                    SendMessageW(hwnd, WM_MOUSEMOVE, 1, drag_packed as isize);
                    SendMessageW(hwnd, WM_LBUTTONUP, 0, drag_packed as isize);
                    for (down, up, wparam) in [
                        (WM_RBUTTONDOWN, WM_RBUTTONUP, 0),
                        (WM_MBUTTONDOWN, WM_MBUTTONUP, 0),
                        (WM_XBUTTONDOWN, WM_XBUTTONUP, 1 << 16),
                    ] {
                        SendMessageW(hwnd, down, wparam, packed as isize);
                        SendMessageW(hwnd, up, wparam, packed as isize);
                    }
                }
                window.set_cursor_position(PhysicalPosition::new(x, 100).into()).unwrap();
                let mut warped = POINT::default();
                unsafe {
                    GetCursorPos(&mut warped);
                }
                assert_eq!((warped.x, warped.y), (rect.left + x, rect.top + 100));
                self.step += 1;
                self.next = Some(Instant::now() + Duration::from_millis(300));
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next.unwrap()));
        }
    }

    pub(super) fn run() {
        EventLoop::new().unwrap().run_app(App::default()).unwrap();
    }
}
