use simple_logger::SimpleLogger;
use winit::{
    dpi::LogicalSize,
    event::{
        ElementState, Event, KeyboardInput, MouseButton, StartCause, VirtualKeyCode, WindowEvent,
    },
    event_loop::{ControlFlow, EventLoop},
    window::{CursorIcon, ResizeDirection, WindowBuilder},
};

#[derive(PartialEq, Eq, Clone, Copy)]
enum CursorLocation {
    Caption,
    Top,
    Bottom,
    Right,
    Left,
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
    Default,
}

impl CursorLocation {
    // Conversion to appropriate cursor Icon
    fn get_cursor_icon(&self) -> CursorIcon {
        match self {
            CursorLocation::Top | CursorLocation::Bottom => CursorIcon::NsResize,
            CursorLocation::Right | CursorLocation::Left => CursorIcon::EwResize,
            CursorLocation::TopRight | CursorLocation::BottomLeft => CursorIcon::NeswResize,
            CursorLocation::TopLeft | CursorLocation::BottomRight => CursorIcon::NwseResize,
            _ => CursorIcon::Arrow,
        }
    }

    // Conversion to a resize direction
    fn to_resize_direction(&self) -> Option<ResizeDirection> {
        match self {
            CursorLocation::Top => Some(ResizeDirection::Top),
            CursorLocation::Bottom => Some(ResizeDirection::Bottom),
            CursorLocation::Right => Some(ResizeDirection::Right),
            CursorLocation::Left => Some(ResizeDirection::Left),
            CursorLocation::TopRight => Some(ResizeDirection::TopRight),
            CursorLocation::TopLeft => Some(ResizeDirection::TopLeft),
            CursorLocation::BottomRight => Some(ResizeDirection::BottomRight),
            CursorLocation::BottomLeft => Some(ResizeDirection::BottomLeft),
            _ => None,
        }
    }

    // Intersects X locations and Y locations
    // Assumes that the x_location will only be Left, Right or Default
    // Assumes that the y_location will only be Top, Caption, Bottom or Default
    fn intersect(x_location: Self, y_location: Self) -> Self {
        match (x_location, y_location) {
            (CursorLocation::Default, _) => y_location,
            (_, CursorLocation::Default) => x_location,
            (CursorLocation::Left, CursorLocation::Top) => CursorLocation::TopLeft,
            (CursorLocation::Left, CursorLocation::Bottom) => CursorLocation::BottomLeft,
            (CursorLocation::Right, CursorLocation::Top) => CursorLocation::TopRight,
            (CursorLocation::Right, CursorLocation::Bottom) => CursorLocation::BottomRight,
            _ => CursorLocation::Default,
        }
    }
}

const BORDER: f64 = 5.;
const CAPTIONHEIGHT: f64 = 20.; // Titlebar
const INIT_WIDTH: f64 = 400.;
const INIT_HEIGHT: f64 = 200.;

fn main() {
    SimpleLogger::new().init().unwrap();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_min_inner_size(Some(LogicalSize::new(INIT_WIDTH, INIT_HEIGHT)));

    let mut border = false;
    window.set_decorations(border);

    let mut cursor_location = CursorLocation::Caption;
    let mut x_border = INIT_WIDTH - BORDER;
    let mut y_border = INIT_HEIGHT - BORDER;

    event_loop.run(move |event, _, control_flow| match event {
        Event::NewEvents(StartCause::Init) => {
            eprintln!(
                "Press 'B' to toggle borderless \nThe top of the screen functions as a titlebar"
            )
        }
        Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
            WindowEvent::CursorMoved { position, .. } => {
                // Test for X
                let x_location = if position.x < BORDER {
                    CursorLocation::Left
                } else if position.x > x_border {
                    CursorLocation::Right
                } else {
                    CursorLocation::Default
                };

                // Test for Y
                let y_location = if position.y < BORDER {
                    CursorLocation::Top
                } else if position.y < CAPTIONHEIGHT {
                    CursorLocation::Caption
                } else if position.y > y_border {
                    CursorLocation::Bottom
                } else {
                    CursorLocation::Default
                };

                let new_location = CursorLocation::intersect(x_location, y_location);

                if new_location != cursor_location {
                    cursor_location = new_location;
                    window.set_cursor_icon(cursor_location.get_cursor_icon())
                }
            }

            WindowEvent::Resized(new_size) => {
                x_border = new_size.width as f64 - BORDER;
                y_border = new_size.height as f64 - BORDER;
            }

            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(dir) = cursor_location.to_resize_direction() {
                    window.drag_resize_window(dir).unwrap()
                } else if cursor_location == CursorLocation::Caption {
                    window.drag_window().unwrap()
                }
            }
            WindowEvent::KeyboardInput {
                input:
                    KeyboardInput {
                        state: ElementState::Released,
                        virtual_keycode: Some(VirtualKeyCode::B),
                        ..
                    },
                ..
            } => {
                border = !border;
                window.set_decorations(border);
            }
            _ => (),
        },
        _ => (),
    });
}
