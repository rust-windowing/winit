// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  cell::RefCell,
  io,
  rc::Rc,
  sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    mpsc::Sender,
  },
  collections::VecDeque,
};

use gdk::{Cursor, EventMask, WindowEdge, WindowExt, WindowState};
use gtk::{prelude::*, ApplicationWindow};
use gdk_pixbuf::Pixbuf;

use crate::{
  dpi::{PhysicalPosition, PhysicalSize, Position, Size},
  error::{ExternalError, NotSupportedError, OsError as RootOsError},
  icon::{Icon, BadIcon},
  monitor::MonitorHandle as RootMonitorHandle,
  window::{CursorIcon, Fullscreen, UserAttentionType, WindowAttributes},
};

use super::event_loop::EventLoopWindowTarget;
use super::monitor::MonitorHandle;

#[derive(Clone, Default)]
pub struct PlatformSpecificWindowBuilderAttributes{}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub(crate) u32);

impl WindowId {
  pub unsafe fn dummy() -> Self {
    WindowId(0)
  }
}

/// An icon used for the window titlebar, taskbar, etc.
#[derive(Debug, Clone)]
pub struct PlatformIcon {
  raw: Vec<u8>,
  width: i32,
  height: i32,
  row_stride: i32,
}

impl From<PlatformIcon> for Pixbuf {
  fn from(icon: PlatformIcon) -> Self {
    Pixbuf::from_mut_slice(
      icon.raw,
      gdk_pixbuf::Colorspace::Rgb,
      true,
      8,
      icon.width,
      icon.height,
      icon.row_stride,
    )
  }
}

