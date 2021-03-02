use std::collections::HashMap;
use winit::{
   event::Event,
   event_loop::{ControlFlow, EventLoop},
   window::WindowBuilder,
   platform::unix::{WindowBuilderExtUnix, XWindowType, XWindowStrut},
   dpi::{PhysicalSize, Position, PhysicalPosition}
};

fn main() {
   let event_loop = EventLoop::new();
   let mut desktop_shells = HashMap::new();

   // Template Window
   let window = |wtype, on_top, with_strut: Option<Vec<XWindowStrut>>| {
      let mut win_builder = WindowBuilder::new()
         .with_decorations(false)
         .with_resizable(false)
         .with_always_on_top(on_top)
         .with_x11_window_type(wtype);
      if let Some(with_strut) = with_strut {
         win_builder = win_builder.with_x11_window_strut(with_strut);
      }

      win_builder.build(&event_loop).unwrap()
   };

   let desktop = window(vec![XWindowType::Desktop], false, None);
   let primary_monitor = desktop.primary_monitor().unwrap();
   desktop.set_inner_size(primary_monitor.size());
   desktop.set_outer_position(primary_monitor.position());
   desktop_shells.insert(desktop.id(), &desktop);

   let toolbar_height: u32 = 30;
   let toolbar = window(vec![XWindowType::Toolbar], true, Some(vec![XWindowStrut::Strut([0, 0, toolbar_height as u64, 0])]));
   toolbar.set_inner_size(PhysicalSize::new(primary_monitor.size().width, toolbar_height));
   toolbar.set_outer_position(Position::Physical(primary_monitor.position()));
   desktop_shells.insert(toolbar.id(), &toolbar);

   let dock_height: u32 = 50;
   let dock = window(vec![XWindowType::Dock], true, Some(vec![XWindowStrut::Strut([0, 0, 0, dock_height as u64])]));
   dock.set_inner_size(PhysicalSize::new(primary_monitor.size().width, dock_height));
   let bottom_at = primary_monitor.size().height - dock_height;
   dock.set_outer_position(Position::Physical(PhysicalPosition::new(0, bottom_at as i32)));
   desktop_shells.insert(dock.id(), &dock);

   event_loop.run(move |event, _, control_flow| {
      *control_flow = ControlFlow::Wait;

      match event {
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