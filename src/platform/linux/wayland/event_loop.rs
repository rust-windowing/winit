use std::collections::VecDeque;
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicBool, Ordering};

use {WindowEvent as Event, ElementState, MouseButton, MouseScrollDelta, TouchPhase, ModifiersState,
     KeyboardInput, EventsLoopClosed, ControlFlow};

use super::{WaylandContext, WindowId, DeviceId};
use super::window::WindowStore;
use super::keyboard::init_keyboard;

use wayland_client::{StateToken, Proxy};
use wayland_client::protocol::{wl_seat, wl_pointer, wl_keyboard};

pub struct EventsLoopSink {
    buffer: VecDeque<::Event>
}

unsafe impl Send for EventsLoopSink { }

impl EventsLoopSink {
    pub fn new() -> EventsLoopSink{
        EventsLoopSink {
            buffer: VecDeque::new()
        }
    }

    pub fn send_event(&mut self, evt: ::WindowEvent, wid: WindowId) {
        let evt = ::Event::WindowEvent {
            event: evt,
            window_id: ::WindowId(::platform::WindowId::Wayland(wid))
        };
        self.buffer.push_back(evt);
    }

    pub fn send_raw_event(&mut self, evt: ::Event) {
        self.buffer.push_back(evt);
    }

    fn empty_with<F>(&mut self, callback: &mut F) where F: FnMut(::Event) {
        for evt in self.buffer.drain(..) {
            callback(evt)
        }
    }
}

pub struct EventsLoop {
    // the wayland context
    ctxt: Arc<WaylandContext>,
    // our sink, shared with some handlers, buffering the events
    sink: Arc<Mutex<EventsLoopSink>>,
    // Whether or not there is a pending `Awakened` event to be emitted.
    pending_wakeup: Arc<AtomicBool>,
    store: StateToken<WindowStore>
}

// A handle that can be sent across threads and used to wake up the `EventsLoop`.
//
// We should only try and wake up the `EventsLoop` if it still exists, so we hold Weak ptrs.
pub struct EventsLoopProxy {
    ctxt: Weak<WaylandContext>,
    pending_wakeup: Weak<AtomicBool>,
}

impl EventsLoopProxy {
    // Causes the `EventsLoop` to stop blocking on `run_forever` and emit an `Awakened` event.
    //
    // Returns `Err` if the associated `EventsLoop` no longer exists.
    pub fn wakeup(&self) -> Result<(), EventsLoopClosed> {
        let ctxt = self.ctxt.upgrade();
        let wakeup = self.pending_wakeup.upgrade();
        match (ctxt, wakeup) {
            (Some(ctxt), Some(wakeup)) => {
                // Update the `EventsLoop`'s `pending_wakeup` flag.
                wakeup.store(true, Ordering::Relaxed);
                // Cause the `EventsLoop` to break from `dispatch` if it is currently blocked.
                ctxt.display.sync();
                ctxt.display.flush()
                            .map_err(|_| EventsLoopClosed)?;
                Ok(())
            },
            _ => Err(EventsLoopClosed),
        }
    }
}

impl EventsLoop {
    pub fn new(mut ctxt: WaylandContext) -> EventsLoop {
        let sink = Arc::new(Mutex::new(EventsLoopSink::new()));

        let store = ctxt.evq.lock().unwrap().state().insert(WindowStore::new());

        let seat_idata = SeatIData {
            sink: sink.clone(),
            keyboard: None,
            pointer: None,
            windows_token: store.clone()
        };

        ctxt.init_seat(|evqh, seat| {
            evqh.register(seat, seat_implementation(), seat_idata);
        });

        EventsLoop {
            ctxt: Arc::new(ctxt),
            sink: sink,
            pending_wakeup: Arc::new(AtomicBool::new(false)),
            store: store
        }
    }

    #[inline]
    pub fn store(&self) -> StateToken<WindowStore> {
        self.store.clone()
    }