impl PlatformIcon {
  /// Creates an `Icon` from 32bpp RGBA data.
  ///
  /// The length of `rgba` must be divisible by 4, and `width * height` must equal
  /// `rgba.len() / 4`. Otherwise, this will return a `BadIcon` error.
  pub fn from_rgba(rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, BadIcon> {
    let image = image::load_from_memory(&rgba)
      .map_err(|_| {
        BadIcon::OsError(io::Error::new(
          io::ErrorKind::InvalidData,
          "Invalid icon data!",
        ))
      })?
      .into_rgba8();
    let row_stride = image.sample_layout().height_stride;
    Ok(Self {
      raw: image.into_raw(),
      width: width as i32,
      height: height as i32,
      row_stride: row_stride as i32,
    })
  }
}

pub struct Window {
  /// Window id.
  pub(crate) window_id: WindowId,
  /// Gtk application window.
  pub(crate) window: gtk::ApplicationWindow,
  /// Window requests sender
  pub(crate) window_requests_tx: Sender<(WindowId, WindowRequest)>,
  scale_factor: Rc<AtomicI32>,
  position: Rc<(AtomicI32, AtomicI32)>,
  size: Rc<(AtomicI32, AtomicI32)>,
  maximized: Rc<AtomicBool>,
  fullscreen: RefCell<Option<Fullscreen>>,
}

impl Window {
  pub(crate) fn new<T>(
    event_loop_window_target: &EventLoopWindowTarget<T>,
    attributes: WindowAttributes,
    _pl_attribs: PlatformSpecificWindowBuilderAttributes,
  ) -> Result<Self, RootOsError> {
    let app = &event_loop_window_target.app;
    let window = gtk::ApplicationWindow::new(app);
    let window_id = WindowId(window.get_id());
    event_loop_window_target
      .windows
      .borrow_mut()
      .insert(window_id);

    // Set Width/Height & Resizable
    let win_scale_factor = window.get_scale_factor();
    let (width, height) = attributes
      .inner_size
      .map(|size| size.to_logical::<f64>(win_scale_factor as f64).into())
      .unwrap_or((800, 600));
    window.set_resizable(attributes.resizable);
    if attributes.resizable {
      window.set_default_size(width, height);
    } else {
      window.set_size_request(width, height);
    }

    // Set Min/Max Size
    let geom_mask = (if attributes.min_inner_size.is_some() {
      gdk::WindowHints::MIN_SIZE
    } else {
      gdk::WindowHints::empty()
    }) | (if attributes.max_inner_size.is_some() {
      gdk::WindowHints::MAX_SIZE
    } else {
      gdk::WindowHints::empty()
    });
    let (min_width, min_height) = attributes
      .min_inner_size
      .map(|size| size.to_logical::<f64>(win_scale_factor as f64).into())
      .unwrap_or_default();
    let (max_width, max_height) = attributes
      .max_inner_size
      .map(|size| size.to_logical::<f64>(win_scale_factor as f64).into())
      .unwrap_or_default();
    window.set_geometry_hints::<ApplicationWindow>(
      None,
      Some(&gdk::Geometry {
        min_width,
        min_height,
        max_width,
        max_height,
        base_width: 0,
        base_height: 0,
        width_inc: 0,
        height_inc: 0,
        min_aspect: 0f64,
        max_aspect: 0f64,
        win_gravity: gdk::Gravity::Center,
      }),
      geom_mask,
    );

    // Set Position
    if let Some(position) = attributes.position {
      let (x, y): (i32, i32) = position.to_physical::<i32>(win_scale_factor as f64).into();
      window.move_(x, y);
    }

    // Set Transparent
    if attributes.transparent {
      if let Some(screen) = window.get_screen() {
        if let Some(visual) = screen.get_rgba_visual() {
          window.set_visual(Some(&visual));
        }
      }

      window.connect_draw(|_, cr| {
        cr.set_source_rgba(0., 0., 0., 0.);
        cr.set_operator(cairo::Operator::Source);
        cr.paint();
        cr.set_operator(cairo::Operator::Over);
        Inhibit(false)
      });
      window.set_app_paintable(true);
    }

    // Rest attributes
    window.set_title(&attributes.title);
    if attributes.fullscreen.is_some() {
      window.fullscreen();
    }
    if attributes.maximized {
      window.maximize();
    }
    window.set_visible(attributes.visible);
    window.set_decorated(attributes.decorations);

    if !attributes.decorations && attributes.resizable {
      window.add_events(EventMask::POINTER_MOTION_MASK | EventMask::BUTTON_MOTION_MASK);

      window.connect_motion_notify_event(|window, event| {
        if let Some(gdk_window) = window.get_window() {
          let (cx, cy) = event.get_root();
          hit_test(&gdk_window, cx, cy);
        }
        Inhibit(false)
      });

      window.connect_button_press_event(|window, event| {
        if event.get_button() == 1 {
          if let Some(gdk_window) = window.get_window() {
            let (cx, cy) = event.get_root();
            let result = hit_test(&gdk_window, cx, cy);

            // this check is necessary, otherwise the window won't recieve the click properly when resize isn't needed
            if result != WindowEdge::__Unknown(8) {
              window.begin_resize_drag(result, 1, cx as i32, cy as i32, event.get_time());
            }
          }
        }
        Inhibit(false)
      });
    }

    window.set_keep_above(attributes.always_on_top);
    if let Some(icon) = attributes.window_icon {
      window.set_icon(Some(&icon.inner.into()));
    }

    window.show_all();

    let window_requests_tx = event_loop_window_target.window_requests_tx.clone();

    let w_pos = window.get_position();
    let position: Rc<(AtomicI32, AtomicI32)> = Rc::new((w_pos.0.into(), w_pos.1.into()));
    let position_clone = position.clone();

    let w_size = window.get_size();
    let size: Rc<(AtomicI32, AtomicI32)> = Rc::new((w_size.0.into(), w_size.1.into()));
    let size_clone = size.clone();

    window.connect_configure_event(move |_window, event| {
      let (x, y) = event.get_position();
      position_clone.0.store(x, Ordering::Release);
      position_clone.1.store(y, Ordering::Release);

      let (w, h) = event.get_size();
      size_clone.0.store(w as i32, Ordering::Release);
      size_clone.1.store(h as i32, Ordering::Release);

      false
    });

    let w_max = window.get_property_is_maximized();
    let maximized: Rc<AtomicBool> = Rc::new(w_max.into());
    let max_clone = maximized.clone();

    window.connect_window_state_event(move |_window, event| {
      let state = event.get_new_window_state();
      max_clone.store(state.contains(WindowState::MAXIMIZED), Ordering::Release);

      Inhibit(false)
    });

    let scale_factor: Rc<AtomicI32> = Rc::new(win_scale_factor.into());
    let scale_factor_clone = scale_factor.clone();
    window.connect_property_scale_factor_notify(move |window| {
      scale_factor_clone.store(window.get_scale_factor(), Ordering::Release);
    });

    if let Err(e) = window_requests_tx.send((window_id, WindowRequest::WireUpEvents)) {
      log::warn!("Fail to send wire up events request: {}", e);
    }

    window.queue_draw();
    Ok(Self {
      window_id,
      window,
      window_requests_tx,
      scale_factor,
      position,
      size,
      maximized,
      fullscreen: RefCell::new(attributes.fullscreen),
    })
  }

  pub fn id(&self) -> WindowId {
    self.window_id
  }

  pub fn scale_factor(&self) -> f64 {
    self.scale_factor.load(Ordering::Acquire) as f64
  }

