use winapi::um::winuser;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::windows::{EventLoopWindowTargetExtWindows, WindowExtWindows},
    window::WindowBuilder,
};

#[derive(Debug, Clone, Copy)]
enum ModalDialogEvent {
    CloseWindow,
}

fn main() {
    simple_logger::init().unwrap();

    let event_loop = EventLoop::<ModalDialogEvent>::with_user_event();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Your faithful window")
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, window_target, control_flow| {
        *control_flow = ControlFlow::Wait;
        println!("{:?}", event);

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            }
            | Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    },
                ..
            } => {
                let hwnd = window.hwnd();
                let proxy = proxy.clone();
                window_target.schedule_modal_fn(move || unsafe {
                    println!("\n\t\tstart modal loop\n");

                    let msg_box_id = winuser::MessageBoxA(
                        hwnd as _,
                        "Are you sure you want close the window?\0".as_ptr() as *const _,
                        "Confirm Close\0".as_ptr() as *const _,
                        winuser::MB_ICONEXCLAMATION | winuser::MB_YESNO,
                    );

                    println!("\n\t\tend modal loop\n");

                    if msg_box_id == winuser::IDYES {
                        proxy.send_event(ModalDialogEvent::CloseWindow).unwrap();
                    }
                });
            }
            Event::UserEvent(ModalDialogEvent::CloseWindow) => {
                *control_flow = ControlFlow::Exit;
            }
            _ => (),
        }
    });
}