    #[inline]
    pub fn context(&self) -> &Arc<WaylandContext> {
        &self.ctxt
    }

    pub fn create_proxy(&self) -> EventsLoopProxy {
        EventsLoopProxy {
            ctxt: Arc::downgrade(&self.ctxt),
            pending_wakeup: Arc::downgrade(&self.pending_wakeup),
        }
    }

    pub fn poll_events<F>(&mut self, mut callback: F)
        where F: FnMut(::Event)
    {
        unimplemented!()
    }

    pub fn run_forever<F>(&mut self, mut callback: F)
        where F: FnMut(::Event) -> ControlFlow,
    {
        unimplemented!()
    }
}

/*
 * Wayland protocol implementations
 */

struct SeatIData {
    sink: Arc<Mutex<EventsLoopSink>>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    windows_token: StateToken<WindowStore>
}

fn seat_implementation() -> wl_seat::Implementation<SeatIData> {
    wl_seat::Implementation {
        name: |_, _, _, _| {},
        capabilities: |evqh, idata, seat, capabilities| {
            // create pointer if applicable
            if capabilities.contains(wl_seat::Capability::Pointer) && idata.pointer.is_none() {
                let pointer = seat.get_pointer().expect("Seat is not dead");
                let p_idata = PointerIData::new(&idata.sink, idata.windows_token.clone());
                evqh.register(&pointer, pointer_implementation(), p_idata);
                idata.pointer = Some(pointer);
            }
            // destroy pointer if applicable
            if !capabilities.contains(wl_seat::Capability::Pointer) {
                if let Some(pointer) = idata.pointer.take() {
                    pointer.release();
                }
            }
            // create keyboard if applicable
            if capabilities.contains(wl_seat::Capability::Keyboard) && idata.keyboard.is_none() {
                let kbd = seat.get_keyboard().expect("Seat is not dead");
                init_keyboard(evqh, &kbd, &idata.sink);
                idata.keyboard = Some(kbd);
            }
            // destroy keyboard if applicable
            if !capabilities.contains(wl_seat::Capability::Keyboard) {
                if let Some(kbd) = idata.keyboard.take() {
                    kbd.release();
                }
            }
            // TODO: Handle touch
        }
    }
}

struct PointerIData {
    sink: Arc<Mutex<EventsLoopSink>>,
    windows_token: StateToken<WindowStore>,
    mouse_focus: Option<WindowId>,
    axis_buffer: Option<(f32, f32)>,
    axis_discrete_buffer: Option<(i32, i32)>,
    axis_state: TouchPhase,
}

impl PointerIData {
    fn new(sink: &Arc<Mutex<EventsLoopSink>>, token: StateToken<WindowStore>)
        -> PointerIData
    {
        PointerIData {
            sink: sink.clone(),
            windows_token: token,
            mouse_focus: None,
            axis_buffer: None,
            axis_discrete_buffer: None,
            axis_state: TouchPhase::Cancelled
        }
    }
}

fn pointer_implementation() -> wl_pointer::Implementation<PointerIData> {
    wl_pointer::Implementation {
        enter: |evqh, idata, _, _, surface, x, y| {
            let wid = evqh.state().get(&idata.windows_token).find_wid(surface);
            if let Some(wid) = wid {
                idata.mouse_focus = Some(wid);
                let mut guard = idata.sink.lock().unwrap();
                guard.send_event(
                    Event::MouseEntered {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                    },
                    wid,
                );
                guard.send_event(
                    Event::MouseMoved {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        position: (x, y),
                    },
                    wid,
                );
            }
        },
        leave: |evqh, idata, _, _, surface| {
            idata.mouse_focus = None;
            let wid = evqh.state().get(&idata.windows_token).find_wid(surface);
            if let Some(wid) = wid {
                let mut guard = idata.sink.lock().unwrap();
                guard.send_event(
                    Event::MouseLeft {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                    },
                    wid,
                );
            }
        },
        motion: |_, idata, _, _, x, y| {
            if let Some(wid) = idata.mouse_focus {
                idata.sink.lock().unwrap().send_event(
                    Event::MouseMoved {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        position: (x, y)
                    },
                    wid
                );
            }
        },
        button: |_, idata, _, _, _, button, state| {
            if let Some(wid) = idata.mouse_focus {
                let state = match state {
                    wl_pointer::ButtonState::Pressed => ElementState::Pressed,
                    wl_pointer::ButtonState::Released => ElementState::Released
                };
                let button = match button {
                    0x110 => MouseButton::Left,
                    0x111 => MouseButton::Right,
                    0x112 => MouseButton::Middle,
                    // TODO figure out the translation ?
                    _ => return
                };
                idata.sink.lock().unwrap().send_event(
                    Event::MouseInput {
                        device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                        state: state,
                        button: button,
                    },
                    wid
                );
            }
        },
        axis: |_, idata, pointer, _, axis, value| {
            if let Some(wid) = idata.mouse_focus {
                if pointer.version() < 5 {
                    let (mut x, mut y) = (0.0, 0.0);
                    // old seat compatibility
                    match axis {
                        // wayland vertical sign convention is the inverse of winit
                        wl_pointer::Axis::VerticalScroll => y -= value as f32,
                        wl_pointer::Axis::HorizontalScroll => x += value as f32
                    }
                    idata.sink.lock().unwrap().send_event(
                        Event::MouseWheel {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            delta: MouseScrollDelta::PixelDelta(x as f32, y as f32),
                            phase: TouchPhase::Moved,
                        },
                        wid
                    );
                } else {
                    let (mut x, mut y) = idata.axis_buffer.unwrap_or((0.0, 0.0));
                    match axis {
                        // wayland vertical sign convention is the inverse of winit
                        wl_pointer::Axis::VerticalScroll => y -= value as f32,
                        wl_pointer::Axis::HorizontalScroll => x += value as f32
                    }
                    idata.axis_buffer = Some((x,y));
                    idata.axis_state = match idata.axis_state {
                        TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                        _ => TouchPhase::Started
                    }
                }
            }
        },
        frame: |_, idata, _| {
            let axis_buffer = idata.axis_buffer.take();
            let axis_discrete_buffer = idata.axis_discrete_buffer.take();
            if let Some(wid) = idata.mouse_focus {
                if let Some((x, y)) = axis_discrete_buffer {
                    idata.sink.lock().unwrap().send_event(
                        Event::MouseWheel {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            delta: MouseScrollDelta::LineDelta(x as f32, y as f32),
                            phase: idata.axis_state,
                        },
                        wid
                    );
                } else if let Some((x, y)) = axis_buffer {
                    idata.sink.lock().unwrap().send_event(
                        Event::MouseWheel {
                            device_id: ::DeviceId(::platform::DeviceId::Wayland(DeviceId)),
                            delta: MouseScrollDelta::PixelDelta(x as f32, y as f32),
                            phase: idata.axis_state,
                        },
                        wid
                    );
                }
            }
        },
        axis_source: |_, _, _, _| {},
        axis_stop: |_, idata, _, _, _| {
            idata.axis_state = TouchPhase::Ended;
        },
        axis_discrete: |_, idata, _, axis, discrete| {
            let (mut x, mut y) = idata.axis_discrete_buffer.unwrap_or((0,0));
            match axis {
                // wayland vertical sign convention is the inverse of winit
                wl_pointer::Axis::VerticalScroll => y -= discrete,
                wl_pointer::Axis::HorizontalScroll => x += discrete
            }
            idata.axis_discrete_buffer = Some((x,y));
            idata.axis_state = match idata.axis_state {
                TouchPhase::Started | TouchPhase::Moved => TouchPhase::Moved,
                _ => TouchPhase::Started
            }
        },
    }
}