  pub fn request_redraw(&self) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Redraw))
    {
      log::warn!("Fail to send redraw request: {}", e);
    }
  }

  pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
    let (x, y) = &*self.position;
    Ok(PhysicalPosition::new(
      x.load(Ordering::Acquire),
      y.load(Ordering::Acquire),
    ))
  }

  pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
    let (x, y) = &*self.position;
    Ok(PhysicalPosition::new(
      x.load(Ordering::Acquire),
      y.load(Ordering::Acquire),
    ))
  }

  pub fn set_outer_position<P: Into<Position>>(&self, position: P) {
    let (x, y): (i32, i32) = position
      .into()
      .to_physical::<i32>(self.scale_factor())
      .into();

    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Position((x, y))))
    {
      log::warn!("Fail to send position request: {}", e);
    }
  }

  pub fn inner_size(&self) -> PhysicalSize<u32> {
    let (width, height) = &*self.size;

    PhysicalSize::new(
      width.load(Ordering::Acquire) as u32,
      height.load(Ordering::Acquire) as u32,
    )
  }

  pub fn set_inner_size<S: Into<Size>>(&self, size: S) {
    let (width, height) = size.into().to_logical::<i32>(self.scale_factor()).into();

    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Size((width, height))))
    {
      log::warn!("Fail to send size request: {}", e);
    }
  }

  pub fn outer_size(&self) -> PhysicalSize<u32> {
    let (width, height) = &*self.size;

    PhysicalSize::new(
      width.load(Ordering::Acquire) as u32,
      height.load(Ordering::Acquire) as u32,
    )
  }

  pub fn set_min_inner_size<S: Into<Size>>(&self, min_size: Option<S>) {
    if let Some(size) = min_size {
      let (min_width, min_height) = size.into().to_logical::<i32>(self.scale_factor()).into();

      if let Err(e) = self.window_requests_tx.send((
        self.window_id,
        WindowRequest::MinSize((min_width, min_height)),
      )) {
        log::warn!("Fail to send min size request: {}", e);
      }
    }
  }
  pub fn set_max_inner_size<S: Into<Size>>(&self, max_size: Option<S>) {
    if let Some(size) = max_size {
      let (max_width, max_height) = size.into().to_logical::<i32>(self.scale_factor()).into();

      if let Err(e) = self.window_requests_tx.send((
        self.window_id,
        WindowRequest::MaxSize((max_width, max_height)),
      )) {
        log::warn!("Fail to send max size request: {}", e);
      }
    }
  }

  pub fn set_title(&self, title: &str) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Title(title.to_string())))
    {
      log::warn!("Fail to send title request: {}", e);
    }
  }

  pub fn set_visible(&self, visible: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Visible(visible)))
    {
      log::warn!("Fail to send visible request: {}", e);
    }
  }

  pub fn set_resizable(&self, resizable: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Resizable(resizable)))
    {
      log::warn!("Fail to send resizable request: {}", e);
    }
  }

  pub fn set_minimized(&self, minimized: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Minimized(minimized)))
    {
      log::warn!("Fail to send minimized request: {}", e);
    }
  }

  pub fn set_maximized(&self, maximized: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Maximized(maximized)))
    {
      log::warn!("Fail to send maximized request: {}", e);
    }
  }

  pub fn is_maximized(&self) -> bool {
    self.maximized.load(Ordering::Acquire)
  }

  pub fn drag_window(&self) -> Result<(), ExternalError> {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::DragWindow))
    {
      log::warn!("Fail to send drag window request: {}", e);
    }
    Ok(())
  }

  pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
    self.fullscreen.replace(fullscreen.clone());
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Fullscreen(fullscreen)))
    {
      log::warn!("Fail to send fullscreen request: {}", e);
    }
  }

  pub fn fullscreen(&self) -> Option<Fullscreen> {
    self.fullscreen.borrow().clone()
  }

  pub fn set_decorations(&self, decorations: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Decorations(decorations)))
    {
      log::warn!("Fail to send decorations request: {}", e);
    }
  }

  pub fn set_always_on_top(&self, always_on_top: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::AlwaysOnTop(always_on_top)))
    {
      log::warn!("Fail to send always on top request: {}", e);
    }
  }

  pub fn set_window_icon(&self, window_icon: Option<Icon>) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::WindowIcon(window_icon)))
    {
      log::warn!("Fail to send window icon request: {}", e);
    }
  }

  pub fn set_ime_position<P: Into<Position>>(&self, _position: P) {
    todo!()
  }

  pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::UserAttention(request_type)))
    {
      log::warn!("Fail to send user attention request: {}", e);
    }
  }

  pub fn set_cursor_icon(&self, cursor: CursorIcon) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::CursorIcon(Some(cursor))))
    {
      log::warn!("Fail to send cursor icon request: {}", e);
    }
  }

  pub fn set_cursor_position<P: Into<Position>>(&self, _position: P) -> Result<(), ExternalError> {
    todo!()
  }

    pub fn set_cursor_grab(&self, _grab: bool) -> Result<(), ExternalError> {
        todo!()
    }

  pub fn set_cursor_visible(&self, visible: bool) {
    let cursor = if visible {
      Some(CursorIcon::Default)
    } else {
      None
    };
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::CursorIcon(cursor)))
    {
      log::warn!("Fail to send cursor visibility request: {}", e);
    }
  }

  pub fn current_monitor(&self) -> Option<RootMonitorHandle> {
    todo!()
  }

  #[inline]
    pub fn available_monitors(&self) -> VecDeque<MonitorHandle> {
        todo!()
    }

  pub fn primary_monitor(&self) -> Option<RootMonitorHandle> {
    todo!()
  }

  pub fn raw_window_handle(&self) -> raw_window_handle::RawWindowHandle {
      todo!()
  }
}

