use std::collections::HashMap;
use winit::{
   event::{Event, WindowEvent, ModifiersState},
   event_loop::{ControlFlow, EventLoop},
   window::WindowBuilder,
   platform::unix::{WindowBuilderExtUnix, XWindowType, XWindowStrut},
   dpi::{PhysicalSize, Position, PhysicalPosition}
};

fn main() {
   let event_loop = EventLoop::new();
   let mut desktop_shells = HashMap::new();
   // Template Window
   let window = |wtype, size, on_top| WindowBuilder::new()
      .with_inner_size(size)
      .with_decorations(false)
      .with_resizable(false)
      .with_always_on_top(on_top)
      .with_x11_window_type(vec![wtype])
      .build(&event_loop).unwrap();

   let desktop = window(XWindowType::Desktop, PhysicalSize::new(1920, 1080), false);
   desktop.set_outer_position(Position::Physical(PhysicalPosition::new(0, 0)));
   desktop_shells.insert(desktop.id(), &desktop);

   let toolbar = WindowBuilder::new()
      .with_inner_size(PhysicalSize::new(1920, 30))
      .with_decorations(false)
      .with_resizable(false)
      .with_always_on_top(true)
      .with_x11_window_type(vec![XWindowType::Toolbar])
      .with_x11_window_strut(vec![XWindowStrut::Strut([0, 0, 30, 0])])
      .build(&event_loop).unwrap();
   toolbar.set_outer_position(Position::Physical(PhysicalPosition::new(0, 0)));
   desktop_shells.insert(toolbar.id(), &toolbar);

   let dock = WindowBuilder::new()
   .with_inner_size(PhysicalSize::new(1920, 30))
   .with_decorations(false)
   .with_resizable(false)
   .with_always_on_top(true)
   .with_x11_window_type(vec![XWindowType::Dock])
   .with_x11_window_strut(vec![XWindowStrut::Strut([0, 0, 0, 40])])
   .build(&event_loop).unwrap();
   dock.set_outer_position(Position::Physical(PhysicalPosition::new(0, 1040)));
   desktop_shells.insert(dock.id(), &dock);

   event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Wait;

      match event {
         // Event::WindowEvent { event, .. } => {
         //    match event {
         //       WindowEvent::CloseRequested =>  *control_flow = ControlFlow::Exit,
         //       WindowEvent::CursorMoved { position, ..} => cursor_position = position,
         //       WindowEvent::ModifiersChanged(modi) => modifiers = modi,
         //       WindowEvent::Resized(size) => {
         //          viewport = Viewport::with_physical_size(
         //             Size::new(size.width, size.height),
         //             desktop.scale_factor()
         //          )
         //       }
         //       _ => {}
         //    }
         // },
         Event::MainEventsCleared => {
            desktop.request_redraw();
            toolbar.request_redraw();
            dock.request_redraw();
         },
         Event::RedrawRequested(_) => {
            // Redraw the application.
            //
            // It's preferable for applications that do not render continuously to render in
            // this event rather than in MainEventsCleared, since rendering in here allows
            // the program to gracefully handle redraws requested by the OS.
         },
         _ => ()
      }
   });
}