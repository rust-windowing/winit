#![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
#![allow(non_upper_case_globals, non_snake_case, unused_imports)]
#![allow(missing_docs, clippy::all)]

use wayland_client::protocol::wl_surface;
use wayland_client::sys;
use wayland_client::{AnonymousObject, Attached, Main, Proxy, ProxyMap};
use wayland_commons::map::{Object, ObjectMetadata};
use wayland_commons::smallvec;
use wayland_commons::wire::{Argument, ArgumentType, Message, MessageDesc};
use wayland_commons::{Interface, MessageGroup};

include!(concat!(env!("OUT_DIR"), "/fractional_scale_v1.rs"));