// We need GtkWindow to initialize WebView, so we have to keep it in the field.
// It is called on any method.
unsafe impl Send for Window {}
unsafe impl Sync for Window {}

pub enum WindowRequest {
  Title(String),
  Position((i32, i32)),
  Size((i32, i32)),
  MinSize((i32, i32)),
  MaxSize((i32, i32)),
  Visible(bool),
  Resizable(bool),
  Minimized(bool),
  Maximized(bool),
  DragWindow,
  Fullscreen(Option<Fullscreen>),
  Decorations(bool),
  AlwaysOnTop(bool),
  WindowIcon(Option<Icon>),
  UserAttention(Option<UserAttentionType>),
  SkipTaskbar,
  CursorIcon(Option<CursorIcon>),
  WireUpEvents,
  Redraw,
}

fn hit_test(window: &gdk::Window, cx: f64, cy: f64) -> WindowEdge {
  let (left, top) = window.get_position();
  let (w, h) = (window.get_width(), window.get_height());
  let (right, bottom) = (left + w, top + h);
  let (cx, cy) = (cx as i32, cy as i32);

  let fake_border = 5; // change this to manipulate how far inside the window, the resize can happen

  let display = window.get_display();

  const LEFT: i32 = 0b00001;
  const RIGHT: i32 = 0b0010;
  const TOP: i32 = 0b0100;
  const BOTTOM: i32 = 0b1000;
  const TOPLEFT: i32 = TOP | LEFT;
  const TOPRIGHT: i32 = TOP | RIGHT;
  const BOTTOMLEFT: i32 = BOTTOM | LEFT;
  const BOTTOMRIGHT: i32 = BOTTOM | RIGHT;

  let result = (LEFT * (if cx < (left + fake_border) { 1 } else { 0 }))
    | (RIGHT * (if cx >= (right - fake_border) { 1 } else { 0 }))
    | (TOP * (if cy < (top + fake_border) { 1 } else { 0 }))
    | (BOTTOM * (if cy >= (bottom - fake_border) { 1 } else { 0 }));

  let edge = match result {
    LEFT => WindowEdge::West,
    TOP => WindowEdge::North,
    RIGHT => WindowEdge::East,
    BOTTOM => WindowEdge::South,
    TOPLEFT => WindowEdge::NorthWest,
    TOPRIGHT => WindowEdge::NorthEast,
    BOTTOMLEFT => WindowEdge::SouthWest,
    BOTTOMRIGHT => WindowEdge::SouthEast,
    // has to be bigger than 7. otherwise it will match the number with a variant of WindowEdge enum and we don't want to do that
    // also if the number ever change, makke sure to change it in the connect_button_press_event for window and webview
    _ => WindowEdge::__Unknown(8),
  };

  // FIXME: calling `window.begin_resize_drag` seems to revert the cursor back to normal style
  window.set_cursor(
    Cursor::from_name(
      &display,
      match edge {
        WindowEdge::North => "n-resize",
        WindowEdge::South => "s-resize",
        WindowEdge::East => "e-resize",
        WindowEdge::West => "w-resize",
        WindowEdge::NorthWest => "nw-resize",
        WindowEdge::NorthEast => "ne-resize",
        WindowEdge::SouthEast => "se-resize",
        WindowEdge::SouthWest => "sw-resize",
        _ => "default",
      },
    )
    .as_ref(),
  );

  edge
}
