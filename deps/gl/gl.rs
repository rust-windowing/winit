// Copyright 2013 The gl-rs developers. For a full listing of the authors,
// refer to the AUTHORS file at the top-level directory of this distribution.
// 
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
// 
//     http://www.apache.org/licenses/LICENSE-2.0
// 
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![crate_id = "github.com/bjz/gl-rs#gl:0.1"]
#![comment = "An OpenGL function loader."]
#![license = "ASL2"]
#![crate_type = "lib"]

#![feature(macro_rules)]
#![feature(globs)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case_functions)]
#![allow(unused_variable)]

extern crate libc;

use libc::*;
use std::mem;

use self::types::*;

pub mod types {
    use libc::*;
    
    // Common types from OpenGL 1.1
    pub type GLenum = c_uint;
    pub type GLboolean = c_uchar;
    pub type GLbitfield = c_uint;
    pub type GLvoid = c_void;
    pub type GLbyte = c_char;
    pub type GLshort = c_short;
    pub type GLint = c_int;
    pub type GLclampx = c_int;
    pub type GLubyte = c_uchar;
    pub type GLushort = c_ushort;
    pub type GLuint = c_uint;
    pub type GLsizei = c_int;
    pub type GLfloat = c_float;
    pub type GLclampf = c_float;
    pub type GLdouble = c_double;
    pub type GLclampd = c_double;
    pub type GLeglImageOES = *const c_void;
    pub type GLchar = c_char;
    pub type GLcharARB = c_char;
    
    #[cfg(target_os = "macos")]
    pub type GLhandleARB = *const c_void;
    #[cfg(not(target_os = "macos"))]
    pub type GLhandleARB = c_uint;
    
    pub type GLhalfARB = c_ushort;
    pub type GLhalf = c_ushort;
    
    // Must be 32 bits
    pub type GLfixed = GLint;
    
    pub type GLintptr = ptrdiff_t;
    pub type GLsizeiptr = ptrdiff_t;
    pub type GLint64 = i64;
    pub type GLuint64 = u64;
    pub type GLintptrARB = ptrdiff_t;
    pub type GLsizeiptrARB = ptrdiff_t;
    pub type GLint64EXT = i64;
    pub type GLuint64EXT = u64;
    
    pub struct __GLsync;
    pub type GLsync = *const __GLsync;
    
    // compatible with OpenCL cl_context
    pub struct _cl_context;
    pub struct _cl_event;
    
    pub type GLDEBUGPROC = extern "system" fn(source: GLenum, gltype: GLenum, id: GLuint, severity: GLenum, length: GLsizei, message: *const GLchar, userParam: *mut c_void);
    pub type GLDEBUGPROCARB = extern "system" fn(source: GLenum, gltype: GLenum, id: GLuint, severity: GLenum, length: GLsizei, message: *const GLchar, userParam: *mut c_void);
    pub type GLDEBUGPROCKHR = extern "system" fn(source: GLenum, gltype: GLenum, id: GLuint, severity: GLenum, length: GLsizei, message: *const GLchar, userParam: *mut c_void);
    
    // Vendor extension types
    pub type GLDEBUGPROCAMD = extern "system" fn(id: GLuint, category: GLenum, severity: GLenum, length: GLsizei, message: *const GLchar, userParam: *mut c_void);
    pub type GLhalfNV = c_ushort;
    pub type GLvdpauSurfaceNV = GLintptr;
}

pub static DEPTH_BUFFER_BIT: GLenum = 0x00000100;
pub static STENCIL_BUFFER_BIT: GLenum = 0x00000400;
pub static COLOR_BUFFER_BIT: GLenum = 0x00004000;
pub static CONTEXT_FLAG_FORWARD_COMPATIBLE_BIT: GLenum = 0x00000001;
pub static CONTEXT_FLAG_DEBUG_BIT: GLenum = 0x00000002;
pub static CONTEXT_CORE_PROFILE_BIT: GLenum = 0x00000001;
pub static CONTEXT_COMPATIBILITY_PROFILE_BIT: GLenum = 0x00000002;
pub static MAP_READ_BIT: GLenum = 0x0001;
pub static MAP_WRITE_BIT: GLenum = 0x0002;
pub static MAP_INVALIDATE_RANGE_BIT: GLenum = 0x0004;
pub static MAP_INVALIDATE_BUFFER_BIT: GLenum = 0x0008;
pub static MAP_FLUSH_EXPLICIT_BIT: GLenum = 0x0010;
pub static MAP_UNSYNCHRONIZED_BIT: GLenum = 0x0020;
pub static VERTEX_ATTRIB_ARRAY_BARRIER_BIT: GLenum = 0x00000001;
pub static ELEMENT_ARRAY_BARRIER_BIT: GLenum = 0x00000002;
pub static UNIFORM_BARRIER_BIT: GLenum = 0x00000004;
pub static TEXTURE_FETCH_BARRIER_BIT: GLenum = 0x00000008;
pub static SHADER_IMAGE_ACCESS_BARRIER_BIT: GLenum = 0x00000020;
pub static COMMAND_BARRIER_BIT: GLenum = 0x00000040;
pub static PIXEL_BUFFER_BARRIER_BIT: GLenum = 0x00000080;
pub static TEXTURE_UPDATE_BARRIER_BIT: GLenum = 0x00000100;
pub static BUFFER_UPDATE_BARRIER_BIT: GLenum = 0x00000200;
pub static FRAMEBUFFER_BARRIER_BIT: GLenum = 0x00000400;
pub static TRANSFORM_FEEDBACK_BARRIER_BIT: GLenum = 0x00000800;
pub static ATOMIC_COUNTER_BARRIER_BIT: GLenum = 0x00001000;
pub static SHADER_STORAGE_BARRIER_BIT: GLenum = 0x00002000;
pub static ALL_BARRIER_BITS: GLenum = 0xFFFFFFFF;
pub static SYNC_FLUSH_COMMANDS_BIT: GLenum = 0x00000001;
pub static VERTEX_SHADER_BIT: GLenum = 0x00000001;
pub static FRAGMENT_SHADER_BIT: GLenum = 0x00000002;
pub static GEOMETRY_SHADER_BIT: GLenum = 0x00000004;
pub static TESS_CONTROL_SHADER_BIT: GLenum = 0x00000008;
pub static TESS_EVALUATION_SHADER_BIT: GLenum = 0x00000010;
pub static ALL_SHADER_BITS: GLenum = 0xFFFFFFFF;
pub static FALSE: GLboolean = 0;
pub static NO_ERROR: GLenum = 0;
pub static ZERO: GLenum = 0;
pub static NONE: GLenum = 0;
pub static TRUE: GLboolean = 1;
pub static ONE: GLenum = 1;
pub static INVALID_INDEX: GLenum = 0xFFFFFFFF;
pub static TIMEOUT_IGNORED: GLuint64 = 0xFFFFFFFFFFFFFFFF;
pub static POINTS: GLenum = 0x0000;
pub static LINES: GLenum = 0x0001;
pub static LINE_LOOP: GLenum = 0x0002;
pub static LINE_STRIP: GLenum = 0x0003;
pub static TRIANGLES: GLenum = 0x0004;
pub static TRIANGLE_STRIP: GLenum = 0x0005;
pub static TRIANGLE_FAN: GLenum = 0x0006;
pub static LINES_ADJACENCY: GLenum = 0x000A;
pub static LINE_STRIP_ADJACENCY: GLenum = 0x000B;
pub static TRIANGLES_ADJACENCY: GLenum = 0x000C;
pub static TRIANGLE_STRIP_ADJACENCY: GLenum = 0x000D;
pub static PATCHES: GLenum = 0x000E;
pub static NEVER: GLenum = 0x0200;
pub static LESS: GLenum = 0x0201;
pub static EQUAL: GLenum = 0x0202;
pub static LEQUAL: GLenum = 0x0203;
pub static GREATER: GLenum = 0x0204;
pub static NOTEQUAL: GLenum = 0x0205;
pub static GEQUAL: GLenum = 0x0206;
pub static ALWAYS: GLenum = 0x0207;
pub static SRC_COLOR: GLenum = 0x0300;
pub static ONE_MINUS_SRC_COLOR: GLenum = 0x0301;
pub static SRC_ALPHA: GLenum = 0x0302;
pub static ONE_MINUS_SRC_ALPHA: GLenum = 0x0303;
pub static DST_ALPHA: GLenum = 0x0304;
pub static ONE_MINUS_DST_ALPHA: GLenum = 0x0305;
pub static DST_COLOR: GLenum = 0x0306;
pub static ONE_MINUS_DST_COLOR: GLenum = 0x0307;
pub static SRC_ALPHA_SATURATE: GLenum = 0x0308;
pub static FRONT_LEFT: GLenum = 0x0400;
pub static FRONT_RIGHT: GLenum = 0x0401;
pub static BACK_LEFT: GLenum = 0x0402;
pub static BACK_RIGHT: GLenum = 0x0403;
pub static FRONT: GLenum = 0x0404;
pub static BACK: GLenum = 0x0405;
pub static LEFT: GLenum = 0x0406;
pub static RIGHT: GLenum = 0x0407;
pub static FRONT_AND_BACK: GLenum = 0x0408;
pub static INVALID_ENUM: GLenum = 0x0500;
pub static INVALID_VALUE: GLenum = 0x0501;
pub static INVALID_OPERATION: GLenum = 0x0502;
pub static OUT_OF_MEMORY: GLenum = 0x0505;
pub static INVALID_FRAMEBUFFER_OPERATION: GLenum = 0x0506;
pub static CW: GLenum = 0x0900;
pub static CCW: GLenum = 0x0901;
pub static POINT_SIZE: GLenum = 0x0B11;
pub static POINT_SIZE_RANGE: GLenum = 0x0B12;
pub static SMOOTH_POINT_SIZE_RANGE: GLenum = 0x0B12;
pub static POINT_SIZE_GRANULARITY: GLenum = 0x0B13;
pub static SMOOTH_POINT_SIZE_GRANULARITY: GLenum = 0x0B13;
pub static LINE_SMOOTH: GLenum = 0x0B20;
pub static LINE_WIDTH: GLenum = 0x0B21;
pub static LINE_WIDTH_RANGE: GLenum = 0x0B22;
pub static SMOOTH_LINE_WIDTH_RANGE: GLenum = 0x0B22;
pub static LINE_WIDTH_GRANULARITY: GLenum = 0x0B23;
pub static SMOOTH_LINE_WIDTH_GRANULARITY: GLenum = 0x0B23;
pub static POLYGON_MODE: GLenum = 0x0B40;
pub static POLYGON_SMOOTH: GLenum = 0x0B41;
pub static CULL_FACE: GLenum = 0x0B44;
pub static CULL_FACE_MODE: GLenum = 0x0B45;
pub static FRONT_FACE: GLenum = 0x0B46;
pub static DEPTH_RANGE: GLenum = 0x0B70;
pub static DEPTH_TEST: GLenum = 0x0B71;
pub static DEPTH_WRITEMASK: GLenum = 0x0B72;
pub static DEPTH_CLEAR_VALUE: GLenum = 0x0B73;
pub static DEPTH_FUNC: GLenum = 0x0B74;
pub static STENCIL_TEST: GLenum = 0x0B90;
pub static STENCIL_CLEAR_VALUE: GLenum = 0x0B91;
pub static STENCIL_FUNC: GLenum = 0x0B92;
pub static STENCIL_VALUE_MASK: GLenum = 0x0B93;
pub static STENCIL_FAIL: GLenum = 0x0B94;
pub static STENCIL_PASS_DEPTH_FAIL: GLenum = 0x0B95;
pub static STENCIL_PASS_DEPTH_PASS: GLenum = 0x0B96;
pub static STENCIL_REF: GLenum = 0x0B97;
pub static STENCIL_WRITEMASK: GLenum = 0x0B98;
pub static VIEWPORT: GLenum = 0x0BA2;
pub static DITHER: GLenum = 0x0BD0;
pub static BLEND_DST: GLenum = 0x0BE0;
pub static BLEND_SRC: GLenum = 0x0BE1;
pub static BLEND: GLenum = 0x0BE2;
pub static LOGIC_OP_MODE: GLenum = 0x0BF0;
pub static COLOR_LOGIC_OP: GLenum = 0x0BF2;
pub static DRAW_BUFFER: GLenum = 0x0C01;
pub static READ_BUFFER: GLenum = 0x0C02;
pub static SCISSOR_BOX: GLenum = 0x0C10;
pub static SCISSOR_TEST: GLenum = 0x0C11;
pub static COLOR_CLEAR_VALUE: GLenum = 0x0C22;
pub static COLOR_WRITEMASK: GLenum = 0x0C23;
pub static DOUBLEBUFFER: GLenum = 0x0C32;
pub static STEREO: GLenum = 0x0C33;
pub static LINE_SMOOTH_HINT: GLenum = 0x0C52;
pub static POLYGON_SMOOTH_HINT: GLenum = 0x0C53;
pub static UNPACK_SWAP_BYTES: GLenum = 0x0CF0;
pub static UNPACK_LSB_FIRST: GLenum = 0x0CF1;
pub static UNPACK_ROW_LENGTH: GLenum = 0x0CF2;
pub static UNPACK_SKIP_ROWS: GLenum = 0x0CF3;
pub static UNPACK_SKIP_PIXELS: GLenum = 0x0CF4;
pub static UNPACK_ALIGNMENT: GLenum = 0x0CF5;
pub static PACK_SWAP_BYTES: GLenum = 0x0D00;
pub static PACK_LSB_FIRST: GLenum = 0x0D01;
pub static PACK_ROW_LENGTH: GLenum = 0x0D02;
pub static PACK_SKIP_ROWS: GLenum = 0x0D03;
pub static PACK_SKIP_PIXELS: GLenum = 0x0D04;
pub static PACK_ALIGNMENT: GLenum = 0x0D05;
pub static MAX_CLIP_DISTANCES: GLenum = 0x0D32;
pub static MAX_TEXTURE_SIZE: GLenum = 0x0D33;
pub static MAX_VIEWPORT_DIMS: GLenum = 0x0D3A;
pub static SUBPIXEL_BITS: GLenum = 0x0D50;
pub static TEXTURE_1D: GLenum = 0x0DE0;
pub static TEXTURE_2D: GLenum = 0x0DE1;
pub static TEXTURE_WIDTH: GLenum = 0x1000;
pub static TEXTURE_HEIGHT: GLenum = 0x1001;
pub static TEXTURE_INTERNAL_FORMAT: GLenum = 0x1003;
pub static TEXTURE_BORDER_COLOR: GLenum = 0x1004;
pub static DONT_CARE: GLenum = 0x1100;
pub static FASTEST: GLenum = 0x1101;
pub static NICEST: GLenum = 0x1102;
pub static BYTE: GLenum = 0x1400;
pub static UNSIGNED_BYTE: GLenum = 0x1401;
pub static SHORT: GLenum = 0x1402;
pub static UNSIGNED_SHORT: GLenum = 0x1403;
pub static INT: GLenum = 0x1404;
pub static UNSIGNED_INT: GLenum = 0x1405;
pub static FLOAT: GLenum = 0x1406;
pub static DOUBLE: GLenum = 0x140A;
pub static HALF_FLOAT: GLenum = 0x140B;
pub static FIXED: GLenum = 0x140C;
pub static CLEAR: GLenum = 0x1500;
pub static AND: GLenum = 0x1501;
pub static AND_REVERSE: GLenum = 0x1502;
pub static COPY: GLenum = 0x1503;
pub static AND_INVERTED: GLenum = 0x1504;
pub static NOOP: GLenum = 0x1505;
pub static XOR: GLenum = 0x1506;
pub static OR: GLenum = 0x1507;
pub static NOR: GLenum = 0x1508;
pub static EQUIV: GLenum = 0x1509;
pub static INVERT: GLenum = 0x150A;
pub static OR_REVERSE: GLenum = 0x150B;
pub static COPY_INVERTED: GLenum = 0x150C;
pub static OR_INVERTED: GLenum = 0x150D;
pub static NAND: GLenum = 0x150E;
pub static SET: GLenum = 0x150F;
pub static TEXTURE: GLenum = 0x1702;
pub static COLOR: GLenum = 0x1800;
pub static DEPTH: GLenum = 0x1801;
pub static STENCIL: GLenum = 0x1802;
pub static STENCIL_INDEX: GLenum = 0x1901;
pub static DEPTH_COMPONENT: GLenum = 0x1902;
pub static RED: GLenum = 0x1903;
pub static GREEN: GLenum = 0x1904;
pub static BLUE: GLenum = 0x1905;
pub static ALPHA: GLenum = 0x1906;
pub static RGB: GLenum = 0x1907;
pub static RGBA: GLenum = 0x1908;
pub static POINT: GLenum = 0x1B00;
pub static LINE: GLenum = 0x1B01;
pub static FILL: GLenum = 0x1B02;
pub static KEEP: GLenum = 0x1E00;
pub static REPLACE: GLenum = 0x1E01;
pub static INCR: GLenum = 0x1E02;
pub static DECR: GLenum = 0x1E03;
pub static VENDOR: GLenum = 0x1F00;
pub static RENDERER: GLenum = 0x1F01;
pub static VERSION: GLenum = 0x1F02;
pub static EXTENSIONS: GLenum = 0x1F03;
pub static NEAREST: GLenum = 0x2600;
pub static LINEAR: GLenum = 0x2601;
pub static NEAREST_MIPMAP_NEAREST: GLenum = 0x2700;
pub static LINEAR_MIPMAP_NEAREST: GLenum = 0x2701;
pub static NEAREST_MIPMAP_LINEAR: GLenum = 0x2702;
pub static LINEAR_MIPMAP_LINEAR: GLenum = 0x2703;
pub static TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub static TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub static TEXTURE_WRAP_S: GLenum = 0x2802;
pub static TEXTURE_WRAP_T: GLenum = 0x2803;
pub static REPEAT: GLenum = 0x2901;
pub static POLYGON_OFFSET_UNITS: GLenum = 0x2A00;
pub static POLYGON_OFFSET_POINT: GLenum = 0x2A01;
pub static POLYGON_OFFSET_LINE: GLenum = 0x2A02;
pub static R3_G3_B2: GLenum = 0x2A10;
pub static CLIP_DISTANCE0: GLenum = 0x3000;
pub static CLIP_DISTANCE1: GLenum = 0x3001;
pub static CLIP_DISTANCE2: GLenum = 0x3002;
pub static CLIP_DISTANCE3: GLenum = 0x3003;
pub static CLIP_DISTANCE4: GLenum = 0x3004;
pub static CLIP_DISTANCE5: GLenum = 0x3005;
pub static CLIP_DISTANCE6: GLenum = 0x3006;
pub static CLIP_DISTANCE7: GLenum = 0x3007;
pub static CONSTANT_COLOR: GLenum = 0x8001;
pub static ONE_MINUS_CONSTANT_COLOR: GLenum = 0x8002;
pub static CONSTANT_ALPHA: GLenum = 0x8003;
pub static ONE_MINUS_CONSTANT_ALPHA: GLenum = 0x8004;
pub static FUNC_ADD: GLenum = 0x8006;
pub static MIN: GLenum = 0x8007;
pub static MAX: GLenum = 0x8008;
pub static BLEND_EQUATION_RGB: GLenum = 0x8009;
pub static FUNC_SUBTRACT: GLenum = 0x800A;
pub static FUNC_REVERSE_SUBTRACT: GLenum = 0x800B;
pub static UNSIGNED_BYTE_3_3_2: GLenum = 0x8032;
pub static UNSIGNED_SHORT_4_4_4_4: GLenum = 0x8033;
pub static UNSIGNED_SHORT_5_5_5_1: GLenum = 0x8034;
pub static UNSIGNED_INT_8_8_8_8: GLenum = 0x8035;
pub static UNSIGNED_INT_10_10_10_2: GLenum = 0x8036;
pub static POLYGON_OFFSET_FILL: GLenum = 0x8037;
pub static POLYGON_OFFSET_FACTOR: GLenum = 0x8038;
pub static RGB4: GLenum = 0x804F;
pub static RGB5: GLenum = 0x8050;
pub static RGB8: GLenum = 0x8051;
pub static RGB10: GLenum = 0x8052;
pub static RGB12: GLenum = 0x8053;
pub static RGB16: GLenum = 0x8054;
pub static RGBA2: GLenum = 0x8055;
pub static RGBA4: GLenum = 0x8056;
pub static RGB5_A1: GLenum = 0x8057;
pub static RGBA8: GLenum = 0x8058;
pub static RGB10_A2: GLenum = 0x8059;
pub static RGBA12: GLenum = 0x805A;
pub static RGBA16: GLenum = 0x805B;
pub static TEXTURE_RED_SIZE: GLenum = 0x805C;
pub static TEXTURE_GREEN_SIZE: GLenum = 0x805D;
pub static TEXTURE_BLUE_SIZE: GLenum = 0x805E;
pub static TEXTURE_ALPHA_SIZE: GLenum = 0x805F;
pub static PROXY_TEXTURE_1D: GLenum = 0x8063;
pub static PROXY_TEXTURE_2D: GLenum = 0x8064;
pub static TEXTURE_BINDING_1D: GLenum = 0x8068;
pub static TEXTURE_BINDING_2D: GLenum = 0x8069;
pub static TEXTURE_BINDING_3D: GLenum = 0x806A;
pub static PACK_SKIP_IMAGES: GLenum = 0x806B;
pub static PACK_IMAGE_HEIGHT: GLenum = 0x806C;
pub static UNPACK_SKIP_IMAGES: GLenum = 0x806D;
pub static UNPACK_IMAGE_HEIGHT: GLenum = 0x806E;
pub static TEXTURE_3D: GLenum = 0x806F;
pub static PROXY_TEXTURE_3D: GLenum = 0x8070;
pub static TEXTURE_DEPTH: GLenum = 0x8071;
pub static TEXTURE_WRAP_R: GLenum = 0x8072;
pub static MAX_3D_TEXTURE_SIZE: GLenum = 0x8073;
pub static MULTISAMPLE: GLenum = 0x809D;
pub static SAMPLE_ALPHA_TO_COVERAGE: GLenum = 0x809E;
pub static SAMPLE_ALPHA_TO_ONE: GLenum = 0x809F;
pub static SAMPLE_COVERAGE: GLenum = 0x80A0;
pub static SAMPLE_BUFFERS: GLenum = 0x80A8;
pub static SAMPLES: GLenum = 0x80A9;
pub static SAMPLE_COVERAGE_VALUE: GLenum = 0x80AA;
pub static SAMPLE_COVERAGE_INVERT: GLenum = 0x80AB;
pub static BLEND_DST_RGB: GLenum = 0x80C8;
pub static BLEND_SRC_RGB: GLenum = 0x80C9;
pub static BLEND_DST_ALPHA: GLenum = 0x80CA;
pub static BLEND_SRC_ALPHA: GLenum = 0x80CB;
pub static BGR: GLenum = 0x80E0;
pub static BGRA: GLenum = 0x80E1;
pub static MAX_ELEMENTS_VERTICES: GLenum = 0x80E8;
pub static MAX_ELEMENTS_INDICES: GLenum = 0x80E9;
pub static POINT_FADE_THRESHOLD_SIZE: GLenum = 0x8128;
pub static CLAMP_TO_BORDER: GLenum = 0x812D;
pub static CLAMP_TO_EDGE: GLenum = 0x812F;
pub static TEXTURE_MIN_LOD: GLenum = 0x813A;
pub static TEXTURE_MAX_LOD: GLenum = 0x813B;
pub static TEXTURE_BASE_LEVEL: GLenum = 0x813C;
pub static TEXTURE_MAX_LEVEL: GLenum = 0x813D;
pub static DEPTH_COMPONENT16: GLenum = 0x81A5;
pub static DEPTH_COMPONENT24: GLenum = 0x81A6;
pub static DEPTH_COMPONENT32: GLenum = 0x81A7;
pub static FRAMEBUFFER_ATTACHMENT_COLOR_ENCODING: GLenum = 0x8210;
pub static FRAMEBUFFER_ATTACHMENT_COMPONENT_TYPE: GLenum = 0x8211;
pub static FRAMEBUFFER_ATTACHMENT_RED_SIZE: GLenum = 0x8212;
pub static FRAMEBUFFER_ATTACHMENT_GREEN_SIZE: GLenum = 0x8213;
pub static FRAMEBUFFER_ATTACHMENT_BLUE_SIZE: GLenum = 0x8214;
pub static FRAMEBUFFER_ATTACHMENT_ALPHA_SIZE: GLenum = 0x8215;
pub static FRAMEBUFFER_ATTACHMENT_DEPTH_SIZE: GLenum = 0x8216;
pub static FRAMEBUFFER_ATTACHMENT_STENCIL_SIZE: GLenum = 0x8217;
pub static FRAMEBUFFER_DEFAULT: GLenum = 0x8218;
pub static FRAMEBUFFER_UNDEFINED: GLenum = 0x8219;
pub static DEPTH_STENCIL_ATTACHMENT: GLenum = 0x821A;
pub static MAJOR_VERSION: GLenum = 0x821B;
pub static MINOR_VERSION: GLenum = 0x821C;
pub static NUM_EXTENSIONS: GLenum = 0x821D;
pub static CONTEXT_FLAGS: GLenum = 0x821E;
pub static INDEX: GLenum = 0x8222;
pub static COMPRESSED_RED: GLenum = 0x8225;
pub static COMPRESSED_RG: GLenum = 0x8226;
pub static RG: GLenum = 0x8227;
pub static RG_INTEGER: GLenum = 0x8228;
pub static R8: GLenum = 0x8229;
pub static R16: GLenum = 0x822A;
pub static RG8: GLenum = 0x822B;
pub static RG16: GLenum = 0x822C;
pub static R16F: GLenum = 0x822D;
pub static R32F: GLenum = 0x822E;
pub static RG16F: GLenum = 0x822F;
pub static RG32F: GLenum = 0x8230;
pub static R8I: GLenum = 0x8231;
pub static R8UI: GLenum = 0x8232;
pub static R16I: GLenum = 0x8233;
pub static R16UI: GLenum = 0x8234;
pub static R32I: GLenum = 0x8235;
pub static R32UI: GLenum = 0x8236;
pub static RG8I: GLenum = 0x8237;
pub static RG8UI: GLenum = 0x8238;
pub static RG16I: GLenum = 0x8239;
pub static RG16UI: GLenum = 0x823A;
pub static RG32I: GLenum = 0x823B;
pub static RG32UI: GLenum = 0x823C;
pub static DEBUG_OUTPUT_SYNCHRONOUS: GLenum = 0x8242;
pub static DEBUG_NEXT_LOGGED_MESSAGE_LENGTH: GLenum = 0x8243;
pub static DEBUG_CALLBACK_FUNCTION: GLenum = 0x8244;
pub static DEBUG_CALLBACK_USER_PARAM: GLenum = 0x8245;
pub static DEBUG_SOURCE_API: GLenum = 0x8246;
pub static DEBUG_SOURCE_WINDOW_SYSTEM: GLenum = 0x8247;
pub static DEBUG_SOURCE_SHADER_COMPILER: GLenum = 0x8248;
pub static DEBUG_SOURCE_THIRD_PARTY: GLenum = 0x8249;
pub static DEBUG_SOURCE_APPLICATION: GLenum = 0x824A;
pub static DEBUG_SOURCE_OTHER: GLenum = 0x824B;
pub static DEBUG_TYPE_ERROR: GLenum = 0x824C;
pub static DEBUG_TYPE_DEPRECATED_BEHAVIOR: GLenum = 0x824D;
pub static DEBUG_TYPE_UNDEFINED_BEHAVIOR: GLenum = 0x824E;
pub static DEBUG_TYPE_PORTABILITY: GLenum = 0x824F;
pub static DEBUG_TYPE_PERFORMANCE: GLenum = 0x8250;
pub static DEBUG_TYPE_OTHER: GLenum = 0x8251;
pub static PROGRAM_BINARY_RETRIEVABLE_HINT: GLenum = 0x8257;
pub static PROGRAM_SEPARABLE: GLenum = 0x8258;
pub static ACTIVE_PROGRAM: GLenum = 0x8259;
pub static PROGRAM_PIPELINE_BINDING: GLenum = 0x825A;
pub static MAX_VIEWPORTS: GLenum = 0x825B;
pub static VIEWPORT_SUBPIXEL_BITS: GLenum = 0x825C;
pub static VIEWPORT_BOUNDS_RANGE: GLenum = 0x825D;
pub static LAYER_PROVOKING_VERTEX: GLenum = 0x825E;
pub static VIEWPORT_INDEX_PROVOKING_VERTEX: GLenum = 0x825F;
pub static UNDEFINED_VERTEX: GLenum = 0x8260;
pub static MAX_COMPUTE_SHARED_MEMORY_SIZE: GLenum = 0x8262;
pub static MAX_COMPUTE_UNIFORM_COMPONENTS: GLenum = 0x8263;
pub static MAX_COMPUTE_ATOMIC_COUNTER_BUFFERS: GLenum = 0x8264;
pub static MAX_COMPUTE_ATOMIC_COUNTERS: GLenum = 0x8265;
pub static MAX_COMBINED_COMPUTE_UNIFORM_COMPONENTS: GLenum = 0x8266;
pub static COMPUTE_WORK_GROUP_SIZE: GLenum = 0x8267;
pub static DEBUG_TYPE_MARKER: GLenum = 0x8268;
pub static DEBUG_TYPE_PUSH_GROUP: GLenum = 0x8269;
pub static DEBUG_TYPE_POP_GROUP: GLenum = 0x826A;
pub static DEBUG_SEVERITY_NOTIFICATION: GLenum = 0x826B;
pub static MAX_DEBUG_GROUP_STACK_DEPTH: GLenum = 0x826C;
pub static DEBUG_GROUP_STACK_DEPTH: GLenum = 0x826D;
pub static MAX_UNIFORM_LOCATIONS: GLenum = 0x826E;
pub static INTERNALFORMAT_SUPPORTED: GLenum = 0x826F;
pub static INTERNALFORMAT_PREFERRED: GLenum = 0x8270;
pub static INTERNALFORMAT_RED_SIZE: GLenum = 0x8271;
pub static INTERNALFORMAT_GREEN_SIZE: GLenum = 0x8272;
pub static INTERNALFORMAT_BLUE_SIZE: GLenum = 0x8273;
pub static INTERNALFORMAT_ALPHA_SIZE: GLenum = 0x8274;
pub static INTERNALFORMAT_DEPTH_SIZE: GLenum = 0x8275;
pub static INTERNALFORMAT_STENCIL_SIZE: GLenum = 0x8276;
pub static INTERNALFORMAT_SHARED_SIZE: GLenum = 0x8277;
pub static INTERNALFORMAT_RED_TYPE: GLenum = 0x8278;
pub static INTERNALFORMAT_GREEN_TYPE: GLenum = 0x8279;
pub static INTERNALFORMAT_BLUE_TYPE: GLenum = 0x827A;
pub static INTERNALFORMAT_ALPHA_TYPE: GLenum = 0x827B;
pub static INTERNALFORMAT_DEPTH_TYPE: GLenum = 0x827C;
pub static INTERNALFORMAT_STENCIL_TYPE: GLenum = 0x827D;
pub static MAX_WIDTH: GLenum = 0x827E;
pub static MAX_HEIGHT: GLenum = 0x827F;
pub static MAX_DEPTH: GLenum = 0x8280;
pub static MAX_LAYERS: GLenum = 0x8281;
pub static MAX_COMBINED_DIMENSIONS: GLenum = 0x8282;
pub static COLOR_COMPONENTS: GLenum = 0x8283;
pub static DEPTH_COMPONENTS: GLenum = 0x8284;
pub static STENCIL_COMPONENTS: GLenum = 0x8285;
pub static COLOR_RENDERABLE: GLenum = 0x8286;
pub static DEPTH_RENDERABLE: GLenum = 0x8287;
pub static STENCIL_RENDERABLE: GLenum = 0x8288;
pub static FRAMEBUFFER_RENDERABLE: GLenum = 0x8289;
pub static FRAMEBUFFER_RENDERABLE_LAYERED: GLenum = 0x828A;
pub static FRAMEBUFFER_BLEND: GLenum = 0x828B;
pub static READ_PIXELS: GLenum = 0x828C;
pub static READ_PIXELS_FORMAT: GLenum = 0x828D;
pub static READ_PIXELS_TYPE: GLenum = 0x828E;
pub static TEXTURE_IMAGE_FORMAT: GLenum = 0x828F;
pub static TEXTURE_IMAGE_TYPE: GLenum = 0x8290;
pub static GET_TEXTURE_IMAGE_FORMAT: GLenum = 0x8291;
pub static GET_TEXTURE_IMAGE_TYPE: GLenum = 0x8292;
pub static MIPMAP: GLenum = 0x8293;
pub static MANUAL_GENERATE_MIPMAP: GLenum = 0x8294;
pub static AUTO_GENERATE_MIPMAP: GLenum = 0x8295;
pub static COLOR_ENCODING: GLenum = 0x8296;
pub static SRGB_READ: GLenum = 0x8297;
pub static SRGB_WRITE: GLenum = 0x8298;
pub static FILTER: GLenum = 0x829A;
pub static VERTEX_TEXTURE: GLenum = 0x829B;
pub static TESS_CONTROL_TEXTURE: GLenum = 0x829C;
pub static TESS_EVALUATION_TEXTURE: GLenum = 0x829D;
pub static GEOMETRY_TEXTURE: GLenum = 0x829E;
pub static FRAGMENT_TEXTURE: GLenum = 0x829F;
pub static COMPUTE_TEXTURE: GLenum = 0x82A0;
pub static TEXTURE_SHADOW: GLenum = 0x82A1;
pub static TEXTURE_GATHER: GLenum = 0x82A2;
pub static TEXTURE_GATHER_SHADOW: GLenum = 0x82A3;
pub static SHADER_IMAGE_LOAD: GLenum = 0x82A4;
pub static SHADER_IMAGE_STORE: GLenum = 0x82A5;
pub static SHADER_IMAGE_ATOMIC: GLenum = 0x82A6;
pub static IMAGE_TEXEL_SIZE: GLenum = 0x82A7;
pub static IMAGE_COMPATIBILITY_CLASS: GLenum = 0x82A8;
pub static IMAGE_PIXEL_FORMAT: GLenum = 0x82A9;
pub static IMAGE_PIXEL_TYPE: GLenum = 0x82AA;
pub static SIMULTANEOUS_TEXTURE_AND_DEPTH_TEST: GLenum = 0x82AC;
pub static SIMULTANEOUS_TEXTURE_AND_STENCIL_TEST: GLenum = 0x82AD;
pub static SIMULTANEOUS_TEXTURE_AND_DEPTH_WRITE: GLenum = 0x82AE;
pub static SIMULTANEOUS_TEXTURE_AND_STENCIL_WRITE: GLenum = 0x82AF;
pub static TEXTURE_COMPRESSED_BLOCK_WIDTH: GLenum = 0x82B1;
pub static TEXTURE_COMPRESSED_BLOCK_HEIGHT: GLenum = 0x82B2;
pub static TEXTURE_COMPRESSED_BLOCK_SIZE: GLenum = 0x82B3;
pub static CLEAR_BUFFER: GLenum = 0x82B4;
pub static TEXTURE_VIEW: GLenum = 0x82B5;
pub static VIEW_COMPATIBILITY_CLASS: GLenum = 0x82B6;
pub static FULL_SUPPORT: GLenum = 0x82B7;
pub static CAVEAT_SUPPORT: GLenum = 0x82B8;
pub static IMAGE_CLASS_4_X_32: GLenum = 0x82B9;
pub static IMAGE_CLASS_2_X_32: GLenum = 0x82BA;
pub static IMAGE_CLASS_1_X_32: GLenum = 0x82BB;
pub static IMAGE_CLASS_4_X_16: GLenum = 0x82BC;
pub static IMAGE_CLASS_2_X_16: GLenum = 0x82BD;
pub static IMAGE_CLASS_1_X_16: GLenum = 0x82BE;
pub static IMAGE_CLASS_4_X_8: GLenum = 0x82BF;
pub static IMAGE_CLASS_2_X_8: GLenum = 0x82C0;
pub static IMAGE_CLASS_1_X_8: GLenum = 0x82C1;
pub static IMAGE_CLASS_11_11_10: GLenum = 0x82C2;
pub static IMAGE_CLASS_10_10_10_2: GLenum = 0x82C3;
pub static VIEW_CLASS_128_BITS: GLenum = 0x82C4;
pub static VIEW_CLASS_96_BITS: GLenum = 0x82C5;
pub static VIEW_CLASS_64_BITS: GLenum = 0x82C6;
pub static VIEW_CLASS_48_BITS: GLenum = 0x82C7;
pub static VIEW_CLASS_32_BITS: GLenum = 0x82C8;
pub static VIEW_CLASS_24_BITS: GLenum = 0x82C9;
pub static VIEW_CLASS_16_BITS: GLenum = 0x82CA;
pub static VIEW_CLASS_8_BITS: GLenum = 0x82CB;
pub static VIEW_CLASS_S3TC_DXT1_RGB: GLenum = 0x82CC;
pub static VIEW_CLASS_S3TC_DXT1_RGBA: GLenum = 0x82CD;
pub static VIEW_CLASS_S3TC_DXT3_RGBA: GLenum = 0x82CE;
pub static VIEW_CLASS_S3TC_DXT5_RGBA: GLenum = 0x82CF;
pub static VIEW_CLASS_RGTC1_RED: GLenum = 0x82D0;
pub static VIEW_CLASS_RGTC2_RG: GLenum = 0x82D1;
pub static VIEW_CLASS_BPTC_UNORM: GLenum = 0x82D2;
pub static VIEW_CLASS_BPTC_FLOAT: GLenum = 0x82D3;
pub static VERTEX_ATTRIB_BINDING: GLenum = 0x82D4;
pub static VERTEX_ATTRIB_RELATIVE_OFFSET: GLenum = 0x82D5;
pub static VERTEX_BINDING_DIVISOR: GLenum = 0x82D6;
pub static VERTEX_BINDING_OFFSET: GLenum = 0x82D7;
pub static VERTEX_BINDING_STRIDE: GLenum = 0x82D8;
pub static MAX_VERTEX_ATTRIB_RELATIVE_OFFSET: GLenum = 0x82D9;
pub static MAX_VERTEX_ATTRIB_BINDINGS: GLenum = 0x82DA;
pub static TEXTURE_VIEW_MIN_LEVEL: GLenum = 0x82DB;
pub static TEXTURE_VIEW_NUM_LEVELS: GLenum = 0x82DC;
pub static TEXTURE_VIEW_MIN_LAYER: GLenum = 0x82DD;
pub static TEXTURE_VIEW_NUM_LAYERS: GLenum = 0x82DE;
pub static TEXTURE_IMMUTABLE_LEVELS: GLenum = 0x82DF;
pub static BUFFER: GLenum = 0x82E0;
pub static SHADER: GLenum = 0x82E1;
pub static PROGRAM: GLenum = 0x82E2;
pub static QUERY: GLenum = 0x82E3;
pub static PROGRAM_PIPELINE: GLenum = 0x82E4;
pub static SAMPLER: GLenum = 0x82E6;
pub static DISPLAY_LIST: GLenum = 0x82E7;
pub static MAX_LABEL_LENGTH: GLenum = 0x82E8;
pub static NUM_SHADING_LANGUAGE_VERSIONS: GLenum = 0x82E9;
pub static UNSIGNED_BYTE_2_3_3_REV: GLenum = 0x8362;
pub static UNSIGNED_SHORT_5_6_5: GLenum = 0x8363;
pub static UNSIGNED_SHORT_5_6_5_REV: GLenum = 0x8364;
pub static UNSIGNED_SHORT_4_4_4_4_REV: GLenum = 0x8365;
pub static UNSIGNED_SHORT_1_5_5_5_REV: GLenum = 0x8366;
pub static UNSIGNED_INT_8_8_8_8_REV: GLenum = 0x8367;
pub static UNSIGNED_INT_2_10_10_10_REV: GLenum = 0x8368;
pub static MIRRORED_REPEAT: GLenum = 0x8370;
pub static ALIASED_LINE_WIDTH_RANGE: GLenum = 0x846E;
pub static TEXTURE0: GLenum = 0x84C0;
pub static TEXTURE1: GLenum = 0x84C1;
pub static TEXTURE2: GLenum = 0x84C2;
pub static TEXTURE3: GLenum = 0x84C3;
pub static TEXTURE4: GLenum = 0x84C4;
pub static TEXTURE5: GLenum = 0x84C5;
pub static TEXTURE6: GLenum = 0x84C6;
pub static TEXTURE7: GLenum = 0x84C7;
pub static TEXTURE8: GLenum = 0x84C8;
pub static TEXTURE9: GLenum = 0x84C9;
pub static TEXTURE10: GLenum = 0x84CA;
pub static TEXTURE11: GLenum = 0x84CB;
pub static TEXTURE12: GLenum = 0x84CC;
pub static TEXTURE13: GLenum = 0x84CD;
pub static TEXTURE14: GLenum = 0x84CE;
pub static TEXTURE15: GLenum = 0x84CF;
pub static TEXTURE16: GLenum = 0x84D0;
pub static TEXTURE17: GLenum = 0x84D1;
pub static TEXTURE18: GLenum = 0x84D2;
pub static TEXTURE19: GLenum = 0x84D3;
pub static TEXTURE20: GLenum = 0x84D4;
pub static TEXTURE21: GLenum = 0x84D5;
pub static TEXTURE22: GLenum = 0x84D6;
pub static TEXTURE23: GLenum = 0x84D7;
pub static TEXTURE24: GLenum = 0x84D8;
pub static TEXTURE25: GLenum = 0x84D9;
pub static TEXTURE26: GLenum = 0x84DA;
pub static TEXTURE27: GLenum = 0x84DB;
pub static TEXTURE28: GLenum = 0x84DC;
pub static TEXTURE29: GLenum = 0x84DD;
pub static TEXTURE30: GLenum = 0x84DE;
pub static TEXTURE31: GLenum = 0x84DF;
pub static ACTIVE_TEXTURE: GLenum = 0x84E0;
pub static MAX_RENDERBUFFER_SIZE: GLenum = 0x84E8;
pub static COMPRESSED_RGB: GLenum = 0x84ED;
pub static COMPRESSED_RGBA: GLenum = 0x84EE;
pub static TEXTURE_COMPRESSION_HINT: GLenum = 0x84EF;
pub static UNIFORM_BLOCK_REFERENCED_BY_TESS_CONTROL_SHADER: GLenum = 0x84F0;
pub static UNIFORM_BLOCK_REFERENCED_BY_TESS_EVALUATION_SHADER: GLenum = 0x84F1;
pub static TEXTURE_RECTANGLE: GLenum = 0x84F5;
pub static TEXTURE_BINDING_RECTANGLE: GLenum = 0x84F6;
pub static PROXY_TEXTURE_RECTANGLE: GLenum = 0x84F7;
pub static MAX_RECTANGLE_TEXTURE_SIZE: GLenum = 0x84F8;
pub static DEPTH_STENCIL: GLenum = 0x84F9;
pub static UNSIGNED_INT_24_8: GLenum = 0x84FA;
pub static MAX_TEXTURE_LOD_BIAS: GLenum = 0x84FD;
pub static TEXTURE_LOD_BIAS: GLenum = 0x8501;
pub static INCR_WRAP: GLenum = 0x8507;
pub static DECR_WRAP: GLenum = 0x8508;
pub static TEXTURE_CUBE_MAP: GLenum = 0x8513;
pub static TEXTURE_BINDING_CUBE_MAP: GLenum = 0x8514;
pub static TEXTURE_CUBE_MAP_POSITIVE_X: GLenum = 0x8515;
pub static TEXTURE_CUBE_MAP_NEGATIVE_X: GLenum = 0x8516;
pub static TEXTURE_CUBE_MAP_POSITIVE_Y: GLenum = 0x8517;
pub static TEXTURE_CUBE_MAP_NEGATIVE_Y: GLenum = 0x8518;
pub static TEXTURE_CUBE_MAP_POSITIVE_Z: GLenum = 0x8519;
pub static TEXTURE_CUBE_MAP_NEGATIVE_Z: GLenum = 0x851A;
pub static PROXY_TEXTURE_CUBE_MAP: GLenum = 0x851B;
pub static MAX_CUBE_MAP_TEXTURE_SIZE: GLenum = 0x851C;
pub static SRC1_ALPHA: GLenum = 0x8589;
pub static VERTEX_ARRAY_BINDING: GLenum = 0x85B5;
pub static VERTEX_ATTRIB_ARRAY_ENABLED: GLenum = 0x8622;
pub static VERTEX_ATTRIB_ARRAY_SIZE: GLenum = 0x8623;
pub static VERTEX_ATTRIB_ARRAY_STRIDE: GLenum = 0x8624;
pub static VERTEX_ATTRIB_ARRAY_TYPE: GLenum = 0x8625;
pub static CURRENT_VERTEX_ATTRIB: GLenum = 0x8626;
pub static VERTEX_PROGRAM_POINT_SIZE: GLenum = 0x8642;
pub static PROGRAM_POINT_SIZE: GLenum = 0x8642;
pub static VERTEX_ATTRIB_ARRAY_POINTER: GLenum = 0x8645;
pub static DEPTH_CLAMP: GLenum = 0x864F;
pub static TEXTURE_COMPRESSED_IMAGE_SIZE: GLenum = 0x86A0;
pub static TEXTURE_COMPRESSED: GLenum = 0x86A1;
pub static NUM_COMPRESSED_TEXTURE_FORMATS: GLenum = 0x86A2;
pub static COMPRESSED_TEXTURE_FORMATS: GLenum = 0x86A3;
pub static PROGRAM_BINARY_LENGTH: GLenum = 0x8741;
pub static VERTEX_ATTRIB_ARRAY_LONG: GLenum = 0x874E;
pub static BUFFER_SIZE: GLenum = 0x8764;
pub static BUFFER_USAGE: GLenum = 0x8765;
pub static NUM_PROGRAM_BINARY_FORMATS: GLenum = 0x87FE;
pub static PROGRAM_BINARY_FORMATS: GLenum = 0x87FF;
pub static STENCIL_BACK_FUNC: GLenum = 0x8800;
pub static STENCIL_BACK_FAIL: GLenum = 0x8801;
pub static STENCIL_BACK_PASS_DEPTH_FAIL: GLenum = 0x8802;
pub static STENCIL_BACK_PASS_DEPTH_PASS: GLenum = 0x8803;
pub static RGBA32F: GLenum = 0x8814;
pub static RGB32F: GLenum = 0x8815;
pub static RGBA16F: GLenum = 0x881A;
pub static RGB16F: GLenum = 0x881B;
pub static MAX_DRAW_BUFFERS: GLenum = 0x8824;
pub static DRAW_BUFFER0: GLenum = 0x8825;
pub static DRAW_BUFFER1: GLenum = 0x8826;
pub static DRAW_BUFFER2: GLenum = 0x8827;
pub static DRAW_BUFFER3: GLenum = 0x8828;
pub static DRAW_BUFFER4: GLenum = 0x8829;
pub static DRAW_BUFFER5: GLenum = 0x882A;
pub static DRAW_BUFFER6: GLenum = 0x882B;
pub static DRAW_BUFFER7: GLenum = 0x882C;
pub static DRAW_BUFFER8: GLenum = 0x882D;
pub static DRAW_BUFFER9: GLenum = 0x882E;
pub static DRAW_BUFFER10: GLenum = 0x882F;
pub static DRAW_BUFFER11: GLenum = 0x8830;
pub static DRAW_BUFFER12: GLenum = 0x8831;
pub static DRAW_BUFFER13: GLenum = 0x8832;
pub static DRAW_BUFFER14: GLenum = 0x8833;
pub static DRAW_BUFFER15: GLenum = 0x8834;
pub static BLEND_EQUATION_ALPHA: GLenum = 0x883D;
pub static TEXTURE_DEPTH_SIZE: GLenum = 0x884A;
pub static TEXTURE_COMPARE_MODE: GLenum = 0x884C;
pub static TEXTURE_COMPARE_FUNC: GLenum = 0x884D;
pub static COMPARE_REF_TO_TEXTURE: GLenum = 0x884E;
pub static TEXTURE_CUBE_MAP_SEAMLESS: GLenum = 0x884F;
pub static QUERY_COUNTER_BITS: GLenum = 0x8864;
pub static CURRENT_QUERY: GLenum = 0x8865;
pub static QUERY_RESULT: GLenum = 0x8866;
pub static QUERY_RESULT_AVAILABLE: GLenum = 0x8867;
pub static MAX_VERTEX_ATTRIBS: GLenum = 0x8869;
pub static VERTEX_ATTRIB_ARRAY_NORMALIZED: GLenum = 0x886A;
pub static MAX_TESS_CONTROL_INPUT_COMPONENTS: GLenum = 0x886C;
pub static MAX_TESS_EVALUATION_INPUT_COMPONENTS: GLenum = 0x886D;
pub static MAX_TEXTURE_IMAGE_UNITS: GLenum = 0x8872;
pub static GEOMETRY_SHADER_INVOCATIONS: GLenum = 0x887F;
pub static ARRAY_BUFFER: GLenum = 0x8892;
pub static ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
pub static ARRAY_BUFFER_BINDING: GLenum = 0x8894;
pub static ELEMENT_ARRAY_BUFFER_BINDING: GLenum = 0x8895;
pub static VERTEX_ATTRIB_ARRAY_BUFFER_BINDING: GLenum = 0x889F;
pub static READ_ONLY: GLenum = 0x88B8;
pub static WRITE_ONLY: GLenum = 0x88B9;
pub static READ_WRITE: GLenum = 0x88BA;
pub static BUFFER_ACCESS: GLenum = 0x88BB;
pub static BUFFER_MAPPED: GLenum = 0x88BC;
pub static BUFFER_MAP_POINTER: GLenum = 0x88BD;
pub static TIME_ELAPSED: GLenum = 0x88BF;
pub static STREAM_DRAW: GLenum = 0x88E0;
pub static STREAM_READ: GLenum = 0x88E1;
pub static STREAM_COPY: GLenum = 0x88E2;
pub static STATIC_DRAW: GLenum = 0x88E4;
pub static STATIC_READ: GLenum = 0x88E5;
pub static STATIC_COPY: GLenum = 0x88E6;
pub static DYNAMIC_DRAW: GLenum = 0x88E8;
pub static DYNAMIC_READ: GLenum = 0x88E9;
pub static DYNAMIC_COPY: GLenum = 0x88EA;
pub static PIXEL_PACK_BUFFER: GLenum = 0x88EB;
pub static PIXEL_UNPACK_BUFFER: GLenum = 0x88EC;
pub static PIXEL_PACK_BUFFER_BINDING: GLenum = 0x88ED;
pub static PIXEL_UNPACK_BUFFER_BINDING: GLenum = 0x88EF;
pub static DEPTH24_STENCIL8: GLenum = 0x88F0;
pub static TEXTURE_STENCIL_SIZE: GLenum = 0x88F1;
pub static SRC1_COLOR: GLenum = 0x88F9;
pub static ONE_MINUS_SRC1_COLOR: GLenum = 0x88FA;
pub static ONE_MINUS_SRC1_ALPHA: GLenum = 0x88FB;
pub static MAX_DUAL_SOURCE_DRAW_BUFFERS: GLenum = 0x88FC;
pub static VERTEX_ATTRIB_ARRAY_INTEGER: GLenum = 0x88FD;
pub static VERTEX_ATTRIB_ARRAY_DIVISOR: GLenum = 0x88FE;
pub static MAX_ARRAY_TEXTURE_LAYERS: GLenum = 0x88FF;
pub static MIN_PROGRAM_TEXEL_OFFSET: GLenum = 0x8904;
pub static MAX_PROGRAM_TEXEL_OFFSET: GLenum = 0x8905;
pub static SAMPLES_PASSED: GLenum = 0x8914;
pub static GEOMETRY_VERTICES_OUT: GLenum = 0x8916;
pub static GEOMETRY_INPUT_TYPE: GLenum = 0x8917;
pub static GEOMETRY_OUTPUT_TYPE: GLenum = 0x8918;
pub static SAMPLER_BINDING: GLenum = 0x8919;
pub static CLAMP_READ_COLOR: GLenum = 0x891C;
pub static FIXED_ONLY: GLenum = 0x891D;
pub static UNIFORM_BUFFER: GLenum = 0x8A11;
pub static UNIFORM_BUFFER_BINDING: GLenum = 0x8A28;
pub static UNIFORM_BUFFER_START: GLenum = 0x8A29;
pub static UNIFORM_BUFFER_SIZE: GLenum = 0x8A2A;
pub static MAX_VERTEX_UNIFORM_BLOCKS: GLenum = 0x8A2B;
pub static MAX_FRAGMENT_UNIFORM_BLOCKS: GLenum = 0x8A2D;
pub static MAX_COMBINED_UNIFORM_BLOCKS: GLenum = 0x8A2E;
pub static MAX_UNIFORM_BUFFER_BINDINGS: GLenum = 0x8A2F;
pub static MAX_UNIFORM_BLOCK_SIZE: GLenum = 0x8A30;
pub static MAX_COMBINED_VERTEX_UNIFORM_COMPONENTS: GLenum = 0x8A31;
pub static MAX_COMBINED_FRAGMENT_UNIFORM_COMPONENTS: GLenum = 0x8A33;
pub static UNIFORM_BUFFER_OFFSET_ALIGNMENT: GLenum = 0x8A34;
pub static ACTIVE_UNIFORM_BLOCK_MAX_NAME_LENGTH: GLenum = 0x8A35;
pub static ACTIVE_UNIFORM_BLOCKS: GLenum = 0x8A36;
pub static UNIFORM_TYPE: GLenum = 0x8A37;
pub static UNIFORM_SIZE: GLenum = 0x8A38;
pub static UNIFORM_NAME_LENGTH: GLenum = 0x8A39;
pub static UNIFORM_BLOCK_INDEX: GLenum = 0x8A3A;
pub static UNIFORM_OFFSET: GLenum = 0x8A3B;
pub static UNIFORM_ARRAY_STRIDE: GLenum = 0x8A3C;
pub static UNIFORM_MATRIX_STRIDE: GLenum = 0x8A3D;
pub static UNIFORM_IS_ROW_MAJOR: GLenum = 0x8A3E;
pub static UNIFORM_BLOCK_BINDING: GLenum = 0x8A3F;
pub static UNIFORM_BLOCK_DATA_SIZE: GLenum = 0x8A40;
pub static UNIFORM_BLOCK_NAME_LENGTH: GLenum = 0x8A41;
pub static UNIFORM_BLOCK_ACTIVE_UNIFORMS: GLenum = 0x8A42;
pub static UNIFORM_BLOCK_ACTIVE_UNIFORM_INDICES: GLenum = 0x8A43;
pub static UNIFORM_BLOCK_REFERENCED_BY_VERTEX_SHADER: GLenum = 0x8A44;
pub static UNIFORM_BLOCK_REFERENCED_BY_FRAGMENT_SHADER: GLenum = 0x8A46;
pub static FRAGMENT_SHADER: GLenum = 0x8B30;
pub static VERTEX_SHADER: GLenum = 0x8B31;
pub static MAX_FRAGMENT_UNIFORM_COMPONENTS: GLenum = 0x8B49;
pub static MAX_VERTEX_UNIFORM_COMPONENTS: GLenum = 0x8B4A;
pub static MAX_VARYING_FLOATS: GLenum = 0x8B4B;
pub static MAX_VARYING_COMPONENTS: GLenum = 0x8B4B;
pub static MAX_VERTEX_TEXTURE_IMAGE_UNITS: GLenum = 0x8B4C;
pub static MAX_COMBINED_TEXTURE_IMAGE_UNITS: GLenum = 0x8B4D;
pub static SHADER_TYPE: GLenum = 0x8B4F;
pub static FLOAT_VEC2: GLenum = 0x8B50;
pub static FLOAT_VEC3: GLenum = 0x8B51;
pub static FLOAT_VEC4: GLenum = 0x8B52;
pub static INT_VEC2: GLenum = 0x8B53;
pub static INT_VEC3: GLenum = 0x8B54;
pub static INT_VEC4: GLenum = 0x8B55;
pub static BOOL: GLenum = 0x8B56;
pub static BOOL_VEC2: GLenum = 0x8B57;
pub static BOOL_VEC3: GLenum = 0x8B58;
pub static BOOL_VEC4: GLenum = 0x8B59;
pub static FLOAT_MAT2: GLenum = 0x8B5A;
pub static FLOAT_MAT3: GLenum = 0x8B5B;
pub static FLOAT_MAT4: GLenum = 0x8B5C;
pub static SAMPLER_1D: GLenum = 0x8B5D;
pub static SAMPLER_2D: GLenum = 0x8B5E;
pub static SAMPLER_3D: GLenum = 0x8B5F;
pub static SAMPLER_CUBE: GLenum = 0x8B60;
pub static SAMPLER_1D_SHADOW: GLenum = 0x8B61;
pub static SAMPLER_2D_SHADOW: GLenum = 0x8B62;
pub static SAMPLER_2D_RECT: GLenum = 0x8B63;
pub static SAMPLER_2D_RECT_SHADOW: GLenum = 0x8B64;
pub static FLOAT_MAT2x3: GLenum = 0x8B65;
pub static FLOAT_MAT2x4: GLenum = 0x8B66;
pub static FLOAT_MAT3x2: GLenum = 0x8B67;
pub static FLOAT_MAT3x4: GLenum = 0x8B68;
pub static FLOAT_MAT4x2: GLenum = 0x8B69;
pub static FLOAT_MAT4x3: GLenum = 0x8B6A;
pub static DELETE_STATUS: GLenum = 0x8B80;
pub static COMPILE_STATUS: GLenum = 0x8B81;
pub static LINK_STATUS: GLenum = 0x8B82;
pub static VALIDATE_STATUS: GLenum = 0x8B83;
pub static INFO_LOG_LENGTH: GLenum = 0x8B84;
pub static ATTACHED_SHADERS: GLenum = 0x8B85;
pub static ACTIVE_UNIFORMS: GLenum = 0x8B86;
pub static ACTIVE_UNIFORM_MAX_LENGTH: GLenum = 0x8B87;
pub static SHADER_SOURCE_LENGTH: GLenum = 0x8B88;
pub static ACTIVE_ATTRIBUTES: GLenum = 0x8B89;
pub static ACTIVE_ATTRIBUTE_MAX_LENGTH: GLenum = 0x8B8A;
pub static FRAGMENT_SHADER_DERIVATIVE_HINT: GLenum = 0x8B8B;
pub static SHADING_LANGUAGE_VERSION: GLenum = 0x8B8C;
pub static CURRENT_PROGRAM: GLenum = 0x8B8D;
pub static IMPLEMENTATION_COLOR_READ_TYPE: GLenum = 0x8B9A;
pub static IMPLEMENTATION_COLOR_READ_FORMAT: GLenum = 0x8B9B;
pub static TEXTURE_RED_TYPE: GLenum = 0x8C10;
pub static TEXTURE_GREEN_TYPE: GLenum = 0x8C11;
pub static TEXTURE_BLUE_TYPE: GLenum = 0x8C12;
pub static TEXTURE_ALPHA_TYPE: GLenum = 0x8C13;
pub static TEXTURE_DEPTH_TYPE: GLenum = 0x8C16;
pub static UNSIGNED_NORMALIZED: GLenum = 0x8C17;
pub static TEXTURE_1D_ARRAY: GLenum = 0x8C18;
pub static PROXY_TEXTURE_1D_ARRAY: GLenum = 0x8C19;
pub static TEXTURE_2D_ARRAY: GLenum = 0x8C1A;
pub static PROXY_TEXTURE_2D_ARRAY: GLenum = 0x8C1B;
pub static TEXTURE_BINDING_1D_ARRAY: GLenum = 0x8C1C;
pub static TEXTURE_BINDING_2D_ARRAY: GLenum = 0x8C1D;
pub static MAX_GEOMETRY_TEXTURE_IMAGE_UNITS: GLenum = 0x8C29;
pub static TEXTURE_BUFFER: GLenum = 0x8C2A;
pub static MAX_TEXTURE_BUFFER_SIZE: GLenum = 0x8C2B;
pub static TEXTURE_BINDING_BUFFER: GLenum = 0x8C2C;
pub static TEXTURE_BUFFER_DATA_STORE_BINDING: GLenum = 0x8C2D;
pub static ANY_SAMPLES_PASSED: GLenum = 0x8C2F;
pub static SAMPLE_SHADING: GLenum = 0x8C36;
pub static MIN_SAMPLE_SHADING_VALUE: GLenum = 0x8C37;
pub static R11F_G11F_B10F: GLenum = 0x8C3A;
pub static UNSIGNED_INT_10F_11F_11F_REV: GLenum = 0x8C3B;
pub static RGB9_E5: GLenum = 0x8C3D;
pub static UNSIGNED_INT_5_9_9_9_REV: GLenum = 0x8C3E;
pub static TEXTURE_SHARED_SIZE: GLenum = 0x8C3F;
pub static SRGB: GLenum = 0x8C40;
pub static SRGB8: GLenum = 0x8C41;
pub static SRGB_ALPHA: GLenum = 0x8C42;
pub static SRGB8_ALPHA8: GLenum = 0x8C43;
pub static COMPRESSED_SRGB: GLenum = 0x8C48;
pub static COMPRESSED_SRGB_ALPHA: GLenum = 0x8C49;
pub static TRANSFORM_FEEDBACK_VARYING_MAX_LENGTH: GLenum = 0x8C76;
pub static TRANSFORM_FEEDBACK_BUFFER_MODE: GLenum = 0x8C7F;
pub static MAX_TRANSFORM_FEEDBACK_SEPARATE_COMPONENTS: GLenum = 0x8C80;
pub static TRANSFORM_FEEDBACK_VARYINGS: GLenum = 0x8C83;
pub static TRANSFORM_FEEDBACK_BUFFER_START: GLenum = 0x8C84;
pub static TRANSFORM_FEEDBACK_BUFFER_SIZE: GLenum = 0x8C85;
pub static PRIMITIVES_GENERATED: GLenum = 0x8C87;
pub static TRANSFORM_FEEDBACK_PRIMITIVES_WRITTEN: GLenum = 0x8C88;
pub static RASTERIZER_DISCARD: GLenum = 0x8C89;
pub static MAX_TRANSFORM_FEEDBACK_INTERLEAVED_COMPONENTS: GLenum = 0x8C8A;
pub static MAX_TRANSFORM_FEEDBACK_SEPARATE_ATTRIBS: GLenum = 0x8C8B;
pub static INTERLEAVED_ATTRIBS: GLenum = 0x8C8C;
pub static SEPARATE_ATTRIBS: GLenum = 0x8C8D;
pub static TRANSFORM_FEEDBACK_BUFFER: GLenum = 0x8C8E;
pub static TRANSFORM_FEEDBACK_BUFFER_BINDING: GLenum = 0x8C8F;
pub static POINT_SPRITE_COORD_ORIGIN: GLenum = 0x8CA0;
pub static LOWER_LEFT: GLenum = 0x8CA1;
pub static UPPER_LEFT: GLenum = 0x8CA2;
pub static STENCIL_BACK_REF: GLenum = 0x8CA3;
pub static STENCIL_BACK_VALUE_MASK: GLenum = 0x8CA4;
pub static STENCIL_BACK_WRITEMASK: GLenum = 0x8CA5;
pub static DRAW_FRAMEBUFFER_BINDING: GLenum = 0x8CA6;
pub static FRAMEBUFFER_BINDING: GLenum = 0x8CA6;
pub static RENDERBUFFER_BINDING: GLenum = 0x8CA7;
pub static READ_FRAMEBUFFER: GLenum = 0x8CA8;
pub static DRAW_FRAMEBUFFER: GLenum = 0x8CA9;
pub static READ_FRAMEBUFFER_BINDING: GLenum = 0x8CAA;
pub static RENDERBUFFER_SAMPLES: GLenum = 0x8CAB;
pub static DEPTH_COMPONENT32F: GLenum = 0x8CAC;
pub static DEPTH32F_STENCIL8: GLenum = 0x8CAD;
pub static FRAMEBUFFER_ATTACHMENT_OBJECT_TYPE: GLenum = 0x8CD0;
pub static FRAMEBUFFER_ATTACHMENT_OBJECT_NAME: GLenum = 0x8CD1;
pub static FRAMEBUFFER_ATTACHMENT_TEXTURE_LEVEL: GLenum = 0x8CD2;
pub static FRAMEBUFFER_ATTACHMENT_TEXTURE_CUBE_MAP_FACE: GLenum = 0x8CD3;
pub static FRAMEBUFFER_ATTACHMENT_TEXTURE_LAYER: GLenum = 0x8CD4;
pub static FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;
pub static FRAMEBUFFER_INCOMPLETE_ATTACHMENT: GLenum = 0x8CD6;
pub static FRAMEBUFFER_INCOMPLETE_MISSING_ATTACHMENT: GLenum = 0x8CD7;
pub static FRAMEBUFFER_INCOMPLETE_DRAW_BUFFER: GLenum = 0x8CDB;
pub static FRAMEBUFFER_INCOMPLETE_READ_BUFFER: GLenum = 0x8CDC;
pub static FRAMEBUFFER_UNSUPPORTED: GLenum = 0x8CDD;
pub static MAX_COLOR_ATTACHMENTS: GLenum = 0x8CDF;
pub static COLOR_ATTACHMENT0: GLenum = 0x8CE0;
pub static COLOR_ATTACHMENT1: GLenum = 0x8CE1;
pub static COLOR_ATTACHMENT2: GLenum = 0x8CE2;
pub static COLOR_ATTACHMENT3: GLenum = 0x8CE3;
pub static COLOR_ATTACHMENT4: GLenum = 0x8CE4;
pub static COLOR_ATTACHMENT5: GLenum = 0x8CE5;
pub static COLOR_ATTACHMENT6: GLenum = 0x8CE6;
pub static COLOR_ATTACHMENT7: GLenum = 0x8CE7;
pub static COLOR_ATTACHMENT8: GLenum = 0x8CE8;
pub static COLOR_ATTACHMENT9: GLenum = 0x8CE9;
pub static COLOR_ATTACHMENT10: GLenum = 0x8CEA;
pub static COLOR_ATTACHMENT11: GLenum = 0x8CEB;
pub static COLOR_ATTACHMENT12: GLenum = 0x8CEC;
pub static COLOR_ATTACHMENT13: GLenum = 0x8CED;
pub static COLOR_ATTACHMENT14: GLenum = 0x8CEE;
pub static COLOR_ATTACHMENT15: GLenum = 0x8CEF;
pub static DEPTH_ATTACHMENT: GLenum = 0x8D00;
pub static STENCIL_ATTACHMENT: GLenum = 0x8D20;
pub static FRAMEBUFFER: GLenum = 0x8D40;
pub static RENDERBUFFER: GLenum = 0x8D41;
pub static RENDERBUFFER_WIDTH: GLenum = 0x8D42;
pub static RENDERBUFFER_HEIGHT: GLenum = 0x8D43;
pub static RENDERBUFFER_INTERNAL_FORMAT: GLenum = 0x8D44;
pub static STENCIL_INDEX1: GLenum = 0x8D46;
pub static STENCIL_INDEX4: GLenum = 0x8D47;
pub static STENCIL_INDEX8: GLenum = 0x8D48;
pub static STENCIL_INDEX16: GLenum = 0x8D49;
pub static RENDERBUFFER_RED_SIZE: GLenum = 0x8D50;
pub static RENDERBUFFER_GREEN_SIZE: GLenum = 0x8D51;
pub static RENDERBUFFER_BLUE_SIZE: GLenum = 0x8D52;
pub static RENDERBUFFER_ALPHA_SIZE: GLenum = 0x8D53;
pub static RENDERBUFFER_DEPTH_SIZE: GLenum = 0x8D54;
pub static RENDERBUFFER_STENCIL_SIZE: GLenum = 0x8D55;
pub static FRAMEBUFFER_INCOMPLETE_MULTISAMPLE: GLenum = 0x8D56;
pub static MAX_SAMPLES: GLenum = 0x8D57;
pub static RGB565: GLenum = 0x8D62;
pub static PRIMITIVE_RESTART_FIXED_INDEX: GLenum = 0x8D69;
pub static ANY_SAMPLES_PASSED_CONSERVATIVE: GLenum = 0x8D6A;
pub static MAX_ELEMENT_INDEX: GLenum = 0x8D6B;
pub static RGBA32UI: GLenum = 0x8D70;
pub static RGB32UI: GLenum = 0x8D71;
pub static RGBA16UI: GLenum = 0x8D76;
pub static RGB16UI: GLenum = 0x8D77;
pub static RGBA8UI: GLenum = 0x8D7C;
pub static RGB8UI: GLenum = 0x8D7D;
pub static RGBA32I: GLenum = 0x8D82;
pub static RGB32I: GLenum = 0x8D83;
pub static RGBA16I: GLenum = 0x8D88;
pub static RGB16I: GLenum = 0x8D89;
pub static RGBA8I: GLenum = 0x8D8E;
pub static RGB8I: GLenum = 0x8D8F;
pub static RED_INTEGER: GLenum = 0x8D94;
pub static GREEN_INTEGER: GLenum = 0x8D95;
pub static BLUE_INTEGER: GLenum = 0x8D96;
pub static RGB_INTEGER: GLenum = 0x8D98;
pub static RGBA_INTEGER: GLenum = 0x8D99;
pub static BGR_INTEGER: GLenum = 0x8D9A;
pub static BGRA_INTEGER: GLenum = 0x8D9B;
pub static INT_2_10_10_10_REV: GLenum = 0x8D9F;
pub static FRAMEBUFFER_ATTACHMENT_LAYERED: GLenum = 0x8DA7;
pub static FRAMEBUFFER_INCOMPLETE_LAYER_TARGETS: GLenum = 0x8DA8;
pub static FLOAT_32_UNSIGNED_INT_24_8_REV: GLenum = 0x8DAD;
pub static FRAMEBUFFER_SRGB: GLenum = 0x8DB9;
pub static COMPRESSED_RED_RGTC1: GLenum = 0x8DBB;
pub static COMPRESSED_SIGNED_RED_RGTC1: GLenum = 0x8DBC;
pub static COMPRESSED_RG_RGTC2: GLenum = 0x8DBD;
pub static COMPRESSED_SIGNED_RG_RGTC2: GLenum = 0x8DBE;
pub static SAMPLER_1D_ARRAY: GLenum = 0x8DC0;
pub static SAMPLER_2D_ARRAY: GLenum = 0x8DC1;
pub static SAMPLER_BUFFER: GLenum = 0x8DC2;
pub static SAMPLER_1D_ARRAY_SHADOW: GLenum = 0x8DC3;
pub static SAMPLER_2D_ARRAY_SHADOW: GLenum = 0x8DC4;
pub static SAMPLER_CUBE_SHADOW: GLenum = 0x8DC5;
pub static UNSIGNED_INT_VEC2: GLenum = 0x8DC6;
pub static UNSIGNED_INT_VEC3: GLenum = 0x8DC7;
pub static UNSIGNED_INT_VEC4: GLenum = 0x8DC8;
pub static INT_SAMPLER_1D: GLenum = 0x8DC9;
pub static INT_SAMPLER_2D: GLenum = 0x8DCA;
pub static INT_SAMPLER_3D: GLenum = 0x8DCB;
pub static INT_SAMPLER_CUBE: GLenum = 0x8DCC;
pub static INT_SAMPLER_2D_RECT: GLenum = 0x8DCD;
pub static INT_SAMPLER_1D_ARRAY: GLenum = 0x8DCE;
pub static INT_SAMPLER_2D_ARRAY: GLenum = 0x8DCF;
pub static INT_SAMPLER_BUFFER: GLenum = 0x8DD0;
pub static UNSIGNED_INT_SAMPLER_1D: GLenum = 0x8DD1;
pub static UNSIGNED_INT_SAMPLER_2D: GLenum = 0x8DD2;
pub static UNSIGNED_INT_SAMPLER_3D: GLenum = 0x8DD3;
pub static UNSIGNED_INT_SAMPLER_CUBE: GLenum = 0x8DD4;
pub static UNSIGNED_INT_SAMPLER_2D_RECT: GLenum = 0x8DD5;
pub static UNSIGNED_INT_SAMPLER_1D_ARRAY: GLenum = 0x8DD6;
pub static UNSIGNED_INT_SAMPLER_2D_ARRAY: GLenum = 0x8DD7;
pub static UNSIGNED_INT_SAMPLER_BUFFER: GLenum = 0x8DD8;
pub static GEOMETRY_SHADER: GLenum = 0x8DD9;
pub static MAX_GEOMETRY_UNIFORM_COMPONENTS: GLenum = 0x8DDF;
pub static MAX_GEOMETRY_OUTPUT_VERTICES: GLenum = 0x8DE0;
pub static MAX_GEOMETRY_TOTAL_OUTPUT_COMPONENTS: GLenum = 0x8DE1;
pub static ACTIVE_SUBROUTINES: GLenum = 0x8DE5;
pub static ACTIVE_SUBROUTINE_UNIFORMS: GLenum = 0x8DE6;
pub static MAX_SUBROUTINES: GLenum = 0x8DE7;
pub static MAX_SUBROUTINE_UNIFORM_LOCATIONS: GLenum = 0x8DE8;
pub static LOW_FLOAT: GLenum = 0x8DF0;
pub static MEDIUM_FLOAT: GLenum = 0x8DF1;
pub static HIGH_FLOAT: GLenum = 0x8DF2;
pub static LOW_INT: GLenum = 0x8DF3;
pub static MEDIUM_INT: GLenum = 0x8DF4;
pub static HIGH_INT: GLenum = 0x8DF5;
pub static SHADER_BINARY_FORMATS: GLenum = 0x8DF8;
pub static NUM_SHADER_BINARY_FORMATS: GLenum = 0x8DF9;
pub static SHADER_COMPILER: GLenum = 0x8DFA;
pub static MAX_VERTEX_UNIFORM_VECTORS: GLenum = 0x8DFB;
pub static MAX_VARYING_VECTORS: GLenum = 0x8DFC;
pub static MAX_FRAGMENT_UNIFORM_VECTORS: GLenum = 0x8DFD;
pub static QUERY_WAIT: GLenum = 0x8E13;
pub static QUERY_NO_WAIT: GLenum = 0x8E14;
pub static QUERY_BY_REGION_WAIT: GLenum = 0x8E15;
pub static QUERY_BY_REGION_NO_WAIT: GLenum = 0x8E16;
pub static MAX_COMBINED_TESS_CONTROL_UNIFORM_COMPONENTS: GLenum = 0x8E1E;
pub static MAX_COMBINED_TESS_EVALUATION_UNIFORM_COMPONENTS: GLenum = 0x8E1F;
pub static TRANSFORM_FEEDBACK: GLenum = 0x8E22;
pub static TRANSFORM_FEEDBACK_BUFFER_PAUSED: GLenum = 0x8E23;
pub static TRANSFORM_FEEDBACK_BUFFER_ACTIVE: GLenum = 0x8E24;
pub static TRANSFORM_FEEDBACK_BINDING: GLenum = 0x8E25;
pub static TIMESTAMP: GLenum = 0x8E28;
pub static TEXTURE_SWIZZLE_R: GLenum = 0x8E42;
pub static TEXTURE_SWIZZLE_G: GLenum = 0x8E43;
pub static TEXTURE_SWIZZLE_B: GLenum = 0x8E44;
pub static TEXTURE_SWIZZLE_A: GLenum = 0x8E45;
pub static TEXTURE_SWIZZLE_RGBA: GLenum = 0x8E46;
pub static ACTIVE_SUBROUTINE_UNIFORM_LOCATIONS: GLenum = 0x8E47;
pub static ACTIVE_SUBROUTINE_MAX_LENGTH: GLenum = 0x8E48;
pub static ACTIVE_SUBROUTINE_UNIFORM_MAX_LENGTH: GLenum = 0x8E49;
pub static NUM_COMPATIBLE_SUBROUTINES: GLenum = 0x8E4A;
pub static COMPATIBLE_SUBROUTINES: GLenum = 0x8E4B;
pub static QUADS_FOLLOW_PROVOKING_VERTEX_CONVENTION: GLenum = 0x8E4C;
pub static FIRST_VERTEX_CONVENTION: GLenum = 0x8E4D;
pub static LAST_VERTEX_CONVENTION: GLenum = 0x8E4E;
pub static PROVOKING_VERTEX: GLenum = 0x8E4F;
pub static SAMPLE_POSITION: GLenum = 0x8E50;
pub static SAMPLE_MASK: GLenum = 0x8E51;
pub static SAMPLE_MASK_VALUE: GLenum = 0x8E52;
pub static MAX_SAMPLE_MASK_WORDS: GLenum = 0x8E59;
pub static MAX_GEOMETRY_SHADER_INVOCATIONS: GLenum = 0x8E5A;
pub static MIN_FRAGMENT_INTERPOLATION_OFFSET: GLenum = 0x8E5B;
pub static MAX_FRAGMENT_INTERPOLATION_OFFSET: GLenum = 0x8E5C;
pub static FRAGMENT_INTERPOLATION_OFFSET_BITS: GLenum = 0x8E5D;
pub static MIN_PROGRAM_TEXTURE_GATHER_OFFSET: GLenum = 0x8E5E;
pub static MAX_PROGRAM_TEXTURE_GATHER_OFFSET: GLenum = 0x8E5F;
pub static MAX_TRANSFORM_FEEDBACK_BUFFERS: GLenum = 0x8E70;
pub static MAX_VERTEX_STREAMS: GLenum = 0x8E71;
pub static PATCH_VERTICES: GLenum = 0x8E72;
pub static PATCH_DEFAULT_INNER_LEVEL: GLenum = 0x8E73;
pub static PATCH_DEFAULT_OUTER_LEVEL: GLenum = 0x8E74;
pub static TESS_CONTROL_OUTPUT_VERTICES: GLenum = 0x8E75;
pub static TESS_GEN_MODE: GLenum = 0x8E76;
pub static TESS_GEN_SPACING: GLenum = 0x8E77;
pub static TESS_GEN_VERTEX_ORDER: GLenum = 0x8E78;
pub static TESS_GEN_POINT_MODE: GLenum = 0x8E79;
pub static ISOLINES: GLenum = 0x8E7A;
pub static FRACTIONAL_ODD: GLenum = 0x8E7B;
pub static FRACTIONAL_EVEN: GLenum = 0x8E7C;
pub static MAX_PATCH_VERTICES: GLenum = 0x8E7D;
pub static MAX_TESS_GEN_LEVEL: GLenum = 0x8E7E;
pub static MAX_TESS_CONTROL_UNIFORM_COMPONENTS: GLenum = 0x8E7F;
pub static MAX_TESS_EVALUATION_UNIFORM_COMPONENTS: GLenum = 0x8E80;
pub static MAX_TESS_CONTROL_TEXTURE_IMAGE_UNITS: GLenum = 0x8E81;
pub static MAX_TESS_EVALUATION_TEXTURE_IMAGE_UNITS: GLenum = 0x8E82;
pub static MAX_TESS_CONTROL_OUTPUT_COMPONENTS: GLenum = 0x8E83;
pub static MAX_TESS_PATCH_COMPONENTS: GLenum = 0x8E84;
pub static MAX_TESS_CONTROL_TOTAL_OUTPUT_COMPONENTS: GLenum = 0x8E85;
pub static MAX_TESS_EVALUATION_OUTPUT_COMPONENTS: GLenum = 0x8E86;
pub static TESS_EVALUATION_SHADER: GLenum = 0x8E87;
pub static TESS_CONTROL_SHADER: GLenum = 0x8E88;
pub static MAX_TESS_CONTROL_UNIFORM_BLOCKS: GLenum = 0x8E89;
pub static MAX_TESS_EVALUATION_UNIFORM_BLOCKS: GLenum = 0x8E8A;
pub static COMPRESSED_RGBA_BPTC_UNORM: GLenum = 0x8E8C;
pub static COMPRESSED_SRGB_ALPHA_BPTC_UNORM: GLenum = 0x8E8D;
pub static COMPRESSED_RGB_BPTC_SIGNED_FLOAT: GLenum = 0x8E8E;
pub static COMPRESSED_RGB_BPTC_UNSIGNED_FLOAT: GLenum = 0x8E8F;
pub static COPY_READ_BUFFER: GLenum = 0x8F36;
pub static COPY_WRITE_BUFFER: GLenum = 0x8F37;
pub static MAX_IMAGE_UNITS: GLenum = 0x8F38;
pub static MAX_COMBINED_IMAGE_UNITS_AND_FRAGMENT_OUTPUTS: GLenum = 0x8F39;
pub static MAX_COMBINED_SHADER_OUTPUT_RESOURCES: GLenum = 0x8F39;
pub static IMAGE_BINDING_NAME: GLenum = 0x8F3A;
pub static IMAGE_BINDING_LEVEL: GLenum = 0x8F3B;
pub static IMAGE_BINDING_LAYERED: GLenum = 0x8F3C;
pub static IMAGE_BINDING_LAYER: GLenum = 0x8F3D;
pub static IMAGE_BINDING_ACCESS: GLenum = 0x8F3E;
pub static DRAW_INDIRECT_BUFFER: GLenum = 0x8F3F;
pub static DRAW_INDIRECT_BUFFER_BINDING: GLenum = 0x8F43;
pub static DOUBLE_MAT2: GLenum = 0x8F46;
pub static DOUBLE_MAT3: GLenum = 0x8F47;
pub static DOUBLE_MAT4: GLenum = 0x8F48;
pub static DOUBLE_MAT2x3: GLenum = 0x8F49;
pub static DOUBLE_MAT2x4: GLenum = 0x8F4A;
pub static DOUBLE_MAT3x2: GLenum = 0x8F4B;
pub static DOUBLE_MAT3x4: GLenum = 0x8F4C;
pub static DOUBLE_MAT4x2: GLenum = 0x8F4D;
pub static DOUBLE_MAT4x3: GLenum = 0x8F4E;
pub static VERTEX_BINDING_BUFFER: GLenum = 0x8F4F;
pub static R8_SNORM: GLenum = 0x8F94;
pub static RG8_SNORM: GLenum = 0x8F95;
pub static RGB8_SNORM: GLenum = 0x8F96;
pub static RGBA8_SNORM: GLenum = 0x8F97;
pub static R16_SNORM: GLenum = 0x8F98;
pub static RG16_SNORM: GLenum = 0x8F99;
pub static RGB16_SNORM: GLenum = 0x8F9A;
pub static RGBA16_SNORM: GLenum = 0x8F9B;
pub static SIGNED_NORMALIZED: GLenum = 0x8F9C;
pub static PRIMITIVE_RESTART: GLenum = 0x8F9D;
pub static PRIMITIVE_RESTART_INDEX: GLenum = 0x8F9E;
pub static DOUBLE_VEC2: GLenum = 0x8FFC;
pub static DOUBLE_VEC3: GLenum = 0x8FFD;
pub static DOUBLE_VEC4: GLenum = 0x8FFE;
pub static TEXTURE_CUBE_MAP_ARRAY: GLenum = 0x9009;
pub static TEXTURE_BINDING_CUBE_MAP_ARRAY: GLenum = 0x900A;
pub static PROXY_TEXTURE_CUBE_MAP_ARRAY: GLenum = 0x900B;
pub static SAMPLER_CUBE_MAP_ARRAY: GLenum = 0x900C;
pub static SAMPLER_CUBE_MAP_ARRAY_SHADOW: GLenum = 0x900D;
pub static INT_SAMPLER_CUBE_MAP_ARRAY: GLenum = 0x900E;
pub static UNSIGNED_INT_SAMPLER_CUBE_MAP_ARRAY: GLenum = 0x900F;
pub static IMAGE_1D: GLenum = 0x904C;
pub static IMAGE_2D: GLenum = 0x904D;
pub static IMAGE_3D: GLenum = 0x904E;
pub static IMAGE_2D_RECT: GLenum = 0x904F;
pub static IMAGE_CUBE: GLenum = 0x9050;
pub static IMAGE_BUFFER: GLenum = 0x9051;
pub static IMAGE_1D_ARRAY: GLenum = 0x9052;
pub static IMAGE_2D_ARRAY: GLenum = 0x9053;
pub static IMAGE_CUBE_MAP_ARRAY: GLenum = 0x9054;
pub static IMAGE_2D_MULTISAMPLE: GLenum = 0x9055;
pub static IMAGE_2D_MULTISAMPLE_ARRAY: GLenum = 0x9056;
pub static INT_IMAGE_1D: GLenum = 0x9057;
pub static INT_IMAGE_2D: GLenum = 0x9058;
pub static INT_IMAGE_3D: GLenum = 0x9059;
pub static INT_IMAGE_2D_RECT: GLenum = 0x905A;
pub static INT_IMAGE_CUBE: GLenum = 0x905B;
pub static INT_IMAGE_BUFFER: GLenum = 0x905C;
pub static INT_IMAGE_1D_ARRAY: GLenum = 0x905D;
pub static INT_IMAGE_2D_ARRAY: GLenum = 0x905E;
pub static INT_IMAGE_CUBE_MAP_ARRAY: GLenum = 0x905F;
pub static INT_IMAGE_2D_MULTISAMPLE: GLenum = 0x9060;
pub static INT_IMAGE_2D_MULTISAMPLE_ARRAY: GLenum = 0x9061;
pub static UNSIGNED_INT_IMAGE_1D: GLenum = 0x9062;
pub static UNSIGNED_INT_IMAGE_2D: GLenum = 0x9063;
pub static UNSIGNED_INT_IMAGE_3D: GLenum = 0x9064;
pub static UNSIGNED_INT_IMAGE_2D_RECT: GLenum = 0x9065;
pub static UNSIGNED_INT_IMAGE_CUBE: GLenum = 0x9066;
pub static UNSIGNED_INT_IMAGE_BUFFER: GLenum = 0x9067;
pub static UNSIGNED_INT_IMAGE_1D_ARRAY: GLenum = 0x9068;
pub static UNSIGNED_INT_IMAGE_2D_ARRAY: GLenum = 0x9069;
pub static UNSIGNED_INT_IMAGE_CUBE_MAP_ARRAY: GLenum = 0x906A;
pub static UNSIGNED_INT_IMAGE_2D_MULTISAMPLE: GLenum = 0x906B;
pub static UNSIGNED_INT_IMAGE_2D_MULTISAMPLE_ARRAY: GLenum = 0x906C;
pub static MAX_IMAGE_SAMPLES: GLenum = 0x906D;
pub static IMAGE_BINDING_FORMAT: GLenum = 0x906E;
pub static RGB10_A2UI: GLenum = 0x906F;
pub static MIN_MAP_BUFFER_ALIGNMENT: GLenum = 0x90BC;
pub static IMAGE_FORMAT_COMPATIBILITY_TYPE: GLenum = 0x90C7;
pub static IMAGE_FORMAT_COMPATIBILITY_BY_SIZE: GLenum = 0x90C8;
pub static IMAGE_FORMAT_COMPATIBILITY_BY_CLASS: GLenum = 0x90C9;
pub static MAX_VERTEX_IMAGE_UNIFORMS: GLenum = 0x90CA;
pub static MAX_TESS_CONTROL_IMAGE_UNIFORMS: GLenum = 0x90CB;
pub static MAX_TESS_EVALUATION_IMAGE_UNIFORMS: GLenum = 0x90CC;
pub static MAX_GEOMETRY_IMAGE_UNIFORMS: GLenum = 0x90CD;
pub static MAX_FRAGMENT_IMAGE_UNIFORMS: GLenum = 0x90CE;
pub static MAX_COMBINED_IMAGE_UNIFORMS: GLenum = 0x90CF;
pub static SHADER_STORAGE_BUFFER: GLenum = 0x90D2;
pub static SHADER_STORAGE_BUFFER_BINDING: GLenum = 0x90D3;
pub static SHADER_STORAGE_BUFFER_START: GLenum = 0x90D4;
pub static SHADER_STORAGE_BUFFER_SIZE: GLenum = 0x90D5;
pub static MAX_VERTEX_SHADER_STORAGE_BLOCKS: GLenum = 0x90D6;
pub static MAX_GEOMETRY_SHADER_STORAGE_BLOCKS: GLenum = 0x90D7;
pub static MAX_TESS_CONTROL_SHADER_STORAGE_BLOCKS: GLenum = 0x90D8;
pub static MAX_TESS_EVALUATION_SHADER_STORAGE_BLOCKS: GLenum = 0x90D9;
pub static MAX_FRAGMENT_SHADER_STORAGE_BLOCKS: GLenum = 0x90DA;
pub static MAX_COMPUTE_SHADER_STORAGE_BLOCKS: GLenum = 0x90DB;
pub static MAX_COMBINED_SHADER_STORAGE_BLOCKS: GLenum = 0x90DC;
pub static MAX_SHADER_STORAGE_BUFFER_BINDINGS: GLenum = 0x90DD;
pub static MAX_SHADER_STORAGE_BLOCK_SIZE: GLenum = 0x90DE;
pub static SHADER_STORAGE_BUFFER_OFFSET_ALIGNMENT: GLenum = 0x90DF;
pub static DEPTH_STENCIL_TEXTURE_MODE: GLenum = 0x90EA;
pub static MAX_COMPUTE_WORK_GROUP_INVOCATIONS: GLenum = 0x90EB;
pub static UNIFORM_BLOCK_REFERENCED_BY_COMPUTE_SHADER: GLenum = 0x90EC;
pub static ATOMIC_COUNTER_BUFFER_REFERENCED_BY_COMPUTE_SHADER: GLenum = 0x90ED;
pub static DISPATCH_INDIRECT_BUFFER: GLenum = 0x90EE;
pub static DISPATCH_INDIRECT_BUFFER_BINDING: GLenum = 0x90EF;
pub static TEXTURE_2D_MULTISAMPLE: GLenum = 0x9100;
pub static PROXY_TEXTURE_2D_MULTISAMPLE: GLenum = 0x9101;
pub static TEXTURE_2D_MULTISAMPLE_ARRAY: GLenum = 0x9102;
pub static PROXY_TEXTURE_2D_MULTISAMPLE_ARRAY: GLenum = 0x9103;
pub static TEXTURE_BINDING_2D_MULTISAMPLE: GLenum = 0x9104;
pub static TEXTURE_BINDING_2D_MULTISAMPLE_ARRAY: GLenum = 0x9105;
pub static TEXTURE_SAMPLES: GLenum = 0x9106;
pub static TEXTURE_FIXED_SAMPLE_LOCATIONS: GLenum = 0x9107;
pub static SAMPLER_2D_MULTISAMPLE: GLenum = 0x9108;
pub static INT_SAMPLER_2D_MULTISAMPLE: GLenum = 0x9109;
pub static UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE: GLenum = 0x910A;
pub static SAMPLER_2D_MULTISAMPLE_ARRAY: GLenum = 0x910B;
pub static INT_SAMPLER_2D_MULTISAMPLE_ARRAY: GLenum = 0x910C;
pub static UNSIGNED_INT_SAMPLER_2D_MULTISAMPLE_ARRAY: GLenum = 0x910D;
pub static MAX_COLOR_TEXTURE_SAMPLES: GLenum = 0x910E;
pub static MAX_DEPTH_TEXTURE_SAMPLES: GLenum = 0x910F;
pub static MAX_INTEGER_SAMPLES: GLenum = 0x9110;
pub static MAX_SERVER_WAIT_TIMEOUT: GLenum = 0x9111;
pub static OBJECT_TYPE: GLenum = 0x9112;
pub static SYNC_CONDITION: GLenum = 0x9113;
pub static SYNC_STATUS: GLenum = 0x9114;
pub static SYNC_FLAGS: GLenum = 0x9115;
pub static SYNC_FENCE: GLenum = 0x9116;
pub static SYNC_GPU_COMMANDS_COMPLETE: GLenum = 0x9117;
pub static UNSIGNALED: GLenum = 0x9118;
pub static SIGNALED: GLenum = 0x9119;
pub static ALREADY_SIGNALED: GLenum = 0x911A;
pub static TIMEOUT_EXPIRED: GLenum = 0x911B;
pub static CONDITION_SATISFIED: GLenum = 0x911C;
pub static WAIT_FAILED: GLenum = 0x911D;
pub static BUFFER_ACCESS_FLAGS: GLenum = 0x911F;
pub static BUFFER_MAP_LENGTH: GLenum = 0x9120;
pub static BUFFER_MAP_OFFSET: GLenum = 0x9121;
pub static MAX_VERTEX_OUTPUT_COMPONENTS: GLenum = 0x9122;
pub static MAX_GEOMETRY_INPUT_COMPONENTS: GLenum = 0x9123;
pub static MAX_GEOMETRY_OUTPUT_COMPONENTS: GLenum = 0x9124;
pub static MAX_FRAGMENT_INPUT_COMPONENTS: GLenum = 0x9125;
pub static CONTEXT_PROFILE_MASK: GLenum = 0x9126;
pub static UNPACK_COMPRESSED_BLOCK_WIDTH: GLenum = 0x9127;
pub static UNPACK_COMPRESSED_BLOCK_HEIGHT: GLenum = 0x9128;
pub static UNPACK_COMPRESSED_BLOCK_DEPTH: GLenum = 0x9129;
pub static UNPACK_COMPRESSED_BLOCK_SIZE: GLenum = 0x912A;
pub static PACK_COMPRESSED_BLOCK_WIDTH: GLenum = 0x912B;
pub static PACK_COMPRESSED_BLOCK_HEIGHT: GLenum = 0x912C;
pub static PACK_COMPRESSED_BLOCK_DEPTH: GLenum = 0x912D;
pub static PACK_COMPRESSED_BLOCK_SIZE: GLenum = 0x912E;
pub static TEXTURE_IMMUTABLE_FORMAT: GLenum = 0x912F;
pub static MAX_DEBUG_MESSAGE_LENGTH: GLenum = 0x9143;
pub static MAX_DEBUG_LOGGED_MESSAGES: GLenum = 0x9144;
pub static DEBUG_LOGGED_MESSAGES: GLenum = 0x9145;
pub static DEBUG_SEVERITY_HIGH: GLenum = 0x9146;
pub static DEBUG_SEVERITY_MEDIUM: GLenum = 0x9147;
pub static DEBUG_SEVERITY_LOW: GLenum = 0x9148;
pub static TEXTURE_BUFFER_OFFSET: GLenum = 0x919D;
pub static TEXTURE_BUFFER_SIZE: GLenum = 0x919E;
pub static TEXTURE_BUFFER_OFFSET_ALIGNMENT: GLenum = 0x919F;
pub static COMPUTE_SHADER: GLenum = 0x91B9;
pub static MAX_COMPUTE_UNIFORM_BLOCKS: GLenum = 0x91BB;
pub static MAX_COMPUTE_TEXTURE_IMAGE_UNITS: GLenum = 0x91BC;
pub static MAX_COMPUTE_IMAGE_UNIFORMS: GLenum = 0x91BD;
pub static MAX_COMPUTE_WORK_GROUP_COUNT: GLenum = 0x91BE;
pub static MAX_COMPUTE_WORK_GROUP_SIZE: GLenum = 0x91BF;
pub static COMPRESSED_R11_EAC: GLenum = 0x9270;
pub static COMPRESSED_SIGNED_R11_EAC: GLenum = 0x9271;
pub static COMPRESSED_RG11_EAC: GLenum = 0x9272;
pub static COMPRESSED_SIGNED_RG11_EAC: GLenum = 0x9273;
pub static COMPRESSED_RGB8_ETC2: GLenum = 0x9274;
pub static COMPRESSED_SRGB8_ETC2: GLenum = 0x9275;
pub static COMPRESSED_RGB8_PUNCHTHROUGH_ALPHA1_ETC2: GLenum = 0x9276;
pub static COMPRESSED_SRGB8_PUNCHTHROUGH_ALPHA1_ETC2: GLenum = 0x9277;
pub static COMPRESSED_RGBA8_ETC2_EAC: GLenum = 0x9278;
pub static COMPRESSED_SRGB8_ALPHA8_ETC2_EAC: GLenum = 0x9279;
pub static ATOMIC_COUNTER_BUFFER: GLenum = 0x92C0;
pub static ATOMIC_COUNTER_BUFFER_BINDING: GLenum = 0x92C1;
pub static ATOMIC_COUNTER_BUFFER_START: GLenum = 0x92C2;
pub static ATOMIC_COUNTER_BUFFER_SIZE: GLenum = 0x92C3;
pub static ATOMIC_COUNTER_BUFFER_DATA_SIZE: GLenum = 0x92C4;
pub static ATOMIC_COUNTER_BUFFER_ACTIVE_ATOMIC_COUNTERS: GLenum = 0x92C5;
pub static ATOMIC_COUNTER_BUFFER_ACTIVE_ATOMIC_COUNTER_INDICES: GLenum = 0x92C6;
pub static ATOMIC_COUNTER_BUFFER_REFERENCED_BY_VERTEX_SHADER: GLenum = 0x92C7;
pub static ATOMIC_COUNTER_BUFFER_REFERENCED_BY_TESS_CONTROL_SHADER: GLenum = 0x92C8;
pub static ATOMIC_COUNTER_BUFFER_REFERENCED_BY_TESS_EVALUATION_SHADER: GLenum = 0x92C9;
pub static ATOMIC_COUNTER_BUFFER_REFERENCED_BY_GEOMETRY_SHADER: GLenum = 0x92CA;
pub static ATOMIC_COUNTER_BUFFER_REFERENCED_BY_FRAGMENT_SHADER: GLenum = 0x92CB;
pub static MAX_VERTEX_ATOMIC_COUNTER_BUFFERS: GLenum = 0x92CC;
pub static MAX_TESS_CONTROL_ATOMIC_COUNTER_BUFFERS: GLenum = 0x92CD;
pub static MAX_TESS_EVALUATION_ATOMIC_COUNTER_BUFFERS: GLenum = 0x92CE;
pub static MAX_GEOMETRY_ATOMIC_COUNTER_BUFFERS: GLenum = 0x92CF;
pub static MAX_FRAGMENT_ATOMIC_COUNTER_BUFFERS: GLenum = 0x92D0;
pub static MAX_COMBINED_ATOMIC_COUNTER_BUFFERS: GLenum = 0x92D1;
pub static MAX_VERTEX_ATOMIC_COUNTERS: GLenum = 0x92D2;
pub static MAX_TESS_CONTROL_ATOMIC_COUNTERS: GLenum = 0x92D3;
pub static MAX_TESS_EVALUATION_ATOMIC_COUNTERS: GLenum = 0x92D4;
pub static MAX_GEOMETRY_ATOMIC_COUNTERS: GLenum = 0x92D5;
pub static MAX_FRAGMENT_ATOMIC_COUNTERS: GLenum = 0x92D6;
pub static MAX_COMBINED_ATOMIC_COUNTERS: GLenum = 0x92D7;
pub static MAX_ATOMIC_COUNTER_BUFFER_SIZE: GLenum = 0x92D8;
pub static ACTIVE_ATOMIC_COUNTER_BUFFERS: GLenum = 0x92D9;
pub static UNIFORM_ATOMIC_COUNTER_BUFFER_INDEX: GLenum = 0x92DA;
pub static UNSIGNED_INT_ATOMIC_COUNTER: GLenum = 0x92DB;
pub static MAX_ATOMIC_COUNTER_BUFFER_BINDINGS: GLenum = 0x92DC;
pub static DEBUG_OUTPUT: GLenum = 0x92E0;
pub static UNIFORM: GLenum = 0x92E1;
pub static UNIFORM_BLOCK: GLenum = 0x92E2;
pub static PROGRAM_INPUT: GLenum = 0x92E3;
pub static PROGRAM_OUTPUT: GLenum = 0x92E4;
pub static BUFFER_VARIABLE: GLenum = 0x92E5;
pub static SHADER_STORAGE_BLOCK: GLenum = 0x92E6;
pub static IS_PER_PATCH: GLenum = 0x92E7;
pub static VERTEX_SUBROUTINE: GLenum = 0x92E8;
pub static TESS_CONTROL_SUBROUTINE: GLenum = 0x92E9;
pub static TESS_EVALUATION_SUBROUTINE: GLenum = 0x92EA;
pub static GEOMETRY_SUBROUTINE: GLenum = 0x92EB;
pub static FRAGMENT_SUBROUTINE: GLenum = 0x92EC;
pub static COMPUTE_SUBROUTINE: GLenum = 0x92ED;
pub static VERTEX_SUBROUTINE_UNIFORM: GLenum = 0x92EE;
pub static TESS_CONTROL_SUBROUTINE_UNIFORM: GLenum = 0x92EF;
pub static TESS_EVALUATION_SUBROUTINE_UNIFORM: GLenum = 0x92F0;
pub static GEOMETRY_SUBROUTINE_UNIFORM: GLenum = 0x92F1;
pub static FRAGMENT_SUBROUTINE_UNIFORM: GLenum = 0x92F2;
pub static COMPUTE_SUBROUTINE_UNIFORM: GLenum = 0x92F3;
pub static TRANSFORM_FEEDBACK_VARYING: GLenum = 0x92F4;
pub static ACTIVE_RESOURCES: GLenum = 0x92F5;
pub static MAX_NAME_LENGTH: GLenum = 0x92F6;
pub static MAX_NUM_ACTIVE_VARIABLES: GLenum = 0x92F7;
pub static MAX_NUM_COMPATIBLE_SUBROUTINES: GLenum = 0x92F8;
pub static NAME_LENGTH: GLenum = 0x92F9;
pub static TYPE: GLenum = 0x92FA;
pub static ARRAY_SIZE: GLenum = 0x92FB;
pub static OFFSET: GLenum = 0x92FC;
pub static BLOCK_INDEX: GLenum = 0x92FD;
pub static ARRAY_STRIDE: GLenum = 0x92FE;
pub static MATRIX_STRIDE: GLenum = 0x92FF;
pub static IS_ROW_MAJOR: GLenum = 0x9300;
pub static ATOMIC_COUNTER_BUFFER_INDEX: GLenum = 0x9301;
pub static BUFFER_BINDING: GLenum = 0x9302;
pub static BUFFER_DATA_SIZE: GLenum = 0x9303;
pub static NUM_ACTIVE_VARIABLES: GLenum = 0x9304;
pub static ACTIVE_VARIABLES: GLenum = 0x9305;
pub static REFERENCED_BY_VERTEX_SHADER: GLenum = 0x9306;
pub static REFERENCED_BY_TESS_CONTROL_SHADER: GLenum = 0x9307;
pub static REFERENCED_BY_TESS_EVALUATION_SHADER: GLenum = 0x9308;
pub static REFERENCED_BY_GEOMETRY_SHADER: GLenum = 0x9309;
pub static REFERENCED_BY_FRAGMENT_SHADER: GLenum = 0x930A;
pub static REFERENCED_BY_COMPUTE_SHADER: GLenum = 0x930B;
pub static TOP_LEVEL_ARRAY_SIZE: GLenum = 0x930C;
pub static TOP_LEVEL_ARRAY_STRIDE: GLenum = 0x930D;
pub static LOCATION: GLenum = 0x930E;
pub static LOCATION_INDEX: GLenum = 0x930F;
pub static FRAMEBUFFER_DEFAULT_WIDTH: GLenum = 0x9310;
pub static FRAMEBUFFER_DEFAULT_HEIGHT: GLenum = 0x9311;
pub static FRAMEBUFFER_DEFAULT_LAYERS: GLenum = 0x9312;
pub static FRAMEBUFFER_DEFAULT_SAMPLES: GLenum = 0x9313;
pub static FRAMEBUFFER_DEFAULT_FIXED_SAMPLE_LOCATIONS: GLenum = 0x9314;
pub static MAX_FRAMEBUFFER_WIDTH: GLenum = 0x9315;
pub static MAX_FRAMEBUFFER_HEIGHT: GLenum = 0x9316;
pub static MAX_FRAMEBUFFER_LAYERS: GLenum = 0x9317;
pub static MAX_FRAMEBUFFER_SAMPLES: GLenum = 0x9318;
pub static NUM_SAMPLE_COUNTS: GLenum = 0x9380;

#[inline] pub fn ActiveShaderProgram(pipeline: GLuint, program: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint)>(storage::ActiveShaderProgram.f)(pipeline, program) } }
#[inline] pub fn ActiveTexture(texture: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::ActiveTexture.f)(texture) } }
#[inline] pub fn AttachShader(program: GLuint, shader: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint)>(storage::AttachShader.f)(program, shader) } }
#[inline] pub fn BeginConditionalRender(id: GLuint, mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum)>(storage::BeginConditionalRender.f)(id, mode) } }
#[inline] pub fn BeginQuery(target: GLenum, id: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::BeginQuery.f)(target, id) } }
#[inline] pub fn BeginQueryIndexed(target: GLenum, index: GLuint, id: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint, GLuint)>(storage::BeginQueryIndexed.f)(target, index, id) } }
#[inline] pub fn BeginTransformFeedback(primitiveMode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::BeginTransformFeedback.f)(primitiveMode) } }
#[inline] pub unsafe fn BindAttribLocation(program: GLuint, index: GLuint, name: *const GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, index: GLuint, name: *const GLchar) >(storage::BindAttribLocation.f)(program, index, name) }
#[inline] pub fn BindBuffer(target: GLenum, buffer: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::BindBuffer.f)(target, buffer) } }
#[inline] pub fn BindBufferBase(target: GLenum, index: GLuint, buffer: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint, GLuint)>(storage::BindBufferBase.f)(target, index, buffer) } }
#[inline] pub fn BindBufferRange(target: GLenum, index: GLuint, buffer: GLuint, offset: GLintptr, size: GLsizeiptr) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint, GLuint, GLintptr, GLsizeiptr)>(storage::BindBufferRange.f)(target, index, buffer, offset, size) } }
#[inline] pub unsafe fn BindFragDataLocation(program: GLuint, color: GLuint, name: *const GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, color: GLuint, name: *const GLchar) >(storage::BindFragDataLocation.f)(program, color, name) }
#[inline] pub unsafe fn BindFragDataLocationIndexed(program: GLuint, colorNumber: GLuint, index: GLuint, name: *const GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, colorNumber: GLuint, index: GLuint, name: *const GLchar) >(storage::BindFragDataLocationIndexed.f)(program, colorNumber, index, name) }
#[inline] pub fn BindFramebuffer(target: GLenum, framebuffer: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::BindFramebuffer.f)(target, framebuffer) } }
#[inline] pub fn BindImageTexture(unit: GLuint, texture: GLuint, level: GLint, layered: GLboolean, layer: GLint, access: GLenum, format: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint, GLint, GLboolean, GLint, GLenum, GLenum)>(storage::BindImageTexture.f)(unit, texture, level, layered, layer, access, format) } }
#[inline] pub fn BindProgramPipeline(pipeline: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::BindProgramPipeline.f)(pipeline) } }
#[inline] pub fn BindRenderbuffer(target: GLenum, renderbuffer: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::BindRenderbuffer.f)(target, renderbuffer) } }
#[inline] pub fn BindSampler(unit: GLuint, sampler: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint)>(storage::BindSampler.f)(unit, sampler) } }
#[inline] pub fn BindTexture(target: GLenum, texture: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::BindTexture.f)(target, texture) } }
#[inline] pub fn BindTransformFeedback(target: GLenum, id: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::BindTransformFeedback.f)(target, id) } }
#[inline] pub fn BindVertexArray(array: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::BindVertexArray.f)(array) } }
#[inline] pub fn BindVertexBuffer(bindingindex: GLuint, buffer: GLuint, offset: GLintptr, stride: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint, GLintptr, GLsizei)>(storage::BindVertexBuffer.f)(bindingindex, buffer, offset, stride) } }
#[inline] pub fn BlendColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat, GLfloat, GLfloat, GLfloat)>(storage::BlendColor.f)(red, green, blue, alpha) } }
#[inline] pub fn BlendEquation(mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::BlendEquation.f)(mode) } }
#[inline] pub fn BlendEquationSeparate(modeRGB: GLenum, modeAlpha: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum)>(storage::BlendEquationSeparate.f)(modeRGB, modeAlpha) } }
#[inline] pub fn BlendEquationSeparatei(buf: GLuint, modeRGB: GLenum, modeAlpha: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLenum)>(storage::BlendEquationSeparatei.f)(buf, modeRGB, modeAlpha) } }
#[inline] pub fn BlendEquationi(buf: GLuint, mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum)>(storage::BlendEquationi.f)(buf, mode) } }
#[inline] pub fn BlendFunc(sfactor: GLenum, dfactor: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum)>(storage::BlendFunc.f)(sfactor, dfactor) } }
#[inline] pub fn BlendFuncSeparate(sfactorRGB: GLenum, dfactorRGB: GLenum, sfactorAlpha: GLenum, dfactorAlpha: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLenum, GLenum)>(storage::BlendFuncSeparate.f)(sfactorRGB, dfactorRGB, sfactorAlpha, dfactorAlpha) } }
#[inline] pub fn BlendFuncSeparatei(buf: GLuint, srcRGB: GLenum, dstRGB: GLenum, srcAlpha: GLenum, dstAlpha: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLenum, GLenum, GLenum)>(storage::BlendFuncSeparatei.f)(buf, srcRGB, dstRGB, srcAlpha, dstAlpha) } }
#[inline] pub fn BlendFunci(buf: GLuint, src: GLenum, dst: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLenum)>(storage::BlendFunci.f)(buf, src, dst) } }
#[inline] pub fn BlitFramebuffer(srcX0: GLint, srcY0: GLint, srcX1: GLint, srcY1: GLint, dstX0: GLint, dstY0: GLint, dstX1: GLint, dstY1: GLint, mask: GLbitfield, filter: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLint, GLint, GLint, GLint, GLint, GLint, GLint, GLbitfield, GLenum)>(storage::BlitFramebuffer.f)(srcX0, srcY0, srcX1, srcY1, dstX0, dstY0, dstX1, dstY1, mask, filter) } }
#[inline] pub unsafe fn BufferData(target: GLenum, size: GLsizeiptr, data: *const c_void, usage: GLenum) { mem::transmute::<_, extern "system" fn(target: GLenum, size: GLsizeiptr, data: *const c_void, usage: GLenum) >(storage::BufferData.f)(target, size, data, usage) }
#[inline] pub unsafe fn BufferSubData(target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *const c_void) >(storage::BufferSubData.f)(target, offset, size, data) }
#[inline] pub fn CheckFramebufferStatus(target: GLenum) -> GLenum { unsafe { mem::transmute::<_, extern "system" fn(GLenum) -> GLenum>(storage::CheckFramebufferStatus.f)(target) } }
#[inline] pub fn ClampColor(target: GLenum, clamp: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum)>(storage::ClampColor.f)(target, clamp) } }
#[inline] pub fn Clear(mask: GLbitfield) { unsafe { mem::transmute::<_, extern "system" fn(GLbitfield)>(storage::Clear.f)(mask) } }
#[inline] pub unsafe fn ClearBufferData(target: GLenum, internalformat: GLenum, format: GLenum, type_: GLenum, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, internalformat: GLenum, format: GLenum, type_: GLenum, data: *const c_void) >(storage::ClearBufferData.f)(target, internalformat, format, type_, data) }
#[inline] pub unsafe fn ClearBufferSubData(target: GLenum, internalformat: GLenum, offset: GLintptr, size: GLsizeiptr, format: GLenum, type_: GLenum, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, internalformat: GLenum, offset: GLintptr, size: GLsizeiptr, format: GLenum, type_: GLenum, data: *const c_void) >(storage::ClearBufferSubData.f)(target, internalformat, offset, size, format, type_, data) }
#[inline] pub fn ClearBufferfi(buffer: GLenum, drawbuffer: GLint, depth: GLfloat, stencil: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLfloat, GLint)>(storage::ClearBufferfi.f)(buffer, drawbuffer, depth, stencil) } }
#[inline] pub unsafe fn ClearBufferfv(buffer: GLenum, drawbuffer: GLint, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(buffer: GLenum, drawbuffer: GLint, value: *const GLfloat) >(storage::ClearBufferfv.f)(buffer, drawbuffer, value) }
#[inline] pub unsafe fn ClearBufferiv(buffer: GLenum, drawbuffer: GLint, value: *const GLint) { mem::transmute::<_, extern "system" fn(buffer: GLenum, drawbuffer: GLint, value: *const GLint) >(storage::ClearBufferiv.f)(buffer, drawbuffer, value) }
#[inline] pub unsafe fn ClearBufferuiv(buffer: GLenum, drawbuffer: GLint, value: *const GLuint) { mem::transmute::<_, extern "system" fn(buffer: GLenum, drawbuffer: GLint, value: *const GLuint) >(storage::ClearBufferuiv.f)(buffer, drawbuffer, value) }
#[inline] pub fn ClearColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat, GLfloat, GLfloat, GLfloat)>(storage::ClearColor.f)(red, green, blue, alpha) } }
#[inline] pub fn ClearDepth(depth: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLdouble)>(storage::ClearDepth.f)(depth) } }
#[inline] pub fn ClearDepthf(d: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat)>(storage::ClearDepthf.f)(d) } }
#[inline] pub fn ClearStencil(s: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLint)>(storage::ClearStencil.f)(s) } }
#[inline] pub fn ClientWaitSync(sync: GLsync, flags: GLbitfield, timeout: GLuint64) -> GLenum { unsafe { mem::transmute::<_, extern "system" fn(GLsync, GLbitfield, GLuint64) -> GLenum>(storage::ClientWaitSync.f)(sync, flags, timeout) } }
#[inline] pub fn ColorMask(red: GLboolean, green: GLboolean, blue: GLboolean, alpha: GLboolean) { unsafe { mem::transmute::<_, extern "system" fn(GLboolean, GLboolean, GLboolean, GLboolean)>(storage::ColorMask.f)(red, green, blue, alpha) } }
#[inline] pub fn ColorMaski(index: GLuint, r: GLboolean, g: GLboolean, b: GLboolean, a: GLboolean) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLboolean, GLboolean, GLboolean, GLboolean)>(storage::ColorMaski.f)(index, r, g, b, a) } }
#[inline] pub fn ColorP3ui(type_: GLenum, color: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::ColorP3ui.f)(type_, color) } }
#[inline] pub unsafe fn ColorP3uiv(type_: GLenum, color: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, color: *const GLuint) >(storage::ColorP3uiv.f)(type_, color) }
#[inline] pub fn ColorP4ui(type_: GLenum, color: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::ColorP4ui.f)(type_, color) } }
#[inline] pub unsafe fn ColorP4uiv(type_: GLenum, color: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, color: *const GLuint) >(storage::ColorP4uiv.f)(type_, color) }
#[inline] pub fn CompileShader(shader: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::CompileShader.f)(shader) } }
#[inline] pub unsafe fn CompressedTexImage1D(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) >(storage::CompressedTexImage1D.f)(target, level, internalformat, width, border, imageSize, data) }
#[inline] pub unsafe fn CompressedTexImage2D(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, height: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, height: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) >(storage::CompressedTexImage2D.f)(target, level, internalformat, width, height, border, imageSize, data) }
#[inline] pub unsafe fn CompressedTexImage3D(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) >(storage::CompressedTexImage3D.f)(target, level, internalformat, width, height, depth, border, imageSize, data) }
#[inline] pub unsafe fn CompressedTexSubImage1D(target: GLenum, level: GLint, xoffset: GLint, width: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, xoffset: GLint, width: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) >(storage::CompressedTexSubImage1D.f)(target, level, xoffset, width, format, imageSize, data) }
#[inline] pub unsafe fn CompressedTexSubImage2D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, width: GLsizei, height: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, width: GLsizei, height: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) >(storage::CompressedTexSubImage2D.f)(target, level, xoffset, yoffset, width, height, format, imageSize, data) }
#[inline] pub unsafe fn CompressedTexSubImage3D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) >(storage::CompressedTexSubImage3D.f)(target, level, xoffset, yoffset, zoffset, width, height, depth, format, imageSize, data) }
#[inline] pub fn CopyBufferSubData(readTarget: GLenum, writeTarget: GLenum, readOffset: GLintptr, writeOffset: GLintptr, size: GLsizeiptr) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLintptr, GLintptr, GLsizeiptr)>(storage::CopyBufferSubData.f)(readTarget, writeTarget, readOffset, writeOffset, size) } }
#[inline] pub fn CopyImageSubData(srcName: GLuint, srcTarget: GLenum, srcLevel: GLint, srcX: GLint, srcY: GLint, srcZ: GLint, dstName: GLuint, dstTarget: GLenum, dstLevel: GLint, dstX: GLint, dstY: GLint, dstZ: GLint, srcWidth: GLsizei, srcHeight: GLsizei, srcDepth: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLint, GLint, GLint, GLint, GLuint, GLenum, GLint, GLint, GLint, GLint, GLsizei, GLsizei, GLsizei)>(storage::CopyImageSubData.f)(srcName, srcTarget, srcLevel, srcX, srcY, srcZ, dstName, dstTarget, dstLevel, dstX, dstY, dstZ, srcWidth, srcHeight, srcDepth) } }
#[inline] pub fn CopyTexImage1D(target: GLenum, level: GLint, internalformat: GLenum, x: GLint, y: GLint, width: GLsizei, border: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLenum, GLint, GLint, GLsizei, GLint)>(storage::CopyTexImage1D.f)(target, level, internalformat, x, y, width, border) } }
#[inline] pub fn CopyTexImage2D(target: GLenum, level: GLint, internalformat: GLenum, x: GLint, y: GLint, width: GLsizei, height: GLsizei, border: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLenum, GLint, GLint, GLsizei, GLsizei, GLint)>(storage::CopyTexImage2D.f)(target, level, internalformat, x, y, width, height, border) } }
#[inline] pub fn CopyTexSubImage1D(target: GLenum, level: GLint, xoffset: GLint, x: GLint, y: GLint, width: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLint, GLint, GLint, GLsizei)>(storage::CopyTexSubImage1D.f)(target, level, xoffset, x, y, width) } }
#[inline] pub fn CopyTexSubImage2D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, x: GLint, y: GLint, width: GLsizei, height: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLint, GLint, GLint, GLint, GLsizei, GLsizei)>(storage::CopyTexSubImage2D.f)(target, level, xoffset, yoffset, x, y, width, height) } }
#[inline] pub fn CopyTexSubImage3D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, x: GLint, y: GLint, width: GLsizei, height: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLint, GLint, GLint, GLint, GLint, GLsizei, GLsizei)>(storage::CopyTexSubImage3D.f)(target, level, xoffset, yoffset, zoffset, x, y, width, height) } }
#[inline] pub fn CreateProgram() -> GLuint { unsafe { mem::transmute::<_, extern "system" fn() -> GLuint>(storage::CreateProgram.f)() } }
#[inline] pub fn CreateShader(type_: GLenum) -> GLuint { unsafe { mem::transmute::<_, extern "system" fn(GLenum) -> GLuint>(storage::CreateShader.f)(type_) } }
#[inline] pub unsafe fn CreateShaderProgramv(type_: GLenum, count: GLsizei, strings: *const *const GLchar) -> GLuint { mem::transmute::<_, extern "system" fn(type_: GLenum, count: GLsizei, strings: *const *const GLchar)  -> GLuint>(storage::CreateShaderProgramv.f)(type_, count, strings) }
#[inline] pub fn CullFace(mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::CullFace.f)(mode) } }
#[inline] pub unsafe fn DebugMessageCallback(callback: GLDEBUGPROC, userParam: *const c_void) { mem::transmute::<_, extern "system" fn(callback: GLDEBUGPROC, userParam: *const c_void) >(storage::DebugMessageCallback.f)(callback, userParam) }
#[inline] pub unsafe fn DebugMessageControl(source: GLenum, type_: GLenum, severity: GLenum, count: GLsizei, ids: *const GLuint, enabled: GLboolean) { mem::transmute::<_, extern "system" fn(source: GLenum, type_: GLenum, severity: GLenum, count: GLsizei, ids: *const GLuint, enabled: GLboolean) >(storage::DebugMessageControl.f)(source, type_, severity, count, ids, enabled) }
#[inline] pub unsafe fn DebugMessageInsert(source: GLenum, type_: GLenum, id: GLuint, severity: GLenum, length: GLsizei, buf: *const GLchar) { mem::transmute::<_, extern "system" fn(source: GLenum, type_: GLenum, id: GLuint, severity: GLenum, length: GLsizei, buf: *const GLchar) >(storage::DebugMessageInsert.f)(source, type_, id, severity, length, buf) }
#[inline] pub unsafe fn DeleteBuffers(n: GLsizei, buffers: *const GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, buffers: *const GLuint) >(storage::DeleteBuffers.f)(n, buffers) }
#[inline] pub unsafe fn DeleteFramebuffers(n: GLsizei, framebuffers: *const GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, framebuffers: *const GLuint) >(storage::DeleteFramebuffers.f)(n, framebuffers) }
#[inline] pub fn DeleteProgram(program: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::DeleteProgram.f)(program) } }
#[inline] pub unsafe fn DeleteProgramPipelines(n: GLsizei, pipelines: *const GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, pipelines: *const GLuint) >(storage::DeleteProgramPipelines.f)(n, pipelines) }
#[inline] pub unsafe fn DeleteQueries(n: GLsizei, ids: *const GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, ids: *const GLuint) >(storage::DeleteQueries.f)(n, ids) }
#[inline] pub unsafe fn DeleteRenderbuffers(n: GLsizei, renderbuffers: *const GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, renderbuffers: *const GLuint) >(storage::DeleteRenderbuffers.f)(n, renderbuffers) }
#[inline] pub unsafe fn DeleteSamplers(count: GLsizei, samplers: *const GLuint) { mem::transmute::<_, extern "system" fn(count: GLsizei, samplers: *const GLuint) >(storage::DeleteSamplers.f)(count, samplers) }
#[inline] pub fn DeleteShader(shader: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::DeleteShader.f)(shader) } }
#[inline] pub fn DeleteSync(sync: GLsync) { unsafe { mem::transmute::<_, extern "system" fn(GLsync)>(storage::DeleteSync.f)(sync) } }
#[inline] pub unsafe fn DeleteTextures(n: GLsizei, textures: *const GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, textures: *const GLuint) >(storage::DeleteTextures.f)(n, textures) }
#[inline] pub unsafe fn DeleteTransformFeedbacks(n: GLsizei, ids: *const GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, ids: *const GLuint) >(storage::DeleteTransformFeedbacks.f)(n, ids) }
#[inline] pub unsafe fn DeleteVertexArrays(n: GLsizei, arrays: *const GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, arrays: *const GLuint) >(storage::DeleteVertexArrays.f)(n, arrays) }
#[inline] pub fn DepthFunc(func: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::DepthFunc.f)(func) } }
#[inline] pub fn DepthMask(flag: GLboolean) { unsafe { mem::transmute::<_, extern "system" fn(GLboolean)>(storage::DepthMask.f)(flag) } }
#[inline] pub fn DepthRange(near: GLdouble, far: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLdouble, GLdouble)>(storage::DepthRange.f)(near, far) } }
#[inline] pub unsafe fn DepthRangeArrayv(first: GLuint, count: GLsizei, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(first: GLuint, count: GLsizei, v: *const GLdouble) >(storage::DepthRangeArrayv.f)(first, count, v) }
#[inline] pub fn DepthRangeIndexed(index: GLuint, n: GLdouble, f: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble, GLdouble)>(storage::DepthRangeIndexed.f)(index, n, f) } }
#[inline] pub fn DepthRangef(n: GLfloat, f: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat, GLfloat)>(storage::DepthRangef.f)(n, f) } }
#[inline] pub fn DetachShader(program: GLuint, shader: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint)>(storage::DetachShader.f)(program, shader) } }
#[inline] pub fn Disable(cap: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::Disable.f)(cap) } }
#[inline] pub fn DisableVertexAttribArray(index: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::DisableVertexAttribArray.f)(index) } }
#[inline] pub fn Disablei(target: GLenum, index: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::Disablei.f)(target, index) } }
#[inline] pub fn DispatchCompute(num_groups_x: GLuint, num_groups_y: GLuint, num_groups_z: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint, GLuint)>(storage::DispatchCompute.f)(num_groups_x, num_groups_y, num_groups_z) } }
#[inline] pub fn DispatchComputeIndirect(indirect: GLintptr) { unsafe { mem::transmute::<_, extern "system" fn(GLintptr)>(storage::DispatchComputeIndirect.f)(indirect) } }
#[inline] pub fn DrawArrays(mode: GLenum, first: GLint, count: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLsizei)>(storage::DrawArrays.f)(mode, first, count) } }
#[inline] pub unsafe fn DrawArraysIndirect(mode: GLenum, indirect: *const c_void) { mem::transmute::<_, extern "system" fn(mode: GLenum, indirect: *const c_void) >(storage::DrawArraysIndirect.f)(mode, indirect) }
#[inline] pub fn DrawArraysInstanced(mode: GLenum, first: GLint, count: GLsizei, instancecount: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLsizei, GLsizei)>(storage::DrawArraysInstanced.f)(mode, first, count, instancecount) } }
#[inline] pub fn DrawArraysInstancedBaseInstance(mode: GLenum, first: GLint, count: GLsizei, instancecount: GLsizei, baseinstance: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLsizei, GLsizei, GLuint)>(storage::DrawArraysInstancedBaseInstance.f)(mode, first, count, instancecount, baseinstance) } }
#[inline] pub fn DrawBuffer(mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::DrawBuffer.f)(mode) } }
#[inline] pub unsafe fn DrawBuffers(n: GLsizei, bufs: *const GLenum) { mem::transmute::<_, extern "system" fn(n: GLsizei, bufs: *const GLenum) >(storage::DrawBuffers.f)(n, bufs) }
#[inline] pub unsafe fn DrawElements(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void) { mem::transmute::<_, extern "system" fn(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void) >(storage::DrawElements.f)(mode, count, type_, indices) }
#[inline] pub unsafe fn DrawElementsBaseVertex(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, basevertex: GLint) { mem::transmute::<_, extern "system" fn(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, basevertex: GLint) >(storage::DrawElementsBaseVertex.f)(mode, count, type_, indices, basevertex) }
#[inline] pub unsafe fn DrawElementsIndirect(mode: GLenum, type_: GLenum, indirect: *const c_void) { mem::transmute::<_, extern "system" fn(mode: GLenum, type_: GLenum, indirect: *const c_void) >(storage::DrawElementsIndirect.f)(mode, type_, indirect) }
#[inline] pub unsafe fn DrawElementsInstanced(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei) { mem::transmute::<_, extern "system" fn(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei) >(storage::DrawElementsInstanced.f)(mode, count, type_, indices, instancecount) }
#[inline] pub unsafe fn DrawElementsInstancedBaseInstance(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, baseinstance: GLuint) { mem::transmute::<_, extern "system" fn(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, baseinstance: GLuint) >(storage::DrawElementsInstancedBaseInstance.f)(mode, count, type_, indices, instancecount, baseinstance) }
#[inline] pub unsafe fn DrawElementsInstancedBaseVertex(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, basevertex: GLint) { mem::transmute::<_, extern "system" fn(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, basevertex: GLint) >(storage::DrawElementsInstancedBaseVertex.f)(mode, count, type_, indices, instancecount, basevertex) }
#[inline] pub unsafe fn DrawElementsInstancedBaseVertexBaseInstance(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, basevertex: GLint, baseinstance: GLuint) { mem::transmute::<_, extern "system" fn(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, basevertex: GLint, baseinstance: GLuint) >(storage::DrawElementsInstancedBaseVertexBaseInstance.f)(mode, count, type_, indices, instancecount, basevertex, baseinstance) }
#[inline] pub unsafe fn DrawRangeElements(mode: GLenum, start: GLuint, end: GLuint, count: GLsizei, type_: GLenum, indices: *const c_void) { mem::transmute::<_, extern "system" fn(mode: GLenum, start: GLuint, end: GLuint, count: GLsizei, type_: GLenum, indices: *const c_void) >(storage::DrawRangeElements.f)(mode, start, end, count, type_, indices) }
#[inline] pub unsafe fn DrawRangeElementsBaseVertex(mode: GLenum, start: GLuint, end: GLuint, count: GLsizei, type_: GLenum, indices: *const c_void, basevertex: GLint) { mem::transmute::<_, extern "system" fn(mode: GLenum, start: GLuint, end: GLuint, count: GLsizei, type_: GLenum, indices: *const c_void, basevertex: GLint) >(storage::DrawRangeElementsBaseVertex.f)(mode, start, end, count, type_, indices, basevertex) }
#[inline] pub fn DrawTransformFeedback(mode: GLenum, id: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::DrawTransformFeedback.f)(mode, id) } }
#[inline] pub fn DrawTransformFeedbackInstanced(mode: GLenum, id: GLuint, instancecount: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint, GLsizei)>(storage::DrawTransformFeedbackInstanced.f)(mode, id, instancecount) } }
#[inline] pub fn DrawTransformFeedbackStream(mode: GLenum, id: GLuint, stream: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint, GLuint)>(storage::DrawTransformFeedbackStream.f)(mode, id, stream) } }
#[inline] pub fn DrawTransformFeedbackStreamInstanced(mode: GLenum, id: GLuint, stream: GLuint, instancecount: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint, GLuint, GLsizei)>(storage::DrawTransformFeedbackStreamInstanced.f)(mode, id, stream, instancecount) } }
#[inline] pub fn Enable(cap: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::Enable.f)(cap) } }
#[inline] pub fn EnableVertexAttribArray(index: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::EnableVertexAttribArray.f)(index) } }
#[inline] pub fn Enablei(target: GLenum, index: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::Enablei.f)(target, index) } }
#[inline] pub fn EndConditionalRender() { unsafe { mem::transmute::<_, extern "system" fn()>(storage::EndConditionalRender.f)() } }
#[inline] pub fn EndQuery(target: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::EndQuery.f)(target) } }
#[inline] pub fn EndQueryIndexed(target: GLenum, index: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::EndQueryIndexed.f)(target, index) } }
#[inline] pub fn EndTransformFeedback() { unsafe { mem::transmute::<_, extern "system" fn()>(storage::EndTransformFeedback.f)() } }
#[inline] pub fn FenceSync(condition: GLenum, flags: GLbitfield) -> GLsync { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLbitfield) -> GLsync>(storage::FenceSync.f)(condition, flags) } }
#[inline] pub fn Finish() { unsafe { mem::transmute::<_, extern "system" fn()>(storage::Finish.f)() } }
#[inline] pub fn Flush() { unsafe { mem::transmute::<_, extern "system" fn()>(storage::Flush.f)() } }
#[inline] pub fn FlushMappedBufferRange(target: GLenum, offset: GLintptr, length: GLsizeiptr) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLintptr, GLsizeiptr)>(storage::FlushMappedBufferRange.f)(target, offset, length) } }
#[inline] pub fn FramebufferParameteri(target: GLenum, pname: GLenum, param: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLint)>(storage::FramebufferParameteri.f)(target, pname, param) } }
#[inline] pub fn FramebufferRenderbuffer(target: GLenum, attachment: GLenum, renderbuffertarget: GLenum, renderbuffer: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLenum, GLuint)>(storage::FramebufferRenderbuffer.f)(target, attachment, renderbuffertarget, renderbuffer) } }
#[inline] pub fn FramebufferTexture(target: GLenum, attachment: GLenum, texture: GLuint, level: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLuint, GLint)>(storage::FramebufferTexture.f)(target, attachment, texture, level) } }
#[inline] pub fn FramebufferTexture1D(target: GLenum, attachment: GLenum, textarget: GLenum, texture: GLuint, level: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLenum, GLuint, GLint)>(storage::FramebufferTexture1D.f)(target, attachment, textarget, texture, level) } }
#[inline] pub fn FramebufferTexture2D(target: GLenum, attachment: GLenum, textarget: GLenum, texture: GLuint, level: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLenum, GLuint, GLint)>(storage::FramebufferTexture2D.f)(target, attachment, textarget, texture, level) } }
#[inline] pub fn FramebufferTexture3D(target: GLenum, attachment: GLenum, textarget: GLenum, texture: GLuint, level: GLint, zoffset: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLenum, GLuint, GLint, GLint)>(storage::FramebufferTexture3D.f)(target, attachment, textarget, texture, level, zoffset) } }
#[inline] pub fn FramebufferTextureLayer(target: GLenum, attachment: GLenum, texture: GLuint, level: GLint, layer: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLuint, GLint, GLint)>(storage::FramebufferTextureLayer.f)(target, attachment, texture, level, layer) } }
#[inline] pub fn FrontFace(mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::FrontFace.f)(mode) } }
#[inline] pub unsafe fn GenBuffers(n: GLsizei, buffers: *mut GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, buffers: *mut GLuint) >(storage::GenBuffers.f)(n, buffers) }
#[inline] pub unsafe fn GenFramebuffers(n: GLsizei, framebuffers: *mut GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, framebuffers: *mut GLuint) >(storage::GenFramebuffers.f)(n, framebuffers) }
#[inline] pub unsafe fn GenProgramPipelines(n: GLsizei, pipelines: *mut GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, pipelines: *mut GLuint) >(storage::GenProgramPipelines.f)(n, pipelines) }
#[inline] pub unsafe fn GenQueries(n: GLsizei, ids: *mut GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, ids: *mut GLuint) >(storage::GenQueries.f)(n, ids) }
#[inline] pub unsafe fn GenRenderbuffers(n: GLsizei, renderbuffers: *mut GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, renderbuffers: *mut GLuint) >(storage::GenRenderbuffers.f)(n, renderbuffers) }
#[inline] pub unsafe fn GenSamplers(count: GLsizei, samplers: *mut GLuint) { mem::transmute::<_, extern "system" fn(count: GLsizei, samplers: *mut GLuint) >(storage::GenSamplers.f)(count, samplers) }
#[inline] pub unsafe fn GenTextures(n: GLsizei, textures: *mut GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, textures: *mut GLuint) >(storage::GenTextures.f)(n, textures) }
#[inline] pub unsafe fn GenTransformFeedbacks(n: GLsizei, ids: *mut GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, ids: *mut GLuint) >(storage::GenTransformFeedbacks.f)(n, ids) }
#[inline] pub unsafe fn GenVertexArrays(n: GLsizei, arrays: *mut GLuint) { mem::transmute::<_, extern "system" fn(n: GLsizei, arrays: *mut GLuint) >(storage::GenVertexArrays.f)(n, arrays) }
#[inline] pub fn GenerateMipmap(target: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::GenerateMipmap.f)(target) } }
#[inline] pub unsafe fn GetActiveAtomicCounterBufferiv(program: GLuint, bufferIndex: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, bufferIndex: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetActiveAtomicCounterBufferiv.f)(program, bufferIndex, pname, params) }
#[inline] pub unsafe fn GetActiveAttrib(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLint, type_: *mut GLenum, name: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLint, type_: *mut GLenum, name: *mut GLchar) >(storage::GetActiveAttrib.f)(program, index, bufSize, length, size, type_, name) }
#[inline] pub unsafe fn GetActiveSubroutineName(program: GLuint, shadertype: GLenum, index: GLuint, bufsize: GLsizei, length: *mut GLsizei, name: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, shadertype: GLenum, index: GLuint, bufsize: GLsizei, length: *mut GLsizei, name: *mut GLchar) >(storage::GetActiveSubroutineName.f)(program, shadertype, index, bufsize, length, name) }
#[inline] pub unsafe fn GetActiveSubroutineUniformName(program: GLuint, shadertype: GLenum, index: GLuint, bufsize: GLsizei, length: *mut GLsizei, name: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, shadertype: GLenum, index: GLuint, bufsize: GLsizei, length: *mut GLsizei, name: *mut GLchar) >(storage::GetActiveSubroutineUniformName.f)(program, shadertype, index, bufsize, length, name) }
#[inline] pub unsafe fn GetActiveSubroutineUniformiv(program: GLuint, shadertype: GLenum, index: GLuint, pname: GLenum, values: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, shadertype: GLenum, index: GLuint, pname: GLenum, values: *mut GLint) >(storage::GetActiveSubroutineUniformiv.f)(program, shadertype, index, pname, values) }
#[inline] pub unsafe fn GetActiveUniform(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLint, type_: *mut GLenum, name: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLint, type_: *mut GLenum, name: *mut GLchar) >(storage::GetActiveUniform.f)(program, index, bufSize, length, size, type_, name) }
#[inline] pub unsafe fn GetActiveUniformBlockName(program: GLuint, uniformBlockIndex: GLuint, bufSize: GLsizei, length: *mut GLsizei, uniformBlockName: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, uniformBlockIndex: GLuint, bufSize: GLsizei, length: *mut GLsizei, uniformBlockName: *mut GLchar) >(storage::GetActiveUniformBlockName.f)(program, uniformBlockIndex, bufSize, length, uniformBlockName) }
#[inline] pub unsafe fn GetActiveUniformBlockiv(program: GLuint, uniformBlockIndex: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, uniformBlockIndex: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetActiveUniformBlockiv.f)(program, uniformBlockIndex, pname, params) }
#[inline] pub unsafe fn GetActiveUniformName(program: GLuint, uniformIndex: GLuint, bufSize: GLsizei, length: *mut GLsizei, uniformName: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, uniformIndex: GLuint, bufSize: GLsizei, length: *mut GLsizei, uniformName: *mut GLchar) >(storage::GetActiveUniformName.f)(program, uniformIndex, bufSize, length, uniformName) }
#[inline] pub unsafe fn GetActiveUniformsiv(program: GLuint, uniformCount: GLsizei, uniformIndices: *const GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, uniformCount: GLsizei, uniformIndices: *const GLuint, pname: GLenum, params: *mut GLint) >(storage::GetActiveUniformsiv.f)(program, uniformCount, uniformIndices, pname, params) }
#[inline] pub unsafe fn GetAttachedShaders(program: GLuint, maxCount: GLsizei, count: *mut GLsizei, shaders: *mut GLuint) { mem::transmute::<_, extern "system" fn(program: GLuint, maxCount: GLsizei, count: *mut GLsizei, shaders: *mut GLuint) >(storage::GetAttachedShaders.f)(program, maxCount, count, shaders) }
#[inline] pub unsafe fn GetAttribLocation(program: GLuint, name: *const GLchar) -> GLint { mem::transmute::<_, extern "system" fn(program: GLuint, name: *const GLchar)  -> GLint>(storage::GetAttribLocation.f)(program, name) }
#[inline] pub unsafe fn GetBooleani_v(target: GLenum, index: GLuint, data: *mut GLboolean) { mem::transmute::<_, extern "system" fn(target: GLenum, index: GLuint, data: *mut GLboolean) >(storage::GetBooleani_v.f)(target, index, data) }
#[inline] pub unsafe fn GetBooleanv(pname: GLenum, data: *mut GLboolean) { mem::transmute::<_, extern "system" fn(pname: GLenum, data: *mut GLboolean) >(storage::GetBooleanv.f)(pname, data) }
#[inline] pub unsafe fn GetBufferParameteri64v(target: GLenum, pname: GLenum, params: *mut GLint64) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLint64) >(storage::GetBufferParameteri64v.f)(target, pname, params) }
#[inline] pub unsafe fn GetBufferParameteriv(target: GLenum, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLint) >(storage::GetBufferParameteriv.f)(target, pname, params) }
#[inline] pub unsafe fn GetBufferPointerv(target: GLenum, pname: GLenum, params: *const *mut c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *const *mut c_void) >(storage::GetBufferPointerv.f)(target, pname, params) }
#[inline] pub unsafe fn GetBufferSubData(target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *mut c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *mut c_void) >(storage::GetBufferSubData.f)(target, offset, size, data) }
#[inline] pub unsafe fn GetCompressedTexImage(target: GLenum, level: GLint, img: *mut c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, img: *mut c_void) >(storage::GetCompressedTexImage.f)(target, level, img) }
#[inline] pub unsafe fn GetDebugMessageLog(count: GLuint, bufSize: GLsizei, sources: *mut GLenum, types: *mut GLenum, ids: *mut GLuint, severities: *mut GLenum, lengths: *mut GLsizei, messageLog: *mut GLchar) -> GLuint { mem::transmute::<_, extern "system" fn(count: GLuint, bufSize: GLsizei, sources: *mut GLenum, types: *mut GLenum, ids: *mut GLuint, severities: *mut GLenum, lengths: *mut GLsizei, messageLog: *mut GLchar)  -> GLuint>(storage::GetDebugMessageLog.f)(count, bufSize, sources, types, ids, severities, lengths, messageLog) }
#[inline] pub unsafe fn GetDoublei_v(target: GLenum, index: GLuint, data: *mut GLdouble) { mem::transmute::<_, extern "system" fn(target: GLenum, index: GLuint, data: *mut GLdouble) >(storage::GetDoublei_v.f)(target, index, data) }
#[inline] pub unsafe fn GetDoublev(pname: GLenum, data: *mut GLdouble) { mem::transmute::<_, extern "system" fn(pname: GLenum, data: *mut GLdouble) >(storage::GetDoublev.f)(pname, data) }
#[inline] pub fn GetError() -> GLenum { unsafe { mem::transmute::<_, extern "system" fn() -> GLenum>(storage::GetError.f)() } }
#[inline] pub unsafe fn GetFloati_v(target: GLenum, index: GLuint, data: *mut GLfloat) { mem::transmute::<_, extern "system" fn(target: GLenum, index: GLuint, data: *mut GLfloat) >(storage::GetFloati_v.f)(target, index, data) }
#[inline] pub unsafe fn GetFloatv(pname: GLenum, data: *mut GLfloat) { mem::transmute::<_, extern "system" fn(pname: GLenum, data: *mut GLfloat) >(storage::GetFloatv.f)(pname, data) }
#[inline] pub unsafe fn GetFragDataIndex(program: GLuint, name: *const GLchar) -> GLint { mem::transmute::<_, extern "system" fn(program: GLuint, name: *const GLchar)  -> GLint>(storage::GetFragDataIndex.f)(program, name) }
#[inline] pub unsafe fn GetFragDataLocation(program: GLuint, name: *const GLchar) -> GLint { mem::transmute::<_, extern "system" fn(program: GLuint, name: *const GLchar)  -> GLint>(storage::GetFragDataLocation.f)(program, name) }
#[inline] pub unsafe fn GetFramebufferAttachmentParameteriv(target: GLenum, attachment: GLenum, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, attachment: GLenum, pname: GLenum, params: *mut GLint) >(storage::GetFramebufferAttachmentParameteriv.f)(target, attachment, pname, params) }
#[inline] pub unsafe fn GetFramebufferParameteriv(target: GLenum, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLint) >(storage::GetFramebufferParameteriv.f)(target, pname, params) }
#[inline] pub unsafe fn GetInteger64i_v(target: GLenum, index: GLuint, data: *mut GLint64) { mem::transmute::<_, extern "system" fn(target: GLenum, index: GLuint, data: *mut GLint64) >(storage::GetInteger64i_v.f)(target, index, data) }
#[inline] pub unsafe fn GetInteger64v(pname: GLenum, data: *mut GLint64) { mem::transmute::<_, extern "system" fn(pname: GLenum, data: *mut GLint64) >(storage::GetInteger64v.f)(pname, data) }
#[inline] pub unsafe fn GetIntegeri_v(target: GLenum, index: GLuint, data: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, index: GLuint, data: *mut GLint) >(storage::GetIntegeri_v.f)(target, index, data) }
#[inline] pub unsafe fn GetIntegerv(pname: GLenum, data: *mut GLint) { mem::transmute::<_, extern "system" fn(pname: GLenum, data: *mut GLint) >(storage::GetIntegerv.f)(pname, data) }
#[inline] pub unsafe fn GetInternalformati64v(target: GLenum, internalformat: GLenum, pname: GLenum, bufSize: GLsizei, params: *mut GLint64) { mem::transmute::<_, extern "system" fn(target: GLenum, internalformat: GLenum, pname: GLenum, bufSize: GLsizei, params: *mut GLint64) >(storage::GetInternalformati64v.f)(target, internalformat, pname, bufSize, params) }
#[inline] pub unsafe fn GetInternalformativ(target: GLenum, internalformat: GLenum, pname: GLenum, bufSize: GLsizei, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, internalformat: GLenum, pname: GLenum, bufSize: GLsizei, params: *mut GLint) >(storage::GetInternalformativ.f)(target, internalformat, pname, bufSize, params) }
#[inline] pub unsafe fn GetMultisamplefv(pname: GLenum, index: GLuint, val: *mut GLfloat) { mem::transmute::<_, extern "system" fn(pname: GLenum, index: GLuint, val: *mut GLfloat) >(storage::GetMultisamplefv.f)(pname, index, val) }
#[inline] pub unsafe fn GetObjectLabel(identifier: GLenum, name: GLuint, bufSize: GLsizei, length: *mut GLsizei, label: *mut GLchar) { mem::transmute::<_, extern "system" fn(identifier: GLenum, name: GLuint, bufSize: GLsizei, length: *mut GLsizei, label: *mut GLchar) >(storage::GetObjectLabel.f)(identifier, name, bufSize, length, label) }
#[inline] pub unsafe fn GetObjectPtrLabel(ptr: *const c_void, bufSize: GLsizei, length: *mut GLsizei, label: *mut GLchar) { mem::transmute::<_, extern "system" fn(ptr: *const c_void, bufSize: GLsizei, length: *mut GLsizei, label: *mut GLchar) >(storage::GetObjectPtrLabel.f)(ptr, bufSize, length, label) }
#[inline] pub unsafe fn GetProgramBinary(program: GLuint, bufSize: GLsizei, length: *mut GLsizei, binaryFormat: *mut GLenum, binary: *mut c_void) { mem::transmute::<_, extern "system" fn(program: GLuint, bufSize: GLsizei, length: *mut GLsizei, binaryFormat: *mut GLenum, binary: *mut c_void) >(storage::GetProgramBinary.f)(program, bufSize, length, binaryFormat, binary) }
#[inline] pub unsafe fn GetProgramInfoLog(program: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) >(storage::GetProgramInfoLog.f)(program, bufSize, length, infoLog) }
#[inline] pub unsafe fn GetProgramInterfaceiv(program: GLuint, programInterface: GLenum, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, programInterface: GLenum, pname: GLenum, params: *mut GLint) >(storage::GetProgramInterfaceiv.f)(program, programInterface, pname, params) }
#[inline] pub unsafe fn GetProgramPipelineInfoLog(pipeline: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) { mem::transmute::<_, extern "system" fn(pipeline: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) >(storage::GetProgramPipelineInfoLog.f)(pipeline, bufSize, length, infoLog) }
#[inline] pub unsafe fn GetProgramPipelineiv(pipeline: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(pipeline: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetProgramPipelineiv.f)(pipeline, pname, params) }
#[inline] pub unsafe fn GetProgramResourceIndex(program: GLuint, programInterface: GLenum, name: *const GLchar) -> GLuint { mem::transmute::<_, extern "system" fn(program: GLuint, programInterface: GLenum, name: *const GLchar)  -> GLuint>(storage::GetProgramResourceIndex.f)(program, programInterface, name) }
#[inline] pub unsafe fn GetProgramResourceLocation(program: GLuint, programInterface: GLenum, name: *const GLchar) -> GLint { mem::transmute::<_, extern "system" fn(program: GLuint, programInterface: GLenum, name: *const GLchar)  -> GLint>(storage::GetProgramResourceLocation.f)(program, programInterface, name) }
#[inline] pub unsafe fn GetProgramResourceLocationIndex(program: GLuint, programInterface: GLenum, name: *const GLchar) -> GLint { mem::transmute::<_, extern "system" fn(program: GLuint, programInterface: GLenum, name: *const GLchar)  -> GLint>(storage::GetProgramResourceLocationIndex.f)(program, programInterface, name) }
#[inline] pub unsafe fn GetProgramResourceName(program: GLuint, programInterface: GLenum, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, name: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, programInterface: GLenum, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, name: *mut GLchar) >(storage::GetProgramResourceName.f)(program, programInterface, index, bufSize, length, name) }
#[inline] pub unsafe fn GetProgramResourceiv(program: GLuint, programInterface: GLenum, index: GLuint, propCount: GLsizei, props: *const GLenum, bufSize: GLsizei, length: *mut GLsizei, params: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, programInterface: GLenum, index: GLuint, propCount: GLsizei, props: *const GLenum, bufSize: GLsizei, length: *mut GLsizei, params: *mut GLint) >(storage::GetProgramResourceiv.f)(program, programInterface, index, propCount, props, bufSize, length, params) }
#[inline] pub unsafe fn GetProgramStageiv(program: GLuint, shadertype: GLenum, pname: GLenum, values: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, shadertype: GLenum, pname: GLenum, values: *mut GLint) >(storage::GetProgramStageiv.f)(program, shadertype, pname, values) }
#[inline] pub unsafe fn GetProgramiv(program: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetProgramiv.f)(program, pname, params) }
#[inline] pub unsafe fn GetQueryIndexediv(target: GLenum, index: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, index: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetQueryIndexediv.f)(target, index, pname, params) }
#[inline] pub unsafe fn GetQueryObjecti64v(id: GLuint, pname: GLenum, params: *mut GLint64) { mem::transmute::<_, extern "system" fn(id: GLuint, pname: GLenum, params: *mut GLint64) >(storage::GetQueryObjecti64v.f)(id, pname, params) }
#[inline] pub unsafe fn GetQueryObjectiv(id: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(id: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetQueryObjectiv.f)(id, pname, params) }
#[inline] pub unsafe fn GetQueryObjectui64v(id: GLuint, pname: GLenum, params: *mut GLuint64) { mem::transmute::<_, extern "system" fn(id: GLuint, pname: GLenum, params: *mut GLuint64) >(storage::GetQueryObjectui64v.f)(id, pname, params) }
#[inline] pub unsafe fn GetQueryObjectuiv(id: GLuint, pname: GLenum, params: *mut GLuint) { mem::transmute::<_, extern "system" fn(id: GLuint, pname: GLenum, params: *mut GLuint) >(storage::GetQueryObjectuiv.f)(id, pname, params) }
#[inline] pub unsafe fn GetQueryiv(target: GLenum, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLint) >(storage::GetQueryiv.f)(target, pname, params) }
#[inline] pub unsafe fn GetRenderbufferParameteriv(target: GLenum, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLint) >(storage::GetRenderbufferParameteriv.f)(target, pname, params) }
#[inline] pub unsafe fn GetSamplerParameterIiv(sampler: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(sampler: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetSamplerParameterIiv.f)(sampler, pname, params) }
#[inline] pub unsafe fn GetSamplerParameterIuiv(sampler: GLuint, pname: GLenum, params: *mut GLuint) { mem::transmute::<_, extern "system" fn(sampler: GLuint, pname: GLenum, params: *mut GLuint) >(storage::GetSamplerParameterIuiv.f)(sampler, pname, params) }
#[inline] pub unsafe fn GetSamplerParameterfv(sampler: GLuint, pname: GLenum, params: *mut GLfloat) { mem::transmute::<_, extern "system" fn(sampler: GLuint, pname: GLenum, params: *mut GLfloat) >(storage::GetSamplerParameterfv.f)(sampler, pname, params) }
#[inline] pub unsafe fn GetSamplerParameteriv(sampler: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(sampler: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetSamplerParameteriv.f)(sampler, pname, params) }
#[inline] pub unsafe fn GetShaderInfoLog(shader: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) { mem::transmute::<_, extern "system" fn(shader: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) >(storage::GetShaderInfoLog.f)(shader, bufSize, length, infoLog) }
#[inline] pub unsafe fn GetShaderPrecisionFormat(shadertype: GLenum, precisiontype: GLenum, range: *mut GLint, precision: *mut GLint) { mem::transmute::<_, extern "system" fn(shadertype: GLenum, precisiontype: GLenum, range: *mut GLint, precision: *mut GLint) >(storage::GetShaderPrecisionFormat.f)(shadertype, precisiontype, range, precision) }
#[inline] pub unsafe fn GetShaderSource(shader: GLuint, bufSize: GLsizei, length: *mut GLsizei, source: *mut GLchar) { mem::transmute::<_, extern "system" fn(shader: GLuint, bufSize: GLsizei, length: *mut GLsizei, source: *mut GLchar) >(storage::GetShaderSource.f)(shader, bufSize, length, source) }
#[inline] pub unsafe fn GetShaderiv(shader: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(shader: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetShaderiv.f)(shader, pname, params) }
#[inline] pub fn GetString(name: GLenum) -> *const GLubyte { unsafe { mem::transmute::<_, extern "system" fn(GLenum) -> *const GLubyte>(storage::GetString.f)(name) } }
#[inline] pub fn GetStringi(name: GLenum, index: GLuint) -> *const GLubyte { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint) -> *const GLubyte>(storage::GetStringi.f)(name, index) } }
#[inline] pub unsafe fn GetSubroutineIndex(program: GLuint, shadertype: GLenum, name: *const GLchar) -> GLuint { mem::transmute::<_, extern "system" fn(program: GLuint, shadertype: GLenum, name: *const GLchar)  -> GLuint>(storage::GetSubroutineIndex.f)(program, shadertype, name) }
#[inline] pub unsafe fn GetSubroutineUniformLocation(program: GLuint, shadertype: GLenum, name: *const GLchar) -> GLint { mem::transmute::<_, extern "system" fn(program: GLuint, shadertype: GLenum, name: *const GLchar)  -> GLint>(storage::GetSubroutineUniformLocation.f)(program, shadertype, name) }
#[inline] pub unsafe fn GetSynciv(sync: GLsync, pname: GLenum, bufSize: GLsizei, length: *mut GLsizei, values: *mut GLint) { mem::transmute::<_, extern "system" fn(sync: GLsync, pname: GLenum, bufSize: GLsizei, length: *mut GLsizei, values: *mut GLint) >(storage::GetSynciv.f)(sync, pname, bufSize, length, values) }
#[inline] pub unsafe fn GetTexImage(target: GLenum, level: GLint, format: GLenum, type_: GLenum, pixels: *mut c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, format: GLenum, type_: GLenum, pixels: *mut c_void) >(storage::GetTexImage.f)(target, level, format, type_, pixels) }
#[inline] pub unsafe fn GetTexLevelParameterfv(target: GLenum, level: GLint, pname: GLenum, params: *mut GLfloat) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, pname: GLenum, params: *mut GLfloat) >(storage::GetTexLevelParameterfv.f)(target, level, pname, params) }
#[inline] pub unsafe fn GetTexLevelParameteriv(target: GLenum, level: GLint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, pname: GLenum, params: *mut GLint) >(storage::GetTexLevelParameteriv.f)(target, level, pname, params) }
#[inline] pub unsafe fn GetTexParameterIiv(target: GLenum, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLint) >(storage::GetTexParameterIiv.f)(target, pname, params) }
#[inline] pub unsafe fn GetTexParameterIuiv(target: GLenum, pname: GLenum, params: *mut GLuint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLuint) >(storage::GetTexParameterIuiv.f)(target, pname, params) }
#[inline] pub unsafe fn GetTexParameterfv(target: GLenum, pname: GLenum, params: *mut GLfloat) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLfloat) >(storage::GetTexParameterfv.f)(target, pname, params) }
#[inline] pub unsafe fn GetTexParameteriv(target: GLenum, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *mut GLint) >(storage::GetTexParameteriv.f)(target, pname, params) }
#[inline] pub unsafe fn GetTransformFeedbackVarying(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLsizei, type_: *mut GLenum, name: *mut GLchar) { mem::transmute::<_, extern "system" fn(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLsizei, type_: *mut GLenum, name: *mut GLchar) >(storage::GetTransformFeedbackVarying.f)(program, index, bufSize, length, size, type_, name) }
#[inline] pub unsafe fn GetUniformBlockIndex(program: GLuint, uniformBlockName: *const GLchar) -> GLuint { mem::transmute::<_, extern "system" fn(program: GLuint, uniformBlockName: *const GLchar)  -> GLuint>(storage::GetUniformBlockIndex.f)(program, uniformBlockName) }
#[inline] pub unsafe fn GetUniformIndices(program: GLuint, uniformCount: GLsizei, uniformNames: *const *const GLchar, uniformIndices: *mut GLuint) { mem::transmute::<_, extern "system" fn(program: GLuint, uniformCount: GLsizei, uniformNames: *const *const GLchar, uniformIndices: *mut GLuint) >(storage::GetUniformIndices.f)(program, uniformCount, uniformNames, uniformIndices) }
#[inline] pub unsafe fn GetUniformLocation(program: GLuint, name: *const GLchar) -> GLint { mem::transmute::<_, extern "system" fn(program: GLuint, name: *const GLchar)  -> GLint>(storage::GetUniformLocation.f)(program, name) }
#[inline] pub unsafe fn GetUniformSubroutineuiv(shadertype: GLenum, location: GLint, params: *mut GLuint) { mem::transmute::<_, extern "system" fn(shadertype: GLenum, location: GLint, params: *mut GLuint) >(storage::GetUniformSubroutineuiv.f)(shadertype, location, params) }
#[inline] pub unsafe fn GetUniformdv(program: GLuint, location: GLint, params: *mut GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, params: *mut GLdouble) >(storage::GetUniformdv.f)(program, location, params) }
#[inline] pub unsafe fn GetUniformfv(program: GLuint, location: GLint, params: *mut GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, params: *mut GLfloat) >(storage::GetUniformfv.f)(program, location, params) }
#[inline] pub unsafe fn GetUniformiv(program: GLuint, location: GLint, params: *mut GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, params: *mut GLint) >(storage::GetUniformiv.f)(program, location, params) }
#[inline] pub unsafe fn GetUniformuiv(program: GLuint, location: GLint, params: *mut GLuint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, params: *mut GLuint) >(storage::GetUniformuiv.f)(program, location, params) }
#[inline] pub unsafe fn GetVertexAttribIiv(index: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetVertexAttribIiv.f)(index, pname, params) }
#[inline] pub unsafe fn GetVertexAttribIuiv(index: GLuint, pname: GLenum, params: *mut GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, pname: GLenum, params: *mut GLuint) >(storage::GetVertexAttribIuiv.f)(index, pname, params) }
#[inline] pub unsafe fn GetVertexAttribLdv(index: GLuint, pname: GLenum, params: *mut GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, pname: GLenum, params: *mut GLdouble) >(storage::GetVertexAttribLdv.f)(index, pname, params) }
#[inline] pub unsafe fn GetVertexAttribPointerv(index: GLuint, pname: GLenum, pointer: *const *mut c_void) { mem::transmute::<_, extern "system" fn(index: GLuint, pname: GLenum, pointer: *const *mut c_void) >(storage::GetVertexAttribPointerv.f)(index, pname, pointer) }
#[inline] pub unsafe fn GetVertexAttribdv(index: GLuint, pname: GLenum, params: *mut GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, pname: GLenum, params: *mut GLdouble) >(storage::GetVertexAttribdv.f)(index, pname, params) }
#[inline] pub unsafe fn GetVertexAttribfv(index: GLuint, pname: GLenum, params: *mut GLfloat) { mem::transmute::<_, extern "system" fn(index: GLuint, pname: GLenum, params: *mut GLfloat) >(storage::GetVertexAttribfv.f)(index, pname, params) }
#[inline] pub unsafe fn GetVertexAttribiv(index: GLuint, pname: GLenum, params: *mut GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, pname: GLenum, params: *mut GLint) >(storage::GetVertexAttribiv.f)(index, pname, params) }
#[inline] pub fn Hint(target: GLenum, mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum)>(storage::Hint.f)(target, mode) } }
#[inline] pub fn InvalidateBufferData(buffer: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::InvalidateBufferData.f)(buffer) } }
#[inline] pub fn InvalidateBufferSubData(buffer: GLuint, offset: GLintptr, length: GLsizeiptr) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLintptr, GLsizeiptr)>(storage::InvalidateBufferSubData.f)(buffer, offset, length) } }
#[inline] pub unsafe fn InvalidateFramebuffer(target: GLenum, numAttachments: GLsizei, attachments: *const GLenum) { mem::transmute::<_, extern "system" fn(target: GLenum, numAttachments: GLsizei, attachments: *const GLenum) >(storage::InvalidateFramebuffer.f)(target, numAttachments, attachments) }
#[inline] pub unsafe fn InvalidateSubFramebuffer(target: GLenum, numAttachments: GLsizei, attachments: *const GLenum, x: GLint, y: GLint, width: GLsizei, height: GLsizei) { mem::transmute::<_, extern "system" fn(target: GLenum, numAttachments: GLsizei, attachments: *const GLenum, x: GLint, y: GLint, width: GLsizei, height: GLsizei) >(storage::InvalidateSubFramebuffer.f)(target, numAttachments, attachments, x, y, width, height) }
#[inline] pub fn InvalidateTexImage(texture: GLuint, level: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint)>(storage::InvalidateTexImage.f)(texture, level) } }
#[inline] pub fn InvalidateTexSubImage(texture: GLuint, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, width: GLsizei, height: GLsizei, depth: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint, GLint, GLint, GLsizei, GLsizei, GLsizei)>(storage::InvalidateTexSubImage.f)(texture, level, xoffset, yoffset, zoffset, width, height, depth) } }
#[inline] pub fn IsBuffer(buffer: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsBuffer.f)(buffer) } }
#[inline] pub fn IsEnabled(cap: GLenum) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLenum) -> GLboolean>(storage::IsEnabled.f)(cap) } }
#[inline] pub fn IsEnabledi(target: GLenum, index: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint) -> GLboolean>(storage::IsEnabledi.f)(target, index) } }
#[inline] pub fn IsFramebuffer(framebuffer: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsFramebuffer.f)(framebuffer) } }
#[inline] pub fn IsProgram(program: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsProgram.f)(program) } }
#[inline] pub fn IsProgramPipeline(pipeline: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsProgramPipeline.f)(pipeline) } }
#[inline] pub fn IsQuery(id: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsQuery.f)(id) } }
#[inline] pub fn IsRenderbuffer(renderbuffer: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsRenderbuffer.f)(renderbuffer) } }
#[inline] pub fn IsSampler(sampler: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsSampler.f)(sampler) } }
#[inline] pub fn IsShader(shader: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsShader.f)(shader) } }
#[inline] pub fn IsSync(sync: GLsync) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLsync) -> GLboolean>(storage::IsSync.f)(sync) } }
#[inline] pub fn IsTexture(texture: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsTexture.f)(texture) } }
#[inline] pub fn IsTransformFeedback(id: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsTransformFeedback.f)(id) } }
#[inline] pub fn IsVertexArray(array: GLuint) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLuint) -> GLboolean>(storage::IsVertexArray.f)(array) } }
#[inline] pub fn LineWidth(width: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat)>(storage::LineWidth.f)(width) } }
#[inline] pub fn LinkProgram(program: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::LinkProgram.f)(program) } }
#[inline] pub fn LogicOp(opcode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::LogicOp.f)(opcode) } }
#[inline] pub fn MapBuffer(target: GLenum, access: GLenum) -> *const c_void { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum) -> *const c_void>(storage::MapBuffer.f)(target, access) } }
#[inline] pub fn MapBufferRange(target: GLenum, offset: GLintptr, length: GLsizeiptr, access: GLbitfield) -> *const c_void { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLintptr, GLsizeiptr, GLbitfield) -> *const c_void>(storage::MapBufferRange.f)(target, offset, length, access) } }
#[inline] pub fn MemoryBarrier(barriers: GLbitfield) { unsafe { mem::transmute::<_, extern "system" fn(GLbitfield)>(storage::MemoryBarrier.f)(barriers) } }
#[inline] pub fn MinSampleShading(value: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat)>(storage::MinSampleShading.f)(value) } }
#[inline] pub unsafe fn MultiDrawArrays(mode: GLenum, first: *const GLint, count: *const GLsizei, drawcount: GLsizei) { mem::transmute::<_, extern "system" fn(mode: GLenum, first: *const GLint, count: *const GLsizei, drawcount: GLsizei) >(storage::MultiDrawArrays.f)(mode, first, count, drawcount) }
#[inline] pub unsafe fn MultiDrawArraysIndirect(mode: GLenum, indirect: *const c_void, drawcount: GLsizei, stride: GLsizei) { mem::transmute::<_, extern "system" fn(mode: GLenum, indirect: *const c_void, drawcount: GLsizei, stride: GLsizei) >(storage::MultiDrawArraysIndirect.f)(mode, indirect, drawcount, stride) }
#[inline] pub unsafe fn MultiDrawElements(mode: GLenum, count: *const GLsizei, type_: GLenum, indices: *const *const c_void, drawcount: GLsizei) { mem::transmute::<_, extern "system" fn(mode: GLenum, count: *const GLsizei, type_: GLenum, indices: *const *const c_void, drawcount: GLsizei) >(storage::MultiDrawElements.f)(mode, count, type_, indices, drawcount) }
#[inline] pub unsafe fn MultiDrawElementsBaseVertex(mode: GLenum, count: *const GLsizei, type_: GLenum, indices: *const *const c_void, drawcount: GLsizei, basevertex: *const GLint) { mem::transmute::<_, extern "system" fn(mode: GLenum, count: *const GLsizei, type_: GLenum, indices: *const *const c_void, drawcount: GLsizei, basevertex: *const GLint) >(storage::MultiDrawElementsBaseVertex.f)(mode, count, type_, indices, drawcount, basevertex) }
#[inline] pub unsafe fn MultiDrawElementsIndirect(mode: GLenum, type_: GLenum, indirect: *const c_void, drawcount: GLsizei, stride: GLsizei) { mem::transmute::<_, extern "system" fn(mode: GLenum, type_: GLenum, indirect: *const c_void, drawcount: GLsizei, stride: GLsizei) >(storage::MultiDrawElementsIndirect.f)(mode, type_, indirect, drawcount, stride) }
#[inline] pub fn MultiTexCoordP1ui(texture: GLenum, type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLuint)>(storage::MultiTexCoordP1ui.f)(texture, type_, coords) } }
#[inline] pub unsafe fn MultiTexCoordP1uiv(texture: GLenum, type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(texture: GLenum, type_: GLenum, coords: *const GLuint) >(storage::MultiTexCoordP1uiv.f)(texture, type_, coords) }
#[inline] pub fn MultiTexCoordP2ui(texture: GLenum, type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLuint)>(storage::MultiTexCoordP2ui.f)(texture, type_, coords) } }
#[inline] pub unsafe fn MultiTexCoordP2uiv(texture: GLenum, type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(texture: GLenum, type_: GLenum, coords: *const GLuint) >(storage::MultiTexCoordP2uiv.f)(texture, type_, coords) }
#[inline] pub fn MultiTexCoordP3ui(texture: GLenum, type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLuint)>(storage::MultiTexCoordP3ui.f)(texture, type_, coords) } }
#[inline] pub unsafe fn MultiTexCoordP3uiv(texture: GLenum, type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(texture: GLenum, type_: GLenum, coords: *const GLuint) >(storage::MultiTexCoordP3uiv.f)(texture, type_, coords) }
#[inline] pub fn MultiTexCoordP4ui(texture: GLenum, type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLuint)>(storage::MultiTexCoordP4ui.f)(texture, type_, coords) } }
#[inline] pub unsafe fn MultiTexCoordP4uiv(texture: GLenum, type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(texture: GLenum, type_: GLenum, coords: *const GLuint) >(storage::MultiTexCoordP4uiv.f)(texture, type_, coords) }
#[inline] pub fn NormalP3ui(type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::NormalP3ui.f)(type_, coords) } }
#[inline] pub unsafe fn NormalP3uiv(type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, coords: *const GLuint) >(storage::NormalP3uiv.f)(type_, coords) }
#[inline] pub unsafe fn ObjectLabel(identifier: GLenum, name: GLuint, length: GLsizei, label: *const GLchar) { mem::transmute::<_, extern "system" fn(identifier: GLenum, name: GLuint, length: GLsizei, label: *const GLchar) >(storage::ObjectLabel.f)(identifier, name, length, label) }
#[inline] pub unsafe fn ObjectPtrLabel(ptr: *const c_void, length: GLsizei, label: *const GLchar) { mem::transmute::<_, extern "system" fn(ptr: *const c_void, length: GLsizei, label: *const GLchar) >(storage::ObjectPtrLabel.f)(ptr, length, label) }
#[inline] pub unsafe fn PatchParameterfv(pname: GLenum, values: *const GLfloat) { mem::transmute::<_, extern "system" fn(pname: GLenum, values: *const GLfloat) >(storage::PatchParameterfv.f)(pname, values) }
#[inline] pub fn PatchParameteri(pname: GLenum, value: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint)>(storage::PatchParameteri.f)(pname, value) } }
#[inline] pub fn PauseTransformFeedback() { unsafe { mem::transmute::<_, extern "system" fn()>(storage::PauseTransformFeedback.f)() } }
#[inline] pub fn PixelStoref(pname: GLenum, param: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLfloat)>(storage::PixelStoref.f)(pname, param) } }
#[inline] pub fn PixelStorei(pname: GLenum, param: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint)>(storage::PixelStorei.f)(pname, param) } }
#[inline] pub fn PointParameterf(pname: GLenum, param: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLfloat)>(storage::PointParameterf.f)(pname, param) } }
#[inline] pub unsafe fn PointParameterfv(pname: GLenum, params: *const GLfloat) { mem::transmute::<_, extern "system" fn(pname: GLenum, params: *const GLfloat) >(storage::PointParameterfv.f)(pname, params) }
#[inline] pub fn PointParameteri(pname: GLenum, param: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint)>(storage::PointParameteri.f)(pname, param) } }
#[inline] pub unsafe fn PointParameteriv(pname: GLenum, params: *const GLint) { mem::transmute::<_, extern "system" fn(pname: GLenum, params: *const GLint) >(storage::PointParameteriv.f)(pname, params) }
#[inline] pub fn PointSize(size: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat)>(storage::PointSize.f)(size) } }
#[inline] pub fn PolygonMode(face: GLenum, mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum)>(storage::PolygonMode.f)(face, mode) } }
#[inline] pub fn PolygonOffset(factor: GLfloat, units: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat, GLfloat)>(storage::PolygonOffset.f)(factor, units) } }
#[inline] pub fn PopDebugGroup() { unsafe { mem::transmute::<_, extern "system" fn()>(storage::PopDebugGroup.f)() } }
#[inline] pub fn PrimitiveRestartIndex(index: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::PrimitiveRestartIndex.f)(index) } }
#[inline] pub unsafe fn ProgramBinary(program: GLuint, binaryFormat: GLenum, binary: *const c_void, length: GLsizei) { mem::transmute::<_, extern "system" fn(program: GLuint, binaryFormat: GLenum, binary: *const c_void, length: GLsizei) >(storage::ProgramBinary.f)(program, binaryFormat, binary, length) }
#[inline] pub fn ProgramParameteri(program: GLuint, pname: GLenum, value: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLint)>(storage::ProgramParameteri.f)(program, pname, value) } }
#[inline] pub fn ProgramUniform1d(program: GLuint, location: GLint, v0: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLdouble)>(storage::ProgramUniform1d.f)(program, location, v0) } }
#[inline] pub unsafe fn ProgramUniform1dv(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) >(storage::ProgramUniform1dv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform1f(program: GLuint, location: GLint, v0: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLfloat)>(storage::ProgramUniform1f.f)(program, location, v0) } }
#[inline] pub unsafe fn ProgramUniform1fv(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) >(storage::ProgramUniform1fv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform1i(program: GLuint, location: GLint, v0: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint)>(storage::ProgramUniform1i.f)(program, location, v0) } }
#[inline] pub unsafe fn ProgramUniform1iv(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) >(storage::ProgramUniform1iv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform1ui(program: GLuint, location: GLint, v0: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLuint)>(storage::ProgramUniform1ui.f)(program, location, v0) } }
#[inline] pub unsafe fn ProgramUniform1uiv(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) >(storage::ProgramUniform1uiv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform2d(program: GLuint, location: GLint, v0: GLdouble, v1: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLdouble, GLdouble)>(storage::ProgramUniform2d.f)(program, location, v0, v1) } }
#[inline] pub unsafe fn ProgramUniform2dv(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) >(storage::ProgramUniform2dv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform2f(program: GLuint, location: GLint, v0: GLfloat, v1: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLfloat, GLfloat)>(storage::ProgramUniform2f.f)(program, location, v0, v1) } }
#[inline] pub unsafe fn ProgramUniform2fv(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) >(storage::ProgramUniform2fv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform2i(program: GLuint, location: GLint, v0: GLint, v1: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint, GLint)>(storage::ProgramUniform2i.f)(program, location, v0, v1) } }
#[inline] pub unsafe fn ProgramUniform2iv(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) >(storage::ProgramUniform2iv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform2ui(program: GLuint, location: GLint, v0: GLuint, v1: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLuint, GLuint)>(storage::ProgramUniform2ui.f)(program, location, v0, v1) } }
#[inline] pub unsafe fn ProgramUniform2uiv(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) >(storage::ProgramUniform2uiv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform3d(program: GLuint, location: GLint, v0: GLdouble, v1: GLdouble, v2: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLdouble, GLdouble, GLdouble)>(storage::ProgramUniform3d.f)(program, location, v0, v1, v2) } }
#[inline] pub unsafe fn ProgramUniform3dv(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) >(storage::ProgramUniform3dv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform3f(program: GLuint, location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLfloat, GLfloat, GLfloat)>(storage::ProgramUniform3f.f)(program, location, v0, v1, v2) } }
#[inline] pub unsafe fn ProgramUniform3fv(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) >(storage::ProgramUniform3fv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform3i(program: GLuint, location: GLint, v0: GLint, v1: GLint, v2: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint, GLint, GLint)>(storage::ProgramUniform3i.f)(program, location, v0, v1, v2) } }
#[inline] pub unsafe fn ProgramUniform3iv(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) >(storage::ProgramUniform3iv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform3ui(program: GLuint, location: GLint, v0: GLuint, v1: GLuint, v2: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLuint, GLuint, GLuint)>(storage::ProgramUniform3ui.f)(program, location, v0, v1, v2) } }
#[inline] pub unsafe fn ProgramUniform3uiv(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) >(storage::ProgramUniform3uiv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform4d(program: GLuint, location: GLint, v0: GLdouble, v1: GLdouble, v2: GLdouble, v3: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLdouble, GLdouble, GLdouble, GLdouble)>(storage::ProgramUniform4d.f)(program, location, v0, v1, v2, v3) } }
#[inline] pub unsafe fn ProgramUniform4dv(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) >(storage::ProgramUniform4dv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform4f(program: GLuint, location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat, v3: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLfloat, GLfloat, GLfloat, GLfloat)>(storage::ProgramUniform4f.f)(program, location, v0, v1, v2, v3) } }
#[inline] pub unsafe fn ProgramUniform4fv(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) >(storage::ProgramUniform4fv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform4i(program: GLuint, location: GLint, v0: GLint, v1: GLint, v2: GLint, v3: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint, GLint, GLint, GLint)>(storage::ProgramUniform4i.f)(program, location, v0, v1, v2, v3) } }
#[inline] pub unsafe fn ProgramUniform4iv(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) >(storage::ProgramUniform4iv.f)(program, location, count, value) }
#[inline] pub fn ProgramUniform4ui(program: GLuint, location: GLint, v0: GLuint, v1: GLuint, v2: GLuint, v3: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLuint, GLuint, GLuint, GLuint)>(storage::ProgramUniform4ui.f)(program, location, v0, v1, v2, v3) } }
#[inline] pub unsafe fn ProgramUniform4uiv(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) >(storage::ProgramUniform4uiv.f)(program, location, count, value) }
#[inline] pub unsafe fn ProgramUniformMatrix2dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix2dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix2fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix2fv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix2x3dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix2x3dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix2x3fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix2x3fv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix2x4dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix2x4dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix2x4fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix2x4fv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix3dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix3dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix3fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix3fv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix3x2dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix3x2dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix3x2fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix3x2fv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix3x4dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix3x4dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix3x4fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix3x4fv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix4dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix4dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix4fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix4fv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix4x2dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix4x2dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix4x2fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix4x2fv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix4x3dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::ProgramUniformMatrix4x3dv.f)(program, location, count, transpose, value) }
#[inline] pub unsafe fn ProgramUniformMatrix4x3fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::ProgramUniformMatrix4x3fv.f)(program, location, count, transpose, value) }
#[inline] pub fn ProvokingVertex(mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::ProvokingVertex.f)(mode) } }
#[inline] pub unsafe fn PushDebugGroup(source: GLenum, id: GLuint, length: GLsizei, message: *const GLchar) { mem::transmute::<_, extern "system" fn(source: GLenum, id: GLuint, length: GLsizei, message: *const GLchar) >(storage::PushDebugGroup.f)(source, id, length, message) }
#[inline] pub fn QueryCounter(id: GLuint, target: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum)>(storage::QueryCounter.f)(id, target) } }
#[inline] pub fn ReadBuffer(mode: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum)>(storage::ReadBuffer.f)(mode) } }
#[inline] pub unsafe fn ReadPixels(x: GLint, y: GLint, width: GLsizei, height: GLsizei, format: GLenum, type_: GLenum, pixels: *mut c_void) { mem::transmute::<_, extern "system" fn(x: GLint, y: GLint, width: GLsizei, height: GLsizei, format: GLenum, type_: GLenum, pixels: *mut c_void) >(storage::ReadPixels.f)(x, y, width, height, format, type_, pixels) }
#[inline] pub fn ReleaseShaderCompiler() { unsafe { mem::transmute::<_, extern "system" fn()>(storage::ReleaseShaderCompiler.f)() } }
#[inline] pub fn RenderbufferStorage(target: GLenum, internalformat: GLenum, width: GLsizei, height: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLsizei, GLsizei)>(storage::RenderbufferStorage.f)(target, internalformat, width, height) } }
#[inline] pub fn RenderbufferStorageMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLsizei, GLenum, GLsizei, GLsizei)>(storage::RenderbufferStorageMultisample.f)(target, samples, internalformat, width, height) } }
#[inline] pub fn ResumeTransformFeedback() { unsafe { mem::transmute::<_, extern "system" fn()>(storage::ResumeTransformFeedback.f)() } }
#[inline] pub fn SampleCoverage(value: GLfloat, invert: GLboolean) { unsafe { mem::transmute::<_, extern "system" fn(GLfloat, GLboolean)>(storage::SampleCoverage.f)(value, invert) } }
#[inline] pub fn SampleMaski(maskNumber: GLuint, mask: GLbitfield) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLbitfield)>(storage::SampleMaski.f)(maskNumber, mask) } }
#[inline] pub unsafe fn SamplerParameterIiv(sampler: GLuint, pname: GLenum, param: *const GLint) { mem::transmute::<_, extern "system" fn(sampler: GLuint, pname: GLenum, param: *const GLint) >(storage::SamplerParameterIiv.f)(sampler, pname, param) }
#[inline] pub unsafe fn SamplerParameterIuiv(sampler: GLuint, pname: GLenum, param: *const GLuint) { mem::transmute::<_, extern "system" fn(sampler: GLuint, pname: GLenum, param: *const GLuint) >(storage::SamplerParameterIuiv.f)(sampler, pname, param) }
#[inline] pub fn SamplerParameterf(sampler: GLuint, pname: GLenum, param: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLfloat)>(storage::SamplerParameterf.f)(sampler, pname, param) } }
#[inline] pub unsafe fn SamplerParameterfv(sampler: GLuint, pname: GLenum, param: *const GLfloat) { mem::transmute::<_, extern "system" fn(sampler: GLuint, pname: GLenum, param: *const GLfloat) >(storage::SamplerParameterfv.f)(sampler, pname, param) }
#[inline] pub fn SamplerParameteri(sampler: GLuint, pname: GLenum, param: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLint)>(storage::SamplerParameteri.f)(sampler, pname, param) } }
#[inline] pub unsafe fn SamplerParameteriv(sampler: GLuint, pname: GLenum, param: *const GLint) { mem::transmute::<_, extern "system" fn(sampler: GLuint, pname: GLenum, param: *const GLint) >(storage::SamplerParameteriv.f)(sampler, pname, param) }
#[inline] pub fn Scissor(x: GLint, y: GLint, width: GLsizei, height: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLint, GLsizei, GLsizei)>(storage::Scissor.f)(x, y, width, height) } }
#[inline] pub unsafe fn ScissorArrayv(first: GLuint, count: GLsizei, v: *const GLint) { mem::transmute::<_, extern "system" fn(first: GLuint, count: GLsizei, v: *const GLint) >(storage::ScissorArrayv.f)(first, count, v) }
#[inline] pub fn ScissorIndexed(index: GLuint, left: GLint, bottom: GLint, width: GLsizei, height: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint, GLsizei, GLsizei)>(storage::ScissorIndexed.f)(index, left, bottom, width, height) } }
#[inline] pub unsafe fn ScissorIndexedv(index: GLuint, v: *const GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLint) >(storage::ScissorIndexedv.f)(index, v) }
#[inline] pub fn SecondaryColorP3ui(type_: GLenum, color: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::SecondaryColorP3ui.f)(type_, color) } }
#[inline] pub unsafe fn SecondaryColorP3uiv(type_: GLenum, color: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, color: *const GLuint) >(storage::SecondaryColorP3uiv.f)(type_, color) }
#[inline] pub unsafe fn ShaderBinary(count: GLsizei, shaders: *const GLuint, binaryformat: GLenum, binary: *const c_void, length: GLsizei) { mem::transmute::<_, extern "system" fn(count: GLsizei, shaders: *const GLuint, binaryformat: GLenum, binary: *const c_void, length: GLsizei) >(storage::ShaderBinary.f)(count, shaders, binaryformat, binary, length) }
#[inline] pub unsafe fn ShaderSource(shader: GLuint, count: GLsizei, string: *const *const GLchar, length: *const GLint) { mem::transmute::<_, extern "system" fn(shader: GLuint, count: GLsizei, string: *const *const GLchar, length: *const GLint) >(storage::ShaderSource.f)(shader, count, string, length) }
#[inline] pub fn ShaderStorageBlockBinding(program: GLuint, storageBlockIndex: GLuint, storageBlockBinding: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint, GLuint)>(storage::ShaderStorageBlockBinding.f)(program, storageBlockIndex, storageBlockBinding) } }
#[inline] pub fn StencilFunc(func: GLenum, ref_: GLint, mask: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLint, GLuint)>(storage::StencilFunc.f)(func, ref_, mask) } }
#[inline] pub fn StencilFuncSeparate(face: GLenum, func: GLenum, ref_: GLint, mask: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLint, GLuint)>(storage::StencilFuncSeparate.f)(face, func, ref_, mask) } }
#[inline] pub fn StencilMask(mask: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::StencilMask.f)(mask) } }
#[inline] pub fn StencilMaskSeparate(face: GLenum, mask: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::StencilMaskSeparate.f)(face, mask) } }
#[inline] pub fn StencilOp(fail: GLenum, zfail: GLenum, zpass: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLenum)>(storage::StencilOp.f)(fail, zfail, zpass) } }
#[inline] pub fn StencilOpSeparate(face: GLenum, sfail: GLenum, dpfail: GLenum, dppass: GLenum) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLenum, GLenum)>(storage::StencilOpSeparate.f)(face, sfail, dpfail, dppass) } }
#[inline] pub fn TexBuffer(target: GLenum, internalformat: GLenum, buffer: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLuint)>(storage::TexBuffer.f)(target, internalformat, buffer) } }
#[inline] pub fn TexBufferRange(target: GLenum, internalformat: GLenum, buffer: GLuint, offset: GLintptr, size: GLsizeiptr) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLuint, GLintptr, GLsizeiptr)>(storage::TexBufferRange.f)(target, internalformat, buffer, offset, size) } }
#[inline] pub fn TexCoordP1ui(type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::TexCoordP1ui.f)(type_, coords) } }
#[inline] pub unsafe fn TexCoordP1uiv(type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, coords: *const GLuint) >(storage::TexCoordP1uiv.f)(type_, coords) }
#[inline] pub fn TexCoordP2ui(type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::TexCoordP2ui.f)(type_, coords) } }
#[inline] pub unsafe fn TexCoordP2uiv(type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, coords: *const GLuint) >(storage::TexCoordP2uiv.f)(type_, coords) }
#[inline] pub fn TexCoordP3ui(type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::TexCoordP3ui.f)(type_, coords) } }
#[inline] pub unsafe fn TexCoordP3uiv(type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, coords: *const GLuint) >(storage::TexCoordP3uiv.f)(type_, coords) }
#[inline] pub fn TexCoordP4ui(type_: GLenum, coords: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::TexCoordP4ui.f)(type_, coords) } }
#[inline] pub unsafe fn TexCoordP4uiv(type_: GLenum, coords: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, coords: *const GLuint) >(storage::TexCoordP4uiv.f)(type_, coords) }
#[inline] pub unsafe fn TexImage1D(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) >(storage::TexImage1D.f)(target, level, internalformat, width, border, format, type_, pixels) }
#[inline] pub unsafe fn TexImage2D(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, height: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, height: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) >(storage::TexImage2D.f)(target, level, internalformat, width, height, border, format, type_, pixels) }
#[inline] pub fn TexImage2DMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, fixedsamplelocations: GLboolean) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLsizei, GLenum, GLsizei, GLsizei, GLboolean)>(storage::TexImage2DMultisample.f)(target, samples, internalformat, width, height, fixedsamplelocations) } }
#[inline] pub unsafe fn TexImage3D(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) >(storage::TexImage3D.f)(target, level, internalformat, width, height, depth, border, format, type_, pixels) }
#[inline] pub fn TexImage3DMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei, fixedsamplelocations: GLboolean) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLsizei, GLenum, GLsizei, GLsizei, GLsizei, GLboolean)>(storage::TexImage3DMultisample.f)(target, samples, internalformat, width, height, depth, fixedsamplelocations) } }
#[inline] pub unsafe fn TexParameterIiv(target: GLenum, pname: GLenum, params: *const GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *const GLint) >(storage::TexParameterIiv.f)(target, pname, params) }
#[inline] pub unsafe fn TexParameterIuiv(target: GLenum, pname: GLenum, params: *const GLuint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *const GLuint) >(storage::TexParameterIuiv.f)(target, pname, params) }
#[inline] pub fn TexParameterf(target: GLenum, pname: GLenum, param: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLfloat)>(storage::TexParameterf.f)(target, pname, param) } }
#[inline] pub unsafe fn TexParameterfv(target: GLenum, pname: GLenum, params: *const GLfloat) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *const GLfloat) >(storage::TexParameterfv.f)(target, pname, params) }
#[inline] pub fn TexParameteri(target: GLenum, pname: GLenum, param: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLenum, GLint)>(storage::TexParameteri.f)(target, pname, param) } }
#[inline] pub unsafe fn TexParameteriv(target: GLenum, pname: GLenum, params: *const GLint) { mem::transmute::<_, extern "system" fn(target: GLenum, pname: GLenum, params: *const GLint) >(storage::TexParameteriv.f)(target, pname, params) }
#[inline] pub fn TexStorage1D(target: GLenum, levels: GLsizei, internalformat: GLenum, width: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLsizei, GLenum, GLsizei)>(storage::TexStorage1D.f)(target, levels, internalformat, width) } }
#[inline] pub fn TexStorage2D(target: GLenum, levels: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLsizei, GLenum, GLsizei, GLsizei)>(storage::TexStorage2D.f)(target, levels, internalformat, width, height) } }
#[inline] pub fn TexStorage2DMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, fixedsamplelocations: GLboolean) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLsizei, GLenum, GLsizei, GLsizei, GLboolean)>(storage::TexStorage2DMultisample.f)(target, samples, internalformat, width, height, fixedsamplelocations) } }
#[inline] pub fn TexStorage3D(target: GLenum, levels: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLsizei, GLenum, GLsizei, GLsizei, GLsizei)>(storage::TexStorage3D.f)(target, levels, internalformat, width, height, depth) } }
#[inline] pub fn TexStorage3DMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei, fixedsamplelocations: GLboolean) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLsizei, GLenum, GLsizei, GLsizei, GLsizei, GLboolean)>(storage::TexStorage3DMultisample.f)(target, samples, internalformat, width, height, depth, fixedsamplelocations) } }
#[inline] pub unsafe fn TexSubImage1D(target: GLenum, level: GLint, xoffset: GLint, width: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, xoffset: GLint, width: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) >(storage::TexSubImage1D.f)(target, level, xoffset, width, format, type_, pixels) }
#[inline] pub unsafe fn TexSubImage2D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, width: GLsizei, height: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, width: GLsizei, height: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) >(storage::TexSubImage2D.f)(target, level, xoffset, yoffset, width, height, format, type_, pixels) }
#[inline] pub unsafe fn TexSubImage3D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) { mem::transmute::<_, extern "system" fn(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) >(storage::TexSubImage3D.f)(target, level, xoffset, yoffset, zoffset, width, height, depth, format, type_, pixels) }
#[inline] pub fn TextureView(texture: GLuint, target: GLenum, origtexture: GLuint, internalformat: GLenum, minlevel: GLuint, numlevels: GLuint, minlayer: GLuint, numlayers: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLuint, GLenum, GLuint, GLuint, GLuint, GLuint)>(storage::TextureView.f)(texture, target, origtexture, internalformat, minlevel, numlevels, minlayer, numlayers) } }
#[inline] pub unsafe fn TransformFeedbackVaryings(program: GLuint, count: GLsizei, varyings: *const *const GLchar, bufferMode: GLenum) { mem::transmute::<_, extern "system" fn(program: GLuint, count: GLsizei, varyings: *const *const GLchar, bufferMode: GLenum) >(storage::TransformFeedbackVaryings.f)(program, count, varyings, bufferMode) }
#[inline] pub fn Uniform1d(location: GLint, x: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLdouble)>(storage::Uniform1d.f)(location, x) } }
#[inline] pub unsafe fn Uniform1dv(location: GLint, count: GLsizei, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLdouble) >(storage::Uniform1dv.f)(location, count, value) }
#[inline] pub fn Uniform1f(location: GLint, v0: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLfloat)>(storage::Uniform1f.f)(location, v0) } }
#[inline] pub unsafe fn Uniform1fv(location: GLint, count: GLsizei, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLfloat) >(storage::Uniform1fv.f)(location, count, value) }
#[inline] pub fn Uniform1i(location: GLint, v0: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLint)>(storage::Uniform1i.f)(location, v0) } }
#[inline] pub unsafe fn Uniform1iv(location: GLint, count: GLsizei, value: *const GLint) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLint) >(storage::Uniform1iv.f)(location, count, value) }
#[inline] pub fn Uniform1ui(location: GLint, v0: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLuint)>(storage::Uniform1ui.f)(location, v0) } }
#[inline] pub unsafe fn Uniform1uiv(location: GLint, count: GLsizei, value: *const GLuint) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLuint) >(storage::Uniform1uiv.f)(location, count, value) }
#[inline] pub fn Uniform2d(location: GLint, x: GLdouble, y: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLdouble, GLdouble)>(storage::Uniform2d.f)(location, x, y) } }
#[inline] pub unsafe fn Uniform2dv(location: GLint, count: GLsizei, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLdouble) >(storage::Uniform2dv.f)(location, count, value) }
#[inline] pub fn Uniform2f(location: GLint, v0: GLfloat, v1: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLfloat, GLfloat)>(storage::Uniform2f.f)(location, v0, v1) } }
#[inline] pub unsafe fn Uniform2fv(location: GLint, count: GLsizei, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLfloat) >(storage::Uniform2fv.f)(location, count, value) }
#[inline] pub fn Uniform2i(location: GLint, v0: GLint, v1: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLint, GLint)>(storage::Uniform2i.f)(location, v0, v1) } }
#[inline] pub unsafe fn Uniform2iv(location: GLint, count: GLsizei, value: *const GLint) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLint) >(storage::Uniform2iv.f)(location, count, value) }
#[inline] pub fn Uniform2ui(location: GLint, v0: GLuint, v1: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLuint, GLuint)>(storage::Uniform2ui.f)(location, v0, v1) } }
#[inline] pub unsafe fn Uniform2uiv(location: GLint, count: GLsizei, value: *const GLuint) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLuint) >(storage::Uniform2uiv.f)(location, count, value) }
#[inline] pub fn Uniform3d(location: GLint, x: GLdouble, y: GLdouble, z: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLdouble, GLdouble, GLdouble)>(storage::Uniform3d.f)(location, x, y, z) } }
#[inline] pub unsafe fn Uniform3dv(location: GLint, count: GLsizei, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLdouble) >(storage::Uniform3dv.f)(location, count, value) }
#[inline] pub fn Uniform3f(location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLfloat, GLfloat, GLfloat)>(storage::Uniform3f.f)(location, v0, v1, v2) } }
#[inline] pub unsafe fn Uniform3fv(location: GLint, count: GLsizei, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLfloat) >(storage::Uniform3fv.f)(location, count, value) }
#[inline] pub fn Uniform3i(location: GLint, v0: GLint, v1: GLint, v2: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLint, GLint, GLint)>(storage::Uniform3i.f)(location, v0, v1, v2) } }
#[inline] pub unsafe fn Uniform3iv(location: GLint, count: GLsizei, value: *const GLint) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLint) >(storage::Uniform3iv.f)(location, count, value) }
#[inline] pub fn Uniform3ui(location: GLint, v0: GLuint, v1: GLuint, v2: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLuint, GLuint, GLuint)>(storage::Uniform3ui.f)(location, v0, v1, v2) } }
#[inline] pub unsafe fn Uniform3uiv(location: GLint, count: GLsizei, value: *const GLuint) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLuint) >(storage::Uniform3uiv.f)(location, count, value) }
#[inline] pub fn Uniform4d(location: GLint, x: GLdouble, y: GLdouble, z: GLdouble, w: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLdouble, GLdouble, GLdouble, GLdouble)>(storage::Uniform4d.f)(location, x, y, z, w) } }
#[inline] pub unsafe fn Uniform4dv(location: GLint, count: GLsizei, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLdouble) >(storage::Uniform4dv.f)(location, count, value) }
#[inline] pub fn Uniform4f(location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat, v3: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLfloat, GLfloat, GLfloat, GLfloat)>(storage::Uniform4f.f)(location, v0, v1, v2, v3) } }
#[inline] pub unsafe fn Uniform4fv(location: GLint, count: GLsizei, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLfloat) >(storage::Uniform4fv.f)(location, count, value) }
#[inline] pub fn Uniform4i(location: GLint, v0: GLint, v1: GLint, v2: GLint, v3: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLint, GLint, GLint, GLint)>(storage::Uniform4i.f)(location, v0, v1, v2, v3) } }
#[inline] pub unsafe fn Uniform4iv(location: GLint, count: GLsizei, value: *const GLint) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLint) >(storage::Uniform4iv.f)(location, count, value) }
#[inline] pub fn Uniform4ui(location: GLint, v0: GLuint, v1: GLuint, v2: GLuint, v3: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLuint, GLuint, GLuint, GLuint)>(storage::Uniform4ui.f)(location, v0, v1, v2, v3) } }
#[inline] pub unsafe fn Uniform4uiv(location: GLint, count: GLsizei, value: *const GLuint) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, value: *const GLuint) >(storage::Uniform4uiv.f)(location, count, value) }
#[inline] pub fn UniformBlockBinding(program: GLuint, uniformBlockIndex: GLuint, uniformBlockBinding: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint, GLuint)>(storage::UniformBlockBinding.f)(program, uniformBlockIndex, uniformBlockBinding) } }
#[inline] pub unsafe fn UniformMatrix2dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix2dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix2fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix2fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix2x3dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix2x3dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix2x3fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix2x3fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix2x4dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix2x4dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix2x4fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix2x4fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix3dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix3dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix3fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix3fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix3x2dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix3x2dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix3x2fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix3x2fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix3x4dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix3x4dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix3x4fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix3x4fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix4dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix4dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix4fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix4fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix4x2dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix4x2dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix4x2fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix4x2fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix4x3dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) >(storage::UniformMatrix4x3dv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformMatrix4x3fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { mem::transmute::<_, extern "system" fn(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) >(storage::UniformMatrix4x3fv.f)(location, count, transpose, value) }
#[inline] pub unsafe fn UniformSubroutinesuiv(shadertype: GLenum, count: GLsizei, indices: *const GLuint) { mem::transmute::<_, extern "system" fn(shadertype: GLenum, count: GLsizei, indices: *const GLuint) >(storage::UniformSubroutinesuiv.f)(shadertype, count, indices) }
#[inline] pub fn UnmapBuffer(target: GLenum) -> GLboolean { unsafe { mem::transmute::<_, extern "system" fn(GLenum) -> GLboolean>(storage::UnmapBuffer.f)(target) } }
#[inline] pub fn UseProgram(program: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::UseProgram.f)(program) } }
#[inline] pub fn UseProgramStages(pipeline: GLuint, stages: GLbitfield, program: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLbitfield, GLuint)>(storage::UseProgramStages.f)(pipeline, stages, program) } }
#[inline] pub fn ValidateProgram(program: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::ValidateProgram.f)(program) } }
#[inline] pub fn ValidateProgramPipeline(pipeline: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint)>(storage::ValidateProgramPipeline.f)(pipeline) } }
#[inline] pub fn VertexAttrib1d(index: GLuint, x: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble)>(storage::VertexAttrib1d.f)(index, x) } }
#[inline] pub unsafe fn VertexAttrib1dv(index: GLuint, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLdouble) >(storage::VertexAttrib1dv.f)(index, v) }
#[inline] pub fn VertexAttrib1f(index: GLuint, x: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLfloat)>(storage::VertexAttrib1f.f)(index, x) } }
#[inline] pub unsafe fn VertexAttrib1fv(index: GLuint, v: *const GLfloat) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLfloat) >(storage::VertexAttrib1fv.f)(index, v) }
#[inline] pub fn VertexAttrib1s(index: GLuint, x: GLshort) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLshort)>(storage::VertexAttrib1s.f)(index, x) } }
#[inline] pub unsafe fn VertexAttrib1sv(index: GLuint, v: *const GLshort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLshort) >(storage::VertexAttrib1sv.f)(index, v) }
#[inline] pub fn VertexAttrib2d(index: GLuint, x: GLdouble, y: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble, GLdouble)>(storage::VertexAttrib2d.f)(index, x, y) } }
#[inline] pub unsafe fn VertexAttrib2dv(index: GLuint, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLdouble) >(storage::VertexAttrib2dv.f)(index, v) }
#[inline] pub fn VertexAttrib2f(index: GLuint, x: GLfloat, y: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLfloat, GLfloat)>(storage::VertexAttrib2f.f)(index, x, y) } }
#[inline] pub unsafe fn VertexAttrib2fv(index: GLuint, v: *const GLfloat) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLfloat) >(storage::VertexAttrib2fv.f)(index, v) }
#[inline] pub fn VertexAttrib2s(index: GLuint, x: GLshort, y: GLshort) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLshort, GLshort)>(storage::VertexAttrib2s.f)(index, x, y) } }
#[inline] pub unsafe fn VertexAttrib2sv(index: GLuint, v: *const GLshort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLshort) >(storage::VertexAttrib2sv.f)(index, v) }
#[inline] pub fn VertexAttrib3d(index: GLuint, x: GLdouble, y: GLdouble, z: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble, GLdouble, GLdouble)>(storage::VertexAttrib3d.f)(index, x, y, z) } }
#[inline] pub unsafe fn VertexAttrib3dv(index: GLuint, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLdouble) >(storage::VertexAttrib3dv.f)(index, v) }
#[inline] pub fn VertexAttrib3f(index: GLuint, x: GLfloat, y: GLfloat, z: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLfloat, GLfloat, GLfloat)>(storage::VertexAttrib3f.f)(index, x, y, z) } }
#[inline] pub unsafe fn VertexAttrib3fv(index: GLuint, v: *const GLfloat) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLfloat) >(storage::VertexAttrib3fv.f)(index, v) }
#[inline] pub fn VertexAttrib3s(index: GLuint, x: GLshort, y: GLshort, z: GLshort) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLshort, GLshort, GLshort)>(storage::VertexAttrib3s.f)(index, x, y, z) } }
#[inline] pub unsafe fn VertexAttrib3sv(index: GLuint, v: *const GLshort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLshort) >(storage::VertexAttrib3sv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4Nbv(index: GLuint, v: *const GLbyte) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLbyte) >(storage::VertexAttrib4Nbv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4Niv(index: GLuint, v: *const GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLint) >(storage::VertexAttrib4Niv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4Nsv(index: GLuint, v: *const GLshort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLshort) >(storage::VertexAttrib4Nsv.f)(index, v) }
#[inline] pub fn VertexAttrib4Nub(index: GLuint, x: GLubyte, y: GLubyte, z: GLubyte, w: GLubyte) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLubyte, GLubyte, GLubyte, GLubyte)>(storage::VertexAttrib4Nub.f)(index, x, y, z, w) } }
#[inline] pub unsafe fn VertexAttrib4Nubv(index: GLuint, v: *const GLubyte) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLubyte) >(storage::VertexAttrib4Nubv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4Nuiv(index: GLuint, v: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLuint) >(storage::VertexAttrib4Nuiv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4Nusv(index: GLuint, v: *const GLushort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLushort) >(storage::VertexAttrib4Nusv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4bv(index: GLuint, v: *const GLbyte) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLbyte) >(storage::VertexAttrib4bv.f)(index, v) }
#[inline] pub fn VertexAttrib4d(index: GLuint, x: GLdouble, y: GLdouble, z: GLdouble, w: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble, GLdouble, GLdouble, GLdouble)>(storage::VertexAttrib4d.f)(index, x, y, z, w) } }
#[inline] pub unsafe fn VertexAttrib4dv(index: GLuint, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLdouble) >(storage::VertexAttrib4dv.f)(index, v) }
#[inline] pub fn VertexAttrib4f(index: GLuint, x: GLfloat, y: GLfloat, z: GLfloat, w: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLfloat, GLfloat, GLfloat, GLfloat)>(storage::VertexAttrib4f.f)(index, x, y, z, w) } }
#[inline] pub unsafe fn VertexAttrib4fv(index: GLuint, v: *const GLfloat) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLfloat) >(storage::VertexAttrib4fv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4iv(index: GLuint, v: *const GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLint) >(storage::VertexAttrib4iv.f)(index, v) }
#[inline] pub fn VertexAttrib4s(index: GLuint, x: GLshort, y: GLshort, z: GLshort, w: GLshort) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLshort, GLshort, GLshort, GLshort)>(storage::VertexAttrib4s.f)(index, x, y, z, w) } }
#[inline] pub unsafe fn VertexAttrib4sv(index: GLuint, v: *const GLshort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLshort) >(storage::VertexAttrib4sv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4ubv(index: GLuint, v: *const GLubyte) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLubyte) >(storage::VertexAttrib4ubv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4uiv(index: GLuint, v: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLuint) >(storage::VertexAttrib4uiv.f)(index, v) }
#[inline] pub unsafe fn VertexAttrib4usv(index: GLuint, v: *const GLushort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLushort) >(storage::VertexAttrib4usv.f)(index, v) }
#[inline] pub fn VertexAttribBinding(attribindex: GLuint, bindingindex: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint)>(storage::VertexAttribBinding.f)(attribindex, bindingindex) } }
#[inline] pub fn VertexAttribDivisor(index: GLuint, divisor: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint)>(storage::VertexAttribDivisor.f)(index, divisor) } }
#[inline] pub fn VertexAttribFormat(attribindex: GLuint, size: GLint, type_: GLenum, normalized: GLboolean, relativeoffset: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLenum, GLboolean, GLuint)>(storage::VertexAttribFormat.f)(attribindex, size, type_, normalized, relativeoffset) } }
#[inline] pub fn VertexAttribI1i(index: GLuint, x: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint)>(storage::VertexAttribI1i.f)(index, x) } }
#[inline] pub unsafe fn VertexAttribI1iv(index: GLuint, v: *const GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLint) >(storage::VertexAttribI1iv.f)(index, v) }
#[inline] pub fn VertexAttribI1ui(index: GLuint, x: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint)>(storage::VertexAttribI1ui.f)(index, x) } }
#[inline] pub unsafe fn VertexAttribI1uiv(index: GLuint, v: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLuint) >(storage::VertexAttribI1uiv.f)(index, v) }
#[inline] pub fn VertexAttribI2i(index: GLuint, x: GLint, y: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint)>(storage::VertexAttribI2i.f)(index, x, y) } }
#[inline] pub unsafe fn VertexAttribI2iv(index: GLuint, v: *const GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLint) >(storage::VertexAttribI2iv.f)(index, v) }
#[inline] pub fn VertexAttribI2ui(index: GLuint, x: GLuint, y: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint, GLuint)>(storage::VertexAttribI2ui.f)(index, x, y) } }
#[inline] pub unsafe fn VertexAttribI2uiv(index: GLuint, v: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLuint) >(storage::VertexAttribI2uiv.f)(index, v) }
#[inline] pub fn VertexAttribI3i(index: GLuint, x: GLint, y: GLint, z: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint, GLint)>(storage::VertexAttribI3i.f)(index, x, y, z) } }
#[inline] pub unsafe fn VertexAttribI3iv(index: GLuint, v: *const GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLint) >(storage::VertexAttribI3iv.f)(index, v) }
#[inline] pub fn VertexAttribI3ui(index: GLuint, x: GLuint, y: GLuint, z: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint, GLuint, GLuint)>(storage::VertexAttribI3ui.f)(index, x, y, z) } }
#[inline] pub unsafe fn VertexAttribI3uiv(index: GLuint, v: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLuint) >(storage::VertexAttribI3uiv.f)(index, v) }
#[inline] pub unsafe fn VertexAttribI4bv(index: GLuint, v: *const GLbyte) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLbyte) >(storage::VertexAttribI4bv.f)(index, v) }
#[inline] pub fn VertexAttribI4i(index: GLuint, x: GLint, y: GLint, z: GLint, w: GLint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLint, GLint, GLint)>(storage::VertexAttribI4i.f)(index, x, y, z, w) } }
#[inline] pub unsafe fn VertexAttribI4iv(index: GLuint, v: *const GLint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLint) >(storage::VertexAttribI4iv.f)(index, v) }
#[inline] pub unsafe fn VertexAttribI4sv(index: GLuint, v: *const GLshort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLshort) >(storage::VertexAttribI4sv.f)(index, v) }
#[inline] pub unsafe fn VertexAttribI4ubv(index: GLuint, v: *const GLubyte) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLubyte) >(storage::VertexAttribI4ubv.f)(index, v) }
#[inline] pub fn VertexAttribI4ui(index: GLuint, x: GLuint, y: GLuint, z: GLuint, w: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint, GLuint, GLuint, GLuint)>(storage::VertexAttribI4ui.f)(index, x, y, z, w) } }
#[inline] pub unsafe fn VertexAttribI4uiv(index: GLuint, v: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLuint) >(storage::VertexAttribI4uiv.f)(index, v) }
#[inline] pub unsafe fn VertexAttribI4usv(index: GLuint, v: *const GLushort) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLushort) >(storage::VertexAttribI4usv.f)(index, v) }
#[inline] pub fn VertexAttribIFormat(attribindex: GLuint, size: GLint, type_: GLenum, relativeoffset: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLenum, GLuint)>(storage::VertexAttribIFormat.f)(attribindex, size, type_, relativeoffset) } }
#[inline] pub unsafe fn VertexAttribIPointer(index: GLuint, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const c_void) { mem::transmute::<_, extern "system" fn(index: GLuint, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const c_void) >(storage::VertexAttribIPointer.f)(index, size, type_, stride, pointer) }
#[inline] pub fn VertexAttribL1d(index: GLuint, x: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble)>(storage::VertexAttribL1d.f)(index, x) } }
#[inline] pub unsafe fn VertexAttribL1dv(index: GLuint, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLdouble) >(storage::VertexAttribL1dv.f)(index, v) }
#[inline] pub fn VertexAttribL2d(index: GLuint, x: GLdouble, y: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble, GLdouble)>(storage::VertexAttribL2d.f)(index, x, y) } }
#[inline] pub unsafe fn VertexAttribL2dv(index: GLuint, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLdouble) >(storage::VertexAttribL2dv.f)(index, v) }
#[inline] pub fn VertexAttribL3d(index: GLuint, x: GLdouble, y: GLdouble, z: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble, GLdouble, GLdouble)>(storage::VertexAttribL3d.f)(index, x, y, z) } }
#[inline] pub unsafe fn VertexAttribL3dv(index: GLuint, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLdouble) >(storage::VertexAttribL3dv.f)(index, v) }
#[inline] pub fn VertexAttribL4d(index: GLuint, x: GLdouble, y: GLdouble, z: GLdouble, w: GLdouble) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLdouble, GLdouble, GLdouble, GLdouble)>(storage::VertexAttribL4d.f)(index, x, y, z, w) } }
#[inline] pub unsafe fn VertexAttribL4dv(index: GLuint, v: *const GLdouble) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLdouble) >(storage::VertexAttribL4dv.f)(index, v) }
#[inline] pub fn VertexAttribLFormat(attribindex: GLuint, size: GLint, type_: GLenum, relativeoffset: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLint, GLenum, GLuint)>(storage::VertexAttribLFormat.f)(attribindex, size, type_, relativeoffset) } }
#[inline] pub unsafe fn VertexAttribLPointer(index: GLuint, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const c_void) { mem::transmute::<_, extern "system" fn(index: GLuint, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const c_void) >(storage::VertexAttribLPointer.f)(index, size, type_, stride, pointer) }
#[inline] pub fn VertexAttribP1ui(index: GLuint, type_: GLenum, normalized: GLboolean, value: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLboolean, GLuint)>(storage::VertexAttribP1ui.f)(index, type_, normalized, value) } }
#[inline] pub unsafe fn VertexAttribP1uiv(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) >(storage::VertexAttribP1uiv.f)(index, type_, normalized, value) }
#[inline] pub fn VertexAttribP2ui(index: GLuint, type_: GLenum, normalized: GLboolean, value: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLboolean, GLuint)>(storage::VertexAttribP2ui.f)(index, type_, normalized, value) } }
#[inline] pub unsafe fn VertexAttribP2uiv(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) >(storage::VertexAttribP2uiv.f)(index, type_, normalized, value) }
#[inline] pub fn VertexAttribP3ui(index: GLuint, type_: GLenum, normalized: GLboolean, value: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLboolean, GLuint)>(storage::VertexAttribP3ui.f)(index, type_, normalized, value) } }
#[inline] pub unsafe fn VertexAttribP3uiv(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) >(storage::VertexAttribP3uiv.f)(index, type_, normalized, value) }
#[inline] pub fn VertexAttribP4ui(index: GLuint, type_: GLenum, normalized: GLboolean, value: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLenum, GLboolean, GLuint)>(storage::VertexAttribP4ui.f)(index, type_, normalized, value) } }
#[inline] pub unsafe fn VertexAttribP4uiv(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) { mem::transmute::<_, extern "system" fn(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) >(storage::VertexAttribP4uiv.f)(index, type_, normalized, value) }
#[inline] pub unsafe fn VertexAttribPointer(index: GLuint, size: GLint, type_: GLenum, normalized: GLboolean, stride: GLsizei, pointer: *const c_void) { mem::transmute::<_, extern "system" fn(index: GLuint, size: GLint, type_: GLenum, normalized: GLboolean, stride: GLsizei, pointer: *const c_void) >(storage::VertexAttribPointer.f)(index, size, type_, normalized, stride, pointer) }
#[inline] pub fn VertexBindingDivisor(bindingindex: GLuint, divisor: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLuint)>(storage::VertexBindingDivisor.f)(bindingindex, divisor) } }
#[inline] pub fn VertexP2ui(type_: GLenum, value: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::VertexP2ui.f)(type_, value) } }
#[inline] pub unsafe fn VertexP2uiv(type_: GLenum, value: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, value: *const GLuint) >(storage::VertexP2uiv.f)(type_, value) }
#[inline] pub fn VertexP3ui(type_: GLenum, value: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::VertexP3ui.f)(type_, value) } }
#[inline] pub unsafe fn VertexP3uiv(type_: GLenum, value: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, value: *const GLuint) >(storage::VertexP3uiv.f)(type_, value) }
#[inline] pub fn VertexP4ui(type_: GLenum, value: GLuint) { unsafe { mem::transmute::<_, extern "system" fn(GLenum, GLuint)>(storage::VertexP4ui.f)(type_, value) } }
#[inline] pub unsafe fn VertexP4uiv(type_: GLenum, value: *const GLuint) { mem::transmute::<_, extern "system" fn(type_: GLenum, value: *const GLuint) >(storage::VertexP4uiv.f)(type_, value) }
#[inline] pub fn Viewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei) { unsafe { mem::transmute::<_, extern "system" fn(GLint, GLint, GLsizei, GLsizei)>(storage::Viewport.f)(x, y, width, height) } }
#[inline] pub unsafe fn ViewportArrayv(first: GLuint, count: GLsizei, v: *const GLfloat) { mem::transmute::<_, extern "system" fn(first: GLuint, count: GLsizei, v: *const GLfloat) >(storage::ViewportArrayv.f)(first, count, v) }
#[inline] pub fn ViewportIndexedf(index: GLuint, x: GLfloat, y: GLfloat, w: GLfloat, h: GLfloat) { unsafe { mem::transmute::<_, extern "system" fn(GLuint, GLfloat, GLfloat, GLfloat, GLfloat)>(storage::ViewportIndexedf.f)(index, x, y, w, h) } }
#[inline] pub unsafe fn ViewportIndexedfv(index: GLuint, v: *const GLfloat) { mem::transmute::<_, extern "system" fn(index: GLuint, v: *const GLfloat) >(storage::ViewportIndexedfv.f)(index, v) }
#[inline] pub fn WaitSync(sync: GLsync, flags: GLbitfield, timeout: GLuint64) { unsafe { mem::transmute::<_, extern "system" fn(GLsync, GLbitfield, GLuint64)>(storage::WaitSync.f)(sync, flags, timeout) } }

pub struct FnPtr { f: *const libc::c_void, is_loaded: bool }

impl FnPtr {
    pub fn new(ptr: *const libc::c_void, failing_fn: *const libc::c_void) -> FnPtr {
        if ptr.is_null() {
            FnPtr { f: failing_fn, is_loaded: false }
        } else {
            FnPtr { f: ptr, is_loaded: true }
        }
    }
}

mod storage {
    use libc;
    use failing;
    use FnPtr;
    
    pub static mut ActiveShaderProgram: FnPtr = FnPtr { f: failing::ActiveShaderProgram as *const libc::c_void, is_loaded: false };
    pub static mut ActiveTexture: FnPtr = FnPtr { f: failing::ActiveTexture as *const libc::c_void, is_loaded: false };
    pub static mut AttachShader: FnPtr = FnPtr { f: failing::AttachShader as *const libc::c_void, is_loaded: false };
    pub static mut BeginConditionalRender: FnPtr = FnPtr { f: failing::BeginConditionalRender as *const libc::c_void, is_loaded: false };
    pub static mut BeginQuery: FnPtr = FnPtr { f: failing::BeginQuery as *const libc::c_void, is_loaded: false };
    pub static mut BeginQueryIndexed: FnPtr = FnPtr { f: failing::BeginQueryIndexed as *const libc::c_void, is_loaded: false };
    pub static mut BeginTransformFeedback: FnPtr = FnPtr { f: failing::BeginTransformFeedback as *const libc::c_void, is_loaded: false };
    pub static mut BindAttribLocation: FnPtr = FnPtr { f: failing::BindAttribLocation as *const libc::c_void, is_loaded: false };
    pub static mut BindBuffer: FnPtr = FnPtr { f: failing::BindBuffer as *const libc::c_void, is_loaded: false };
    pub static mut BindBufferBase: FnPtr = FnPtr { f: failing::BindBufferBase as *const libc::c_void, is_loaded: false };
    pub static mut BindBufferRange: FnPtr = FnPtr { f: failing::BindBufferRange as *const libc::c_void, is_loaded: false };
    pub static mut BindFragDataLocation: FnPtr = FnPtr { f: failing::BindFragDataLocation as *const libc::c_void, is_loaded: false };
    pub static mut BindFragDataLocationIndexed: FnPtr = FnPtr { f: failing::BindFragDataLocationIndexed as *const libc::c_void, is_loaded: false };
    pub static mut BindFramebuffer: FnPtr = FnPtr { f: failing::BindFramebuffer as *const libc::c_void, is_loaded: false };
    pub static mut BindImageTexture: FnPtr = FnPtr { f: failing::BindImageTexture as *const libc::c_void, is_loaded: false };
    pub static mut BindProgramPipeline: FnPtr = FnPtr { f: failing::BindProgramPipeline as *const libc::c_void, is_loaded: false };
    pub static mut BindRenderbuffer: FnPtr = FnPtr { f: failing::BindRenderbuffer as *const libc::c_void, is_loaded: false };
    pub static mut BindSampler: FnPtr = FnPtr { f: failing::BindSampler as *const libc::c_void, is_loaded: false };
    pub static mut BindTexture: FnPtr = FnPtr { f: failing::BindTexture as *const libc::c_void, is_loaded: false };
    pub static mut BindTransformFeedback: FnPtr = FnPtr { f: failing::BindTransformFeedback as *const libc::c_void, is_loaded: false };
    pub static mut BindVertexArray: FnPtr = FnPtr { f: failing::BindVertexArray as *const libc::c_void, is_loaded: false };
    pub static mut BindVertexBuffer: FnPtr = FnPtr { f: failing::BindVertexBuffer as *const libc::c_void, is_loaded: false };
    pub static mut BlendColor: FnPtr = FnPtr { f: failing::BlendColor as *const libc::c_void, is_loaded: false };
    pub static mut BlendEquation: FnPtr = FnPtr { f: failing::BlendEquation as *const libc::c_void, is_loaded: false };
    pub static mut BlendEquationSeparate: FnPtr = FnPtr { f: failing::BlendEquationSeparate as *const libc::c_void, is_loaded: false };
    pub static mut BlendEquationSeparatei: FnPtr = FnPtr { f: failing::BlendEquationSeparatei as *const libc::c_void, is_loaded: false };
    pub static mut BlendEquationi: FnPtr = FnPtr { f: failing::BlendEquationi as *const libc::c_void, is_loaded: false };
    pub static mut BlendFunc: FnPtr = FnPtr { f: failing::BlendFunc as *const libc::c_void, is_loaded: false };
    pub static mut BlendFuncSeparate: FnPtr = FnPtr { f: failing::BlendFuncSeparate as *const libc::c_void, is_loaded: false };
    pub static mut BlendFuncSeparatei: FnPtr = FnPtr { f: failing::BlendFuncSeparatei as *const libc::c_void, is_loaded: false };
    pub static mut BlendFunci: FnPtr = FnPtr { f: failing::BlendFunci as *const libc::c_void, is_loaded: false };
    pub static mut BlitFramebuffer: FnPtr = FnPtr { f: failing::BlitFramebuffer as *const libc::c_void, is_loaded: false };
    pub static mut BufferData: FnPtr = FnPtr { f: failing::BufferData as *const libc::c_void, is_loaded: false };
    pub static mut BufferSubData: FnPtr = FnPtr { f: failing::BufferSubData as *const libc::c_void, is_loaded: false };
    pub static mut CheckFramebufferStatus: FnPtr = FnPtr { f: failing::CheckFramebufferStatus as *const libc::c_void, is_loaded: false };
    pub static mut ClampColor: FnPtr = FnPtr { f: failing::ClampColor as *const libc::c_void, is_loaded: false };
    pub static mut Clear: FnPtr = FnPtr { f: failing::Clear as *const libc::c_void, is_loaded: false };
    pub static mut ClearBufferData: FnPtr = FnPtr { f: failing::ClearBufferData as *const libc::c_void, is_loaded: false };
    pub static mut ClearBufferSubData: FnPtr = FnPtr { f: failing::ClearBufferSubData as *const libc::c_void, is_loaded: false };
    pub static mut ClearBufferfi: FnPtr = FnPtr { f: failing::ClearBufferfi as *const libc::c_void, is_loaded: false };
    pub static mut ClearBufferfv: FnPtr = FnPtr { f: failing::ClearBufferfv as *const libc::c_void, is_loaded: false };
    pub static mut ClearBufferiv: FnPtr = FnPtr { f: failing::ClearBufferiv as *const libc::c_void, is_loaded: false };
    pub static mut ClearBufferuiv: FnPtr = FnPtr { f: failing::ClearBufferuiv as *const libc::c_void, is_loaded: false };
    pub static mut ClearColor: FnPtr = FnPtr { f: failing::ClearColor as *const libc::c_void, is_loaded: false };
    pub static mut ClearDepth: FnPtr = FnPtr { f: failing::ClearDepth as *const libc::c_void, is_loaded: false };
    pub static mut ClearDepthf: FnPtr = FnPtr { f: failing::ClearDepthf as *const libc::c_void, is_loaded: false };
    pub static mut ClearStencil: FnPtr = FnPtr { f: failing::ClearStencil as *const libc::c_void, is_loaded: false };
    pub static mut ClientWaitSync: FnPtr = FnPtr { f: failing::ClientWaitSync as *const libc::c_void, is_loaded: false };
    pub static mut ColorMask: FnPtr = FnPtr { f: failing::ColorMask as *const libc::c_void, is_loaded: false };
    pub static mut ColorMaski: FnPtr = FnPtr { f: failing::ColorMaski as *const libc::c_void, is_loaded: false };
    pub static mut ColorP3ui: FnPtr = FnPtr { f: failing::ColorP3ui as *const libc::c_void, is_loaded: false };
    pub static mut ColorP3uiv: FnPtr = FnPtr { f: failing::ColorP3uiv as *const libc::c_void, is_loaded: false };
    pub static mut ColorP4ui: FnPtr = FnPtr { f: failing::ColorP4ui as *const libc::c_void, is_loaded: false };
    pub static mut ColorP4uiv: FnPtr = FnPtr { f: failing::ColorP4uiv as *const libc::c_void, is_loaded: false };
    pub static mut CompileShader: FnPtr = FnPtr { f: failing::CompileShader as *const libc::c_void, is_loaded: false };
    pub static mut CompressedTexImage1D: FnPtr = FnPtr { f: failing::CompressedTexImage1D as *const libc::c_void, is_loaded: false };
    pub static mut CompressedTexImage2D: FnPtr = FnPtr { f: failing::CompressedTexImage2D as *const libc::c_void, is_loaded: false };
    pub static mut CompressedTexImage3D: FnPtr = FnPtr { f: failing::CompressedTexImage3D as *const libc::c_void, is_loaded: false };
    pub static mut CompressedTexSubImage1D: FnPtr = FnPtr { f: failing::CompressedTexSubImage1D as *const libc::c_void, is_loaded: false };
    pub static mut CompressedTexSubImage2D: FnPtr = FnPtr { f: failing::CompressedTexSubImage2D as *const libc::c_void, is_loaded: false };
    pub static mut CompressedTexSubImage3D: FnPtr = FnPtr { f: failing::CompressedTexSubImage3D as *const libc::c_void, is_loaded: false };
    pub static mut CopyBufferSubData: FnPtr = FnPtr { f: failing::CopyBufferSubData as *const libc::c_void, is_loaded: false };
    pub static mut CopyImageSubData: FnPtr = FnPtr { f: failing::CopyImageSubData as *const libc::c_void, is_loaded: false };
    pub static mut CopyTexImage1D: FnPtr = FnPtr { f: failing::CopyTexImage1D as *const libc::c_void, is_loaded: false };
    pub static mut CopyTexImage2D: FnPtr = FnPtr { f: failing::CopyTexImage2D as *const libc::c_void, is_loaded: false };
    pub static mut CopyTexSubImage1D: FnPtr = FnPtr { f: failing::CopyTexSubImage1D as *const libc::c_void, is_loaded: false };
    pub static mut CopyTexSubImage2D: FnPtr = FnPtr { f: failing::CopyTexSubImage2D as *const libc::c_void, is_loaded: false };
    pub static mut CopyTexSubImage3D: FnPtr = FnPtr { f: failing::CopyTexSubImage3D as *const libc::c_void, is_loaded: false };
    pub static mut CreateProgram: FnPtr = FnPtr { f: failing::CreateProgram as *const libc::c_void, is_loaded: false };
    pub static mut CreateShader: FnPtr = FnPtr { f: failing::CreateShader as *const libc::c_void, is_loaded: false };
    pub static mut CreateShaderProgramv: FnPtr = FnPtr { f: failing::CreateShaderProgramv as *const libc::c_void, is_loaded: false };
    pub static mut CullFace: FnPtr = FnPtr { f: failing::CullFace as *const libc::c_void, is_loaded: false };
    pub static mut DebugMessageCallback: FnPtr = FnPtr { f: failing::DebugMessageCallback as *const libc::c_void, is_loaded: false };
    pub static mut DebugMessageControl: FnPtr = FnPtr { f: failing::DebugMessageControl as *const libc::c_void, is_loaded: false };
    pub static mut DebugMessageInsert: FnPtr = FnPtr { f: failing::DebugMessageInsert as *const libc::c_void, is_loaded: false };
    pub static mut DeleteBuffers: FnPtr = FnPtr { f: failing::DeleteBuffers as *const libc::c_void, is_loaded: false };
    pub static mut DeleteFramebuffers: FnPtr = FnPtr { f: failing::DeleteFramebuffers as *const libc::c_void, is_loaded: false };
    pub static mut DeleteProgram: FnPtr = FnPtr { f: failing::DeleteProgram as *const libc::c_void, is_loaded: false };
    pub static mut DeleteProgramPipelines: FnPtr = FnPtr { f: failing::DeleteProgramPipelines as *const libc::c_void, is_loaded: false };
    pub static mut DeleteQueries: FnPtr = FnPtr { f: failing::DeleteQueries as *const libc::c_void, is_loaded: false };
    pub static mut DeleteRenderbuffers: FnPtr = FnPtr { f: failing::DeleteRenderbuffers as *const libc::c_void, is_loaded: false };
    pub static mut DeleteSamplers: FnPtr = FnPtr { f: failing::DeleteSamplers as *const libc::c_void, is_loaded: false };
    pub static mut DeleteShader: FnPtr = FnPtr { f: failing::DeleteShader as *const libc::c_void, is_loaded: false };
    pub static mut DeleteSync: FnPtr = FnPtr { f: failing::DeleteSync as *const libc::c_void, is_loaded: false };
    pub static mut DeleteTextures: FnPtr = FnPtr { f: failing::DeleteTextures as *const libc::c_void, is_loaded: false };
    pub static mut DeleteTransformFeedbacks: FnPtr = FnPtr { f: failing::DeleteTransformFeedbacks as *const libc::c_void, is_loaded: false };
    pub static mut DeleteVertexArrays: FnPtr = FnPtr { f: failing::DeleteVertexArrays as *const libc::c_void, is_loaded: false };
    pub static mut DepthFunc: FnPtr = FnPtr { f: failing::DepthFunc as *const libc::c_void, is_loaded: false };
    pub static mut DepthMask: FnPtr = FnPtr { f: failing::DepthMask as *const libc::c_void, is_loaded: false };
    pub static mut DepthRange: FnPtr = FnPtr { f: failing::DepthRange as *const libc::c_void, is_loaded: false };
    pub static mut DepthRangeArrayv: FnPtr = FnPtr { f: failing::DepthRangeArrayv as *const libc::c_void, is_loaded: false };
    pub static mut DepthRangeIndexed: FnPtr = FnPtr { f: failing::DepthRangeIndexed as *const libc::c_void, is_loaded: false };
    pub static mut DepthRangef: FnPtr = FnPtr { f: failing::DepthRangef as *const libc::c_void, is_loaded: false };
    pub static mut DetachShader: FnPtr = FnPtr { f: failing::DetachShader as *const libc::c_void, is_loaded: false };
    pub static mut Disable: FnPtr = FnPtr { f: failing::Disable as *const libc::c_void, is_loaded: false };
    pub static mut DisableVertexAttribArray: FnPtr = FnPtr { f: failing::DisableVertexAttribArray as *const libc::c_void, is_loaded: false };
    pub static mut Disablei: FnPtr = FnPtr { f: failing::Disablei as *const libc::c_void, is_loaded: false };
    pub static mut DispatchCompute: FnPtr = FnPtr { f: failing::DispatchCompute as *const libc::c_void, is_loaded: false };
    pub static mut DispatchComputeIndirect: FnPtr = FnPtr { f: failing::DispatchComputeIndirect as *const libc::c_void, is_loaded: false };
    pub static mut DrawArrays: FnPtr = FnPtr { f: failing::DrawArrays as *const libc::c_void, is_loaded: false };
    pub static mut DrawArraysIndirect: FnPtr = FnPtr { f: failing::DrawArraysIndirect as *const libc::c_void, is_loaded: false };
    pub static mut DrawArraysInstanced: FnPtr = FnPtr { f: failing::DrawArraysInstanced as *const libc::c_void, is_loaded: false };
    pub static mut DrawArraysInstancedBaseInstance: FnPtr = FnPtr { f: failing::DrawArraysInstancedBaseInstance as *const libc::c_void, is_loaded: false };
    pub static mut DrawBuffer: FnPtr = FnPtr { f: failing::DrawBuffer as *const libc::c_void, is_loaded: false };
    pub static mut DrawBuffers: FnPtr = FnPtr { f: failing::DrawBuffers as *const libc::c_void, is_loaded: false };
    pub static mut DrawElements: FnPtr = FnPtr { f: failing::DrawElements as *const libc::c_void, is_loaded: false };
    pub static mut DrawElementsBaseVertex: FnPtr = FnPtr { f: failing::DrawElementsBaseVertex as *const libc::c_void, is_loaded: false };
    pub static mut DrawElementsIndirect: FnPtr = FnPtr { f: failing::DrawElementsIndirect as *const libc::c_void, is_loaded: false };
    pub static mut DrawElementsInstanced: FnPtr = FnPtr { f: failing::DrawElementsInstanced as *const libc::c_void, is_loaded: false };
    pub static mut DrawElementsInstancedBaseInstance: FnPtr = FnPtr { f: failing::DrawElementsInstancedBaseInstance as *const libc::c_void, is_loaded: false };
    pub static mut DrawElementsInstancedBaseVertex: FnPtr = FnPtr { f: failing::DrawElementsInstancedBaseVertex as *const libc::c_void, is_loaded: false };
    pub static mut DrawElementsInstancedBaseVertexBaseInstance: FnPtr = FnPtr { f: failing::DrawElementsInstancedBaseVertexBaseInstance as *const libc::c_void, is_loaded: false };
    pub static mut DrawRangeElements: FnPtr = FnPtr { f: failing::DrawRangeElements as *const libc::c_void, is_loaded: false };
    pub static mut DrawRangeElementsBaseVertex: FnPtr = FnPtr { f: failing::DrawRangeElementsBaseVertex as *const libc::c_void, is_loaded: false };
    pub static mut DrawTransformFeedback: FnPtr = FnPtr { f: failing::DrawTransformFeedback as *const libc::c_void, is_loaded: false };
    pub static mut DrawTransformFeedbackInstanced: FnPtr = FnPtr { f: failing::DrawTransformFeedbackInstanced as *const libc::c_void, is_loaded: false };
    pub static mut DrawTransformFeedbackStream: FnPtr = FnPtr { f: failing::DrawTransformFeedbackStream as *const libc::c_void, is_loaded: false };
    pub static mut DrawTransformFeedbackStreamInstanced: FnPtr = FnPtr { f: failing::DrawTransformFeedbackStreamInstanced as *const libc::c_void, is_loaded: false };
    pub static mut Enable: FnPtr = FnPtr { f: failing::Enable as *const libc::c_void, is_loaded: false };
    pub static mut EnableVertexAttribArray: FnPtr = FnPtr { f: failing::EnableVertexAttribArray as *const libc::c_void, is_loaded: false };
    pub static mut Enablei: FnPtr = FnPtr { f: failing::Enablei as *const libc::c_void, is_loaded: false };
    pub static mut EndConditionalRender: FnPtr = FnPtr { f: failing::EndConditionalRender as *const libc::c_void, is_loaded: false };
    pub static mut EndQuery: FnPtr = FnPtr { f: failing::EndQuery as *const libc::c_void, is_loaded: false };
    pub static mut EndQueryIndexed: FnPtr = FnPtr { f: failing::EndQueryIndexed as *const libc::c_void, is_loaded: false };
    pub static mut EndTransformFeedback: FnPtr = FnPtr { f: failing::EndTransformFeedback as *const libc::c_void, is_loaded: false };
    pub static mut FenceSync: FnPtr = FnPtr { f: failing::FenceSync as *const libc::c_void, is_loaded: false };
    pub static mut Finish: FnPtr = FnPtr { f: failing::Finish as *const libc::c_void, is_loaded: false };
    pub static mut Flush: FnPtr = FnPtr { f: failing::Flush as *const libc::c_void, is_loaded: false };
    pub static mut FlushMappedBufferRange: FnPtr = FnPtr { f: failing::FlushMappedBufferRange as *const libc::c_void, is_loaded: false };
    pub static mut FramebufferParameteri: FnPtr = FnPtr { f: failing::FramebufferParameteri as *const libc::c_void, is_loaded: false };
    pub static mut FramebufferRenderbuffer: FnPtr = FnPtr { f: failing::FramebufferRenderbuffer as *const libc::c_void, is_loaded: false };
    pub static mut FramebufferTexture: FnPtr = FnPtr { f: failing::FramebufferTexture as *const libc::c_void, is_loaded: false };
    pub static mut FramebufferTexture1D: FnPtr = FnPtr { f: failing::FramebufferTexture1D as *const libc::c_void, is_loaded: false };
    pub static mut FramebufferTexture2D: FnPtr = FnPtr { f: failing::FramebufferTexture2D as *const libc::c_void, is_loaded: false };
    pub static mut FramebufferTexture3D: FnPtr = FnPtr { f: failing::FramebufferTexture3D as *const libc::c_void, is_loaded: false };
    pub static mut FramebufferTextureLayer: FnPtr = FnPtr { f: failing::FramebufferTextureLayer as *const libc::c_void, is_loaded: false };
    pub static mut FrontFace: FnPtr = FnPtr { f: failing::FrontFace as *const libc::c_void, is_loaded: false };
    pub static mut GenBuffers: FnPtr = FnPtr { f: failing::GenBuffers as *const libc::c_void, is_loaded: false };
    pub static mut GenFramebuffers: FnPtr = FnPtr { f: failing::GenFramebuffers as *const libc::c_void, is_loaded: false };
    pub static mut GenProgramPipelines: FnPtr = FnPtr { f: failing::GenProgramPipelines as *const libc::c_void, is_loaded: false };
    pub static mut GenQueries: FnPtr = FnPtr { f: failing::GenQueries as *const libc::c_void, is_loaded: false };
    pub static mut GenRenderbuffers: FnPtr = FnPtr { f: failing::GenRenderbuffers as *const libc::c_void, is_loaded: false };
    pub static mut GenSamplers: FnPtr = FnPtr { f: failing::GenSamplers as *const libc::c_void, is_loaded: false };
    pub static mut GenTextures: FnPtr = FnPtr { f: failing::GenTextures as *const libc::c_void, is_loaded: false };
    pub static mut GenTransformFeedbacks: FnPtr = FnPtr { f: failing::GenTransformFeedbacks as *const libc::c_void, is_loaded: false };
    pub static mut GenVertexArrays: FnPtr = FnPtr { f: failing::GenVertexArrays as *const libc::c_void, is_loaded: false };
    pub static mut GenerateMipmap: FnPtr = FnPtr { f: failing::GenerateMipmap as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveAtomicCounterBufferiv: FnPtr = FnPtr { f: failing::GetActiveAtomicCounterBufferiv as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveAttrib: FnPtr = FnPtr { f: failing::GetActiveAttrib as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveSubroutineName: FnPtr = FnPtr { f: failing::GetActiveSubroutineName as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveSubroutineUniformName: FnPtr = FnPtr { f: failing::GetActiveSubroutineUniformName as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveSubroutineUniformiv: FnPtr = FnPtr { f: failing::GetActiveSubroutineUniformiv as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveUniform: FnPtr = FnPtr { f: failing::GetActiveUniform as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveUniformBlockName: FnPtr = FnPtr { f: failing::GetActiveUniformBlockName as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveUniformBlockiv: FnPtr = FnPtr { f: failing::GetActiveUniformBlockiv as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveUniformName: FnPtr = FnPtr { f: failing::GetActiveUniformName as *const libc::c_void, is_loaded: false };
    pub static mut GetActiveUniformsiv: FnPtr = FnPtr { f: failing::GetActiveUniformsiv as *const libc::c_void, is_loaded: false };
    pub static mut GetAttachedShaders: FnPtr = FnPtr { f: failing::GetAttachedShaders as *const libc::c_void, is_loaded: false };
    pub static mut GetAttribLocation: FnPtr = FnPtr { f: failing::GetAttribLocation as *const libc::c_void, is_loaded: false };
    pub static mut GetBooleani_v: FnPtr = FnPtr { f: failing::GetBooleani_v as *const libc::c_void, is_loaded: false };
    pub static mut GetBooleanv: FnPtr = FnPtr { f: failing::GetBooleanv as *const libc::c_void, is_loaded: false };
    pub static mut GetBufferParameteri64v: FnPtr = FnPtr { f: failing::GetBufferParameteri64v as *const libc::c_void, is_loaded: false };
    pub static mut GetBufferParameteriv: FnPtr = FnPtr { f: failing::GetBufferParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut GetBufferPointerv: FnPtr = FnPtr { f: failing::GetBufferPointerv as *const libc::c_void, is_loaded: false };
    pub static mut GetBufferSubData: FnPtr = FnPtr { f: failing::GetBufferSubData as *const libc::c_void, is_loaded: false };
    pub static mut GetCompressedTexImage: FnPtr = FnPtr { f: failing::GetCompressedTexImage as *const libc::c_void, is_loaded: false };
    pub static mut GetDebugMessageLog: FnPtr = FnPtr { f: failing::GetDebugMessageLog as *const libc::c_void, is_loaded: false };
    pub static mut GetDoublei_v: FnPtr = FnPtr { f: failing::GetDoublei_v as *const libc::c_void, is_loaded: false };
    pub static mut GetDoublev: FnPtr = FnPtr { f: failing::GetDoublev as *const libc::c_void, is_loaded: false };
    pub static mut GetError: FnPtr = FnPtr { f: failing::GetError as *const libc::c_void, is_loaded: false };
    pub static mut GetFloati_v: FnPtr = FnPtr { f: failing::GetFloati_v as *const libc::c_void, is_loaded: false };
    pub static mut GetFloatv: FnPtr = FnPtr { f: failing::GetFloatv as *const libc::c_void, is_loaded: false };
    pub static mut GetFragDataIndex: FnPtr = FnPtr { f: failing::GetFragDataIndex as *const libc::c_void, is_loaded: false };
    pub static mut GetFragDataLocation: FnPtr = FnPtr { f: failing::GetFragDataLocation as *const libc::c_void, is_loaded: false };
    pub static mut GetFramebufferAttachmentParameteriv: FnPtr = FnPtr { f: failing::GetFramebufferAttachmentParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut GetFramebufferParameteriv: FnPtr = FnPtr { f: failing::GetFramebufferParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut GetInteger64i_v: FnPtr = FnPtr { f: failing::GetInteger64i_v as *const libc::c_void, is_loaded: false };
    pub static mut GetInteger64v: FnPtr = FnPtr { f: failing::GetInteger64v as *const libc::c_void, is_loaded: false };
    pub static mut GetIntegeri_v: FnPtr = FnPtr { f: failing::GetIntegeri_v as *const libc::c_void, is_loaded: false };
    pub static mut GetIntegerv: FnPtr = FnPtr { f: failing::GetIntegerv as *const libc::c_void, is_loaded: false };
    pub static mut GetInternalformati64v: FnPtr = FnPtr { f: failing::GetInternalformati64v as *const libc::c_void, is_loaded: false };
    pub static mut GetInternalformativ: FnPtr = FnPtr { f: failing::GetInternalformativ as *const libc::c_void, is_loaded: false };
    pub static mut GetMultisamplefv: FnPtr = FnPtr { f: failing::GetMultisamplefv as *const libc::c_void, is_loaded: false };
    pub static mut GetObjectLabel: FnPtr = FnPtr { f: failing::GetObjectLabel as *const libc::c_void, is_loaded: false };
    pub static mut GetObjectPtrLabel: FnPtr = FnPtr { f: failing::GetObjectPtrLabel as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramBinary: FnPtr = FnPtr { f: failing::GetProgramBinary as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramInfoLog: FnPtr = FnPtr { f: failing::GetProgramInfoLog as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramInterfaceiv: FnPtr = FnPtr { f: failing::GetProgramInterfaceiv as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramPipelineInfoLog: FnPtr = FnPtr { f: failing::GetProgramPipelineInfoLog as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramPipelineiv: FnPtr = FnPtr { f: failing::GetProgramPipelineiv as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramResourceIndex: FnPtr = FnPtr { f: failing::GetProgramResourceIndex as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramResourceLocation: FnPtr = FnPtr { f: failing::GetProgramResourceLocation as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramResourceLocationIndex: FnPtr = FnPtr { f: failing::GetProgramResourceLocationIndex as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramResourceName: FnPtr = FnPtr { f: failing::GetProgramResourceName as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramResourceiv: FnPtr = FnPtr { f: failing::GetProgramResourceiv as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramStageiv: FnPtr = FnPtr { f: failing::GetProgramStageiv as *const libc::c_void, is_loaded: false };
    pub static mut GetProgramiv: FnPtr = FnPtr { f: failing::GetProgramiv as *const libc::c_void, is_loaded: false };
    pub static mut GetQueryIndexediv: FnPtr = FnPtr { f: failing::GetQueryIndexediv as *const libc::c_void, is_loaded: false };
    pub static mut GetQueryObjecti64v: FnPtr = FnPtr { f: failing::GetQueryObjecti64v as *const libc::c_void, is_loaded: false };
    pub static mut GetQueryObjectiv: FnPtr = FnPtr { f: failing::GetQueryObjectiv as *const libc::c_void, is_loaded: false };
    pub static mut GetQueryObjectui64v: FnPtr = FnPtr { f: failing::GetQueryObjectui64v as *const libc::c_void, is_loaded: false };
    pub static mut GetQueryObjectuiv: FnPtr = FnPtr { f: failing::GetQueryObjectuiv as *const libc::c_void, is_loaded: false };
    pub static mut GetQueryiv: FnPtr = FnPtr { f: failing::GetQueryiv as *const libc::c_void, is_loaded: false };
    pub static mut GetRenderbufferParameteriv: FnPtr = FnPtr { f: failing::GetRenderbufferParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut GetSamplerParameterIiv: FnPtr = FnPtr { f: failing::GetSamplerParameterIiv as *const libc::c_void, is_loaded: false };
    pub static mut GetSamplerParameterIuiv: FnPtr = FnPtr { f: failing::GetSamplerParameterIuiv as *const libc::c_void, is_loaded: false };
    pub static mut GetSamplerParameterfv: FnPtr = FnPtr { f: failing::GetSamplerParameterfv as *const libc::c_void, is_loaded: false };
    pub static mut GetSamplerParameteriv: FnPtr = FnPtr { f: failing::GetSamplerParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut GetShaderInfoLog: FnPtr = FnPtr { f: failing::GetShaderInfoLog as *const libc::c_void, is_loaded: false };
    pub static mut GetShaderPrecisionFormat: FnPtr = FnPtr { f: failing::GetShaderPrecisionFormat as *const libc::c_void, is_loaded: false };
    pub static mut GetShaderSource: FnPtr = FnPtr { f: failing::GetShaderSource as *const libc::c_void, is_loaded: false };
    pub static mut GetShaderiv: FnPtr = FnPtr { f: failing::GetShaderiv as *const libc::c_void, is_loaded: false };
    pub static mut GetString: FnPtr = FnPtr { f: failing::GetString as *const libc::c_void, is_loaded: false };
    pub static mut GetStringi: FnPtr = FnPtr { f: failing::GetStringi as *const libc::c_void, is_loaded: false };
    pub static mut GetSubroutineIndex: FnPtr = FnPtr { f: failing::GetSubroutineIndex as *const libc::c_void, is_loaded: false };
    pub static mut GetSubroutineUniformLocation: FnPtr = FnPtr { f: failing::GetSubroutineUniformLocation as *const libc::c_void, is_loaded: false };
    pub static mut GetSynciv: FnPtr = FnPtr { f: failing::GetSynciv as *const libc::c_void, is_loaded: false };
    pub static mut GetTexImage: FnPtr = FnPtr { f: failing::GetTexImage as *const libc::c_void, is_loaded: false };
    pub static mut GetTexLevelParameterfv: FnPtr = FnPtr { f: failing::GetTexLevelParameterfv as *const libc::c_void, is_loaded: false };
    pub static mut GetTexLevelParameteriv: FnPtr = FnPtr { f: failing::GetTexLevelParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut GetTexParameterIiv: FnPtr = FnPtr { f: failing::GetTexParameterIiv as *const libc::c_void, is_loaded: false };
    pub static mut GetTexParameterIuiv: FnPtr = FnPtr { f: failing::GetTexParameterIuiv as *const libc::c_void, is_loaded: false };
    pub static mut GetTexParameterfv: FnPtr = FnPtr { f: failing::GetTexParameterfv as *const libc::c_void, is_loaded: false };
    pub static mut GetTexParameteriv: FnPtr = FnPtr { f: failing::GetTexParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut GetTransformFeedbackVarying: FnPtr = FnPtr { f: failing::GetTransformFeedbackVarying as *const libc::c_void, is_loaded: false };
    pub static mut GetUniformBlockIndex: FnPtr = FnPtr { f: failing::GetUniformBlockIndex as *const libc::c_void, is_loaded: false };
    pub static mut GetUniformIndices: FnPtr = FnPtr { f: failing::GetUniformIndices as *const libc::c_void, is_loaded: false };
    pub static mut GetUniformLocation: FnPtr = FnPtr { f: failing::GetUniformLocation as *const libc::c_void, is_loaded: false };
    pub static mut GetUniformSubroutineuiv: FnPtr = FnPtr { f: failing::GetUniformSubroutineuiv as *const libc::c_void, is_loaded: false };
    pub static mut GetUniformdv: FnPtr = FnPtr { f: failing::GetUniformdv as *const libc::c_void, is_loaded: false };
    pub static mut GetUniformfv: FnPtr = FnPtr { f: failing::GetUniformfv as *const libc::c_void, is_loaded: false };
    pub static mut GetUniformiv: FnPtr = FnPtr { f: failing::GetUniformiv as *const libc::c_void, is_loaded: false };
    pub static mut GetUniformuiv: FnPtr = FnPtr { f: failing::GetUniformuiv as *const libc::c_void, is_loaded: false };
    pub static mut GetVertexAttribIiv: FnPtr = FnPtr { f: failing::GetVertexAttribIiv as *const libc::c_void, is_loaded: false };
    pub static mut GetVertexAttribIuiv: FnPtr = FnPtr { f: failing::GetVertexAttribIuiv as *const libc::c_void, is_loaded: false };
    pub static mut GetVertexAttribLdv: FnPtr = FnPtr { f: failing::GetVertexAttribLdv as *const libc::c_void, is_loaded: false };
    pub static mut GetVertexAttribPointerv: FnPtr = FnPtr { f: failing::GetVertexAttribPointerv as *const libc::c_void, is_loaded: false };
    pub static mut GetVertexAttribdv: FnPtr = FnPtr { f: failing::GetVertexAttribdv as *const libc::c_void, is_loaded: false };
    pub static mut GetVertexAttribfv: FnPtr = FnPtr { f: failing::GetVertexAttribfv as *const libc::c_void, is_loaded: false };
    pub static mut GetVertexAttribiv: FnPtr = FnPtr { f: failing::GetVertexAttribiv as *const libc::c_void, is_loaded: false };
    pub static mut Hint: FnPtr = FnPtr { f: failing::Hint as *const libc::c_void, is_loaded: false };
    pub static mut InvalidateBufferData: FnPtr = FnPtr { f: failing::InvalidateBufferData as *const libc::c_void, is_loaded: false };
    pub static mut InvalidateBufferSubData: FnPtr = FnPtr { f: failing::InvalidateBufferSubData as *const libc::c_void, is_loaded: false };
    pub static mut InvalidateFramebuffer: FnPtr = FnPtr { f: failing::InvalidateFramebuffer as *const libc::c_void, is_loaded: false };
    pub static mut InvalidateSubFramebuffer: FnPtr = FnPtr { f: failing::InvalidateSubFramebuffer as *const libc::c_void, is_loaded: false };
    pub static mut InvalidateTexImage: FnPtr = FnPtr { f: failing::InvalidateTexImage as *const libc::c_void, is_loaded: false };
    pub static mut InvalidateTexSubImage: FnPtr = FnPtr { f: failing::InvalidateTexSubImage as *const libc::c_void, is_loaded: false };
    pub static mut IsBuffer: FnPtr = FnPtr { f: failing::IsBuffer as *const libc::c_void, is_loaded: false };
    pub static mut IsEnabled: FnPtr = FnPtr { f: failing::IsEnabled as *const libc::c_void, is_loaded: false };
    pub static mut IsEnabledi: FnPtr = FnPtr { f: failing::IsEnabledi as *const libc::c_void, is_loaded: false };
    pub static mut IsFramebuffer: FnPtr = FnPtr { f: failing::IsFramebuffer as *const libc::c_void, is_loaded: false };
    pub static mut IsProgram: FnPtr = FnPtr { f: failing::IsProgram as *const libc::c_void, is_loaded: false };
    pub static mut IsProgramPipeline: FnPtr = FnPtr { f: failing::IsProgramPipeline as *const libc::c_void, is_loaded: false };
    pub static mut IsQuery: FnPtr = FnPtr { f: failing::IsQuery as *const libc::c_void, is_loaded: false };
    pub static mut IsRenderbuffer: FnPtr = FnPtr { f: failing::IsRenderbuffer as *const libc::c_void, is_loaded: false };
    pub static mut IsSampler: FnPtr = FnPtr { f: failing::IsSampler as *const libc::c_void, is_loaded: false };
    pub static mut IsShader: FnPtr = FnPtr { f: failing::IsShader as *const libc::c_void, is_loaded: false };
    pub static mut IsSync: FnPtr = FnPtr { f: failing::IsSync as *const libc::c_void, is_loaded: false };
    pub static mut IsTexture: FnPtr = FnPtr { f: failing::IsTexture as *const libc::c_void, is_loaded: false };
    pub static mut IsTransformFeedback: FnPtr = FnPtr { f: failing::IsTransformFeedback as *const libc::c_void, is_loaded: false };
    pub static mut IsVertexArray: FnPtr = FnPtr { f: failing::IsVertexArray as *const libc::c_void, is_loaded: false };
    pub static mut LineWidth: FnPtr = FnPtr { f: failing::LineWidth as *const libc::c_void, is_loaded: false };
    pub static mut LinkProgram: FnPtr = FnPtr { f: failing::LinkProgram as *const libc::c_void, is_loaded: false };
    pub static mut LogicOp: FnPtr = FnPtr { f: failing::LogicOp as *const libc::c_void, is_loaded: false };
    pub static mut MapBuffer: FnPtr = FnPtr { f: failing::MapBuffer as *const libc::c_void, is_loaded: false };
    pub static mut MapBufferRange: FnPtr = FnPtr { f: failing::MapBufferRange as *const libc::c_void, is_loaded: false };
    pub static mut MemoryBarrier: FnPtr = FnPtr { f: failing::MemoryBarrier as *const libc::c_void, is_loaded: false };
    pub static mut MinSampleShading: FnPtr = FnPtr { f: failing::MinSampleShading as *const libc::c_void, is_loaded: false };
    pub static mut MultiDrawArrays: FnPtr = FnPtr { f: failing::MultiDrawArrays as *const libc::c_void, is_loaded: false };
    pub static mut MultiDrawArraysIndirect: FnPtr = FnPtr { f: failing::MultiDrawArraysIndirect as *const libc::c_void, is_loaded: false };
    pub static mut MultiDrawElements: FnPtr = FnPtr { f: failing::MultiDrawElements as *const libc::c_void, is_loaded: false };
    pub static mut MultiDrawElementsBaseVertex: FnPtr = FnPtr { f: failing::MultiDrawElementsBaseVertex as *const libc::c_void, is_loaded: false };
    pub static mut MultiDrawElementsIndirect: FnPtr = FnPtr { f: failing::MultiDrawElementsIndirect as *const libc::c_void, is_loaded: false };
    pub static mut MultiTexCoordP1ui: FnPtr = FnPtr { f: failing::MultiTexCoordP1ui as *const libc::c_void, is_loaded: false };
    pub static mut MultiTexCoordP1uiv: FnPtr = FnPtr { f: failing::MultiTexCoordP1uiv as *const libc::c_void, is_loaded: false };
    pub static mut MultiTexCoordP2ui: FnPtr = FnPtr { f: failing::MultiTexCoordP2ui as *const libc::c_void, is_loaded: false };
    pub static mut MultiTexCoordP2uiv: FnPtr = FnPtr { f: failing::MultiTexCoordP2uiv as *const libc::c_void, is_loaded: false };
    pub static mut MultiTexCoordP3ui: FnPtr = FnPtr { f: failing::MultiTexCoordP3ui as *const libc::c_void, is_loaded: false };
    pub static mut MultiTexCoordP3uiv: FnPtr = FnPtr { f: failing::MultiTexCoordP3uiv as *const libc::c_void, is_loaded: false };
    pub static mut MultiTexCoordP4ui: FnPtr = FnPtr { f: failing::MultiTexCoordP4ui as *const libc::c_void, is_loaded: false };
    pub static mut MultiTexCoordP4uiv: FnPtr = FnPtr { f: failing::MultiTexCoordP4uiv as *const libc::c_void, is_loaded: false };
    pub static mut NormalP3ui: FnPtr = FnPtr { f: failing::NormalP3ui as *const libc::c_void, is_loaded: false };
    pub static mut NormalP3uiv: FnPtr = FnPtr { f: failing::NormalP3uiv as *const libc::c_void, is_loaded: false };
    pub static mut ObjectLabel: FnPtr = FnPtr { f: failing::ObjectLabel as *const libc::c_void, is_loaded: false };
    pub static mut ObjectPtrLabel: FnPtr = FnPtr { f: failing::ObjectPtrLabel as *const libc::c_void, is_loaded: false };
    pub static mut PatchParameterfv: FnPtr = FnPtr { f: failing::PatchParameterfv as *const libc::c_void, is_loaded: false };
    pub static mut PatchParameteri: FnPtr = FnPtr { f: failing::PatchParameteri as *const libc::c_void, is_loaded: false };
    pub static mut PauseTransformFeedback: FnPtr = FnPtr { f: failing::PauseTransformFeedback as *const libc::c_void, is_loaded: false };
    pub static mut PixelStoref: FnPtr = FnPtr { f: failing::PixelStoref as *const libc::c_void, is_loaded: false };
    pub static mut PixelStorei: FnPtr = FnPtr { f: failing::PixelStorei as *const libc::c_void, is_loaded: false };
    pub static mut PointParameterf: FnPtr = FnPtr { f: failing::PointParameterf as *const libc::c_void, is_loaded: false };
    pub static mut PointParameterfv: FnPtr = FnPtr { f: failing::PointParameterfv as *const libc::c_void, is_loaded: false };
    pub static mut PointParameteri: FnPtr = FnPtr { f: failing::PointParameteri as *const libc::c_void, is_loaded: false };
    pub static mut PointParameteriv: FnPtr = FnPtr { f: failing::PointParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut PointSize: FnPtr = FnPtr { f: failing::PointSize as *const libc::c_void, is_loaded: false };
    pub static mut PolygonMode: FnPtr = FnPtr { f: failing::PolygonMode as *const libc::c_void, is_loaded: false };
    pub static mut PolygonOffset: FnPtr = FnPtr { f: failing::PolygonOffset as *const libc::c_void, is_loaded: false };
    pub static mut PopDebugGroup: FnPtr = FnPtr { f: failing::PopDebugGroup as *const libc::c_void, is_loaded: false };
    pub static mut PrimitiveRestartIndex: FnPtr = FnPtr { f: failing::PrimitiveRestartIndex as *const libc::c_void, is_loaded: false };
    pub static mut ProgramBinary: FnPtr = FnPtr { f: failing::ProgramBinary as *const libc::c_void, is_loaded: false };
    pub static mut ProgramParameteri: FnPtr = FnPtr { f: failing::ProgramParameteri as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform1d: FnPtr = FnPtr { f: failing::ProgramUniform1d as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform1dv: FnPtr = FnPtr { f: failing::ProgramUniform1dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform1f: FnPtr = FnPtr { f: failing::ProgramUniform1f as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform1fv: FnPtr = FnPtr { f: failing::ProgramUniform1fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform1i: FnPtr = FnPtr { f: failing::ProgramUniform1i as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform1iv: FnPtr = FnPtr { f: failing::ProgramUniform1iv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform1ui: FnPtr = FnPtr { f: failing::ProgramUniform1ui as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform1uiv: FnPtr = FnPtr { f: failing::ProgramUniform1uiv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform2d: FnPtr = FnPtr { f: failing::ProgramUniform2d as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform2dv: FnPtr = FnPtr { f: failing::ProgramUniform2dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform2f: FnPtr = FnPtr { f: failing::ProgramUniform2f as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform2fv: FnPtr = FnPtr { f: failing::ProgramUniform2fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform2i: FnPtr = FnPtr { f: failing::ProgramUniform2i as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform2iv: FnPtr = FnPtr { f: failing::ProgramUniform2iv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform2ui: FnPtr = FnPtr { f: failing::ProgramUniform2ui as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform2uiv: FnPtr = FnPtr { f: failing::ProgramUniform2uiv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform3d: FnPtr = FnPtr { f: failing::ProgramUniform3d as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform3dv: FnPtr = FnPtr { f: failing::ProgramUniform3dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform3f: FnPtr = FnPtr { f: failing::ProgramUniform3f as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform3fv: FnPtr = FnPtr { f: failing::ProgramUniform3fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform3i: FnPtr = FnPtr { f: failing::ProgramUniform3i as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform3iv: FnPtr = FnPtr { f: failing::ProgramUniform3iv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform3ui: FnPtr = FnPtr { f: failing::ProgramUniform3ui as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform3uiv: FnPtr = FnPtr { f: failing::ProgramUniform3uiv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform4d: FnPtr = FnPtr { f: failing::ProgramUniform4d as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform4dv: FnPtr = FnPtr { f: failing::ProgramUniform4dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform4f: FnPtr = FnPtr { f: failing::ProgramUniform4f as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform4fv: FnPtr = FnPtr { f: failing::ProgramUniform4fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform4i: FnPtr = FnPtr { f: failing::ProgramUniform4i as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform4iv: FnPtr = FnPtr { f: failing::ProgramUniform4iv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform4ui: FnPtr = FnPtr { f: failing::ProgramUniform4ui as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniform4uiv: FnPtr = FnPtr { f: failing::ProgramUniform4uiv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix2dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix2dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix2fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix2fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix2x3dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix2x3dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix2x3fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix2x3fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix2x4dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix2x4dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix2x4fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix2x4fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix3dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix3dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix3fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix3fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix3x2dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix3x2dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix3x2fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix3x2fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix3x4dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix3x4dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix3x4fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix3x4fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix4dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix4dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix4fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix4fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix4x2dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix4x2dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix4x2fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix4x2fv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix4x3dv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix4x3dv as *const libc::c_void, is_loaded: false };
    pub static mut ProgramUniformMatrix4x3fv: FnPtr = FnPtr { f: failing::ProgramUniformMatrix4x3fv as *const libc::c_void, is_loaded: false };
    pub static mut ProvokingVertex: FnPtr = FnPtr { f: failing::ProvokingVertex as *const libc::c_void, is_loaded: false };
    pub static mut PushDebugGroup: FnPtr = FnPtr { f: failing::PushDebugGroup as *const libc::c_void, is_loaded: false };
    pub static mut QueryCounter: FnPtr = FnPtr { f: failing::QueryCounter as *const libc::c_void, is_loaded: false };
    pub static mut ReadBuffer: FnPtr = FnPtr { f: failing::ReadBuffer as *const libc::c_void, is_loaded: false };
    pub static mut ReadPixels: FnPtr = FnPtr { f: failing::ReadPixels as *const libc::c_void, is_loaded: false };
    pub static mut ReleaseShaderCompiler: FnPtr = FnPtr { f: failing::ReleaseShaderCompiler as *const libc::c_void, is_loaded: false };
    pub static mut RenderbufferStorage: FnPtr = FnPtr { f: failing::RenderbufferStorage as *const libc::c_void, is_loaded: false };
    pub static mut RenderbufferStorageMultisample: FnPtr = FnPtr { f: failing::RenderbufferStorageMultisample as *const libc::c_void, is_loaded: false };
    pub static mut ResumeTransformFeedback: FnPtr = FnPtr { f: failing::ResumeTransformFeedback as *const libc::c_void, is_loaded: false };
    pub static mut SampleCoverage: FnPtr = FnPtr { f: failing::SampleCoverage as *const libc::c_void, is_loaded: false };
    pub static mut SampleMaski: FnPtr = FnPtr { f: failing::SampleMaski as *const libc::c_void, is_loaded: false };
    pub static mut SamplerParameterIiv: FnPtr = FnPtr { f: failing::SamplerParameterIiv as *const libc::c_void, is_loaded: false };
    pub static mut SamplerParameterIuiv: FnPtr = FnPtr { f: failing::SamplerParameterIuiv as *const libc::c_void, is_loaded: false };
    pub static mut SamplerParameterf: FnPtr = FnPtr { f: failing::SamplerParameterf as *const libc::c_void, is_loaded: false };
    pub static mut SamplerParameterfv: FnPtr = FnPtr { f: failing::SamplerParameterfv as *const libc::c_void, is_loaded: false };
    pub static mut SamplerParameteri: FnPtr = FnPtr { f: failing::SamplerParameteri as *const libc::c_void, is_loaded: false };
    pub static mut SamplerParameteriv: FnPtr = FnPtr { f: failing::SamplerParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut Scissor: FnPtr = FnPtr { f: failing::Scissor as *const libc::c_void, is_loaded: false };
    pub static mut ScissorArrayv: FnPtr = FnPtr { f: failing::ScissorArrayv as *const libc::c_void, is_loaded: false };
    pub static mut ScissorIndexed: FnPtr = FnPtr { f: failing::ScissorIndexed as *const libc::c_void, is_loaded: false };
    pub static mut ScissorIndexedv: FnPtr = FnPtr { f: failing::ScissorIndexedv as *const libc::c_void, is_loaded: false };
    pub static mut SecondaryColorP3ui: FnPtr = FnPtr { f: failing::SecondaryColorP3ui as *const libc::c_void, is_loaded: false };
    pub static mut SecondaryColorP3uiv: FnPtr = FnPtr { f: failing::SecondaryColorP3uiv as *const libc::c_void, is_loaded: false };
    pub static mut ShaderBinary: FnPtr = FnPtr { f: failing::ShaderBinary as *const libc::c_void, is_loaded: false };
    pub static mut ShaderSource: FnPtr = FnPtr { f: failing::ShaderSource as *const libc::c_void, is_loaded: false };
    pub static mut ShaderStorageBlockBinding: FnPtr = FnPtr { f: failing::ShaderStorageBlockBinding as *const libc::c_void, is_loaded: false };
    pub static mut StencilFunc: FnPtr = FnPtr { f: failing::StencilFunc as *const libc::c_void, is_loaded: false };
    pub static mut StencilFuncSeparate: FnPtr = FnPtr { f: failing::StencilFuncSeparate as *const libc::c_void, is_loaded: false };
    pub static mut StencilMask: FnPtr = FnPtr { f: failing::StencilMask as *const libc::c_void, is_loaded: false };
    pub static mut StencilMaskSeparate: FnPtr = FnPtr { f: failing::StencilMaskSeparate as *const libc::c_void, is_loaded: false };
    pub static mut StencilOp: FnPtr = FnPtr { f: failing::StencilOp as *const libc::c_void, is_loaded: false };
    pub static mut StencilOpSeparate: FnPtr = FnPtr { f: failing::StencilOpSeparate as *const libc::c_void, is_loaded: false };
    pub static mut TexBuffer: FnPtr = FnPtr { f: failing::TexBuffer as *const libc::c_void, is_loaded: false };
    pub static mut TexBufferRange: FnPtr = FnPtr { f: failing::TexBufferRange as *const libc::c_void, is_loaded: false };
    pub static mut TexCoordP1ui: FnPtr = FnPtr { f: failing::TexCoordP1ui as *const libc::c_void, is_loaded: false };
    pub static mut TexCoordP1uiv: FnPtr = FnPtr { f: failing::TexCoordP1uiv as *const libc::c_void, is_loaded: false };
    pub static mut TexCoordP2ui: FnPtr = FnPtr { f: failing::TexCoordP2ui as *const libc::c_void, is_loaded: false };
    pub static mut TexCoordP2uiv: FnPtr = FnPtr { f: failing::TexCoordP2uiv as *const libc::c_void, is_loaded: false };
    pub static mut TexCoordP3ui: FnPtr = FnPtr { f: failing::TexCoordP3ui as *const libc::c_void, is_loaded: false };
    pub static mut TexCoordP3uiv: FnPtr = FnPtr { f: failing::TexCoordP3uiv as *const libc::c_void, is_loaded: false };
    pub static mut TexCoordP4ui: FnPtr = FnPtr { f: failing::TexCoordP4ui as *const libc::c_void, is_loaded: false };
    pub static mut TexCoordP4uiv: FnPtr = FnPtr { f: failing::TexCoordP4uiv as *const libc::c_void, is_loaded: false };
    pub static mut TexImage1D: FnPtr = FnPtr { f: failing::TexImage1D as *const libc::c_void, is_loaded: false };
    pub static mut TexImage2D: FnPtr = FnPtr { f: failing::TexImage2D as *const libc::c_void, is_loaded: false };
    pub static mut TexImage2DMultisample: FnPtr = FnPtr { f: failing::TexImage2DMultisample as *const libc::c_void, is_loaded: false };
    pub static mut TexImage3D: FnPtr = FnPtr { f: failing::TexImage3D as *const libc::c_void, is_loaded: false };
    pub static mut TexImage3DMultisample: FnPtr = FnPtr { f: failing::TexImage3DMultisample as *const libc::c_void, is_loaded: false };
    pub static mut TexParameterIiv: FnPtr = FnPtr { f: failing::TexParameterIiv as *const libc::c_void, is_loaded: false };
    pub static mut TexParameterIuiv: FnPtr = FnPtr { f: failing::TexParameterIuiv as *const libc::c_void, is_loaded: false };
    pub static mut TexParameterf: FnPtr = FnPtr { f: failing::TexParameterf as *const libc::c_void, is_loaded: false };
    pub static mut TexParameterfv: FnPtr = FnPtr { f: failing::TexParameterfv as *const libc::c_void, is_loaded: false };
    pub static mut TexParameteri: FnPtr = FnPtr { f: failing::TexParameteri as *const libc::c_void, is_loaded: false };
    pub static mut TexParameteriv: FnPtr = FnPtr { f: failing::TexParameteriv as *const libc::c_void, is_loaded: false };
    pub static mut TexStorage1D: FnPtr = FnPtr { f: failing::TexStorage1D as *const libc::c_void, is_loaded: false };
    pub static mut TexStorage2D: FnPtr = FnPtr { f: failing::TexStorage2D as *const libc::c_void, is_loaded: false };
    pub static mut TexStorage2DMultisample: FnPtr = FnPtr { f: failing::TexStorage2DMultisample as *const libc::c_void, is_loaded: false };
    pub static mut TexStorage3D: FnPtr = FnPtr { f: failing::TexStorage3D as *const libc::c_void, is_loaded: false };
    pub static mut TexStorage3DMultisample: FnPtr = FnPtr { f: failing::TexStorage3DMultisample as *const libc::c_void, is_loaded: false };
    pub static mut TexSubImage1D: FnPtr = FnPtr { f: failing::TexSubImage1D as *const libc::c_void, is_loaded: false };
    pub static mut TexSubImage2D: FnPtr = FnPtr { f: failing::TexSubImage2D as *const libc::c_void, is_loaded: false };
    pub static mut TexSubImage3D: FnPtr = FnPtr { f: failing::TexSubImage3D as *const libc::c_void, is_loaded: false };
    pub static mut TextureView: FnPtr = FnPtr { f: failing::TextureView as *const libc::c_void, is_loaded: false };
    pub static mut TransformFeedbackVaryings: FnPtr = FnPtr { f: failing::TransformFeedbackVaryings as *const libc::c_void, is_loaded: false };
    pub static mut Uniform1d: FnPtr = FnPtr { f: failing::Uniform1d as *const libc::c_void, is_loaded: false };
    pub static mut Uniform1dv: FnPtr = FnPtr { f: failing::Uniform1dv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform1f: FnPtr = FnPtr { f: failing::Uniform1f as *const libc::c_void, is_loaded: false };
    pub static mut Uniform1fv: FnPtr = FnPtr { f: failing::Uniform1fv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform1i: FnPtr = FnPtr { f: failing::Uniform1i as *const libc::c_void, is_loaded: false };
    pub static mut Uniform1iv: FnPtr = FnPtr { f: failing::Uniform1iv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform1ui: FnPtr = FnPtr { f: failing::Uniform1ui as *const libc::c_void, is_loaded: false };
    pub static mut Uniform1uiv: FnPtr = FnPtr { f: failing::Uniform1uiv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform2d: FnPtr = FnPtr { f: failing::Uniform2d as *const libc::c_void, is_loaded: false };
    pub static mut Uniform2dv: FnPtr = FnPtr { f: failing::Uniform2dv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform2f: FnPtr = FnPtr { f: failing::Uniform2f as *const libc::c_void, is_loaded: false };
    pub static mut Uniform2fv: FnPtr = FnPtr { f: failing::Uniform2fv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform2i: FnPtr = FnPtr { f: failing::Uniform2i as *const libc::c_void, is_loaded: false };
    pub static mut Uniform2iv: FnPtr = FnPtr { f: failing::Uniform2iv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform2ui: FnPtr = FnPtr { f: failing::Uniform2ui as *const libc::c_void, is_loaded: false };
    pub static mut Uniform2uiv: FnPtr = FnPtr { f: failing::Uniform2uiv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform3d: FnPtr = FnPtr { f: failing::Uniform3d as *const libc::c_void, is_loaded: false };
    pub static mut Uniform3dv: FnPtr = FnPtr { f: failing::Uniform3dv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform3f: FnPtr = FnPtr { f: failing::Uniform3f as *const libc::c_void, is_loaded: false };
    pub static mut Uniform3fv: FnPtr = FnPtr { f: failing::Uniform3fv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform3i: FnPtr = FnPtr { f: failing::Uniform3i as *const libc::c_void, is_loaded: false };
    pub static mut Uniform3iv: FnPtr = FnPtr { f: failing::Uniform3iv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform3ui: FnPtr = FnPtr { f: failing::Uniform3ui as *const libc::c_void, is_loaded: false };
    pub static mut Uniform3uiv: FnPtr = FnPtr { f: failing::Uniform3uiv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform4d: FnPtr = FnPtr { f: failing::Uniform4d as *const libc::c_void, is_loaded: false };
    pub static mut Uniform4dv: FnPtr = FnPtr { f: failing::Uniform4dv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform4f: FnPtr = FnPtr { f: failing::Uniform4f as *const libc::c_void, is_loaded: false };
    pub static mut Uniform4fv: FnPtr = FnPtr { f: failing::Uniform4fv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform4i: FnPtr = FnPtr { f: failing::Uniform4i as *const libc::c_void, is_loaded: false };
    pub static mut Uniform4iv: FnPtr = FnPtr { f: failing::Uniform4iv as *const libc::c_void, is_loaded: false };
    pub static mut Uniform4ui: FnPtr = FnPtr { f: failing::Uniform4ui as *const libc::c_void, is_loaded: false };
    pub static mut Uniform4uiv: FnPtr = FnPtr { f: failing::Uniform4uiv as *const libc::c_void, is_loaded: false };
    pub static mut UniformBlockBinding: FnPtr = FnPtr { f: failing::UniformBlockBinding as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix2dv: FnPtr = FnPtr { f: failing::UniformMatrix2dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix2fv: FnPtr = FnPtr { f: failing::UniformMatrix2fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix2x3dv: FnPtr = FnPtr { f: failing::UniformMatrix2x3dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix2x3fv: FnPtr = FnPtr { f: failing::UniformMatrix2x3fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix2x4dv: FnPtr = FnPtr { f: failing::UniformMatrix2x4dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix2x4fv: FnPtr = FnPtr { f: failing::UniformMatrix2x4fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix3dv: FnPtr = FnPtr { f: failing::UniformMatrix3dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix3fv: FnPtr = FnPtr { f: failing::UniformMatrix3fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix3x2dv: FnPtr = FnPtr { f: failing::UniformMatrix3x2dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix3x2fv: FnPtr = FnPtr { f: failing::UniformMatrix3x2fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix3x4dv: FnPtr = FnPtr { f: failing::UniformMatrix3x4dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix3x4fv: FnPtr = FnPtr { f: failing::UniformMatrix3x4fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix4dv: FnPtr = FnPtr { f: failing::UniformMatrix4dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix4fv: FnPtr = FnPtr { f: failing::UniformMatrix4fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix4x2dv: FnPtr = FnPtr { f: failing::UniformMatrix4x2dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix4x2fv: FnPtr = FnPtr { f: failing::UniformMatrix4x2fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix4x3dv: FnPtr = FnPtr { f: failing::UniformMatrix4x3dv as *const libc::c_void, is_loaded: false };
    pub static mut UniformMatrix4x3fv: FnPtr = FnPtr { f: failing::UniformMatrix4x3fv as *const libc::c_void, is_loaded: false };
    pub static mut UniformSubroutinesuiv: FnPtr = FnPtr { f: failing::UniformSubroutinesuiv as *const libc::c_void, is_loaded: false };
    pub static mut UnmapBuffer: FnPtr = FnPtr { f: failing::UnmapBuffer as *const libc::c_void, is_loaded: false };
    pub static mut UseProgram: FnPtr = FnPtr { f: failing::UseProgram as *const libc::c_void, is_loaded: false };
    pub static mut UseProgramStages: FnPtr = FnPtr { f: failing::UseProgramStages as *const libc::c_void, is_loaded: false };
    pub static mut ValidateProgram: FnPtr = FnPtr { f: failing::ValidateProgram as *const libc::c_void, is_loaded: false };
    pub static mut ValidateProgramPipeline: FnPtr = FnPtr { f: failing::ValidateProgramPipeline as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib1d: FnPtr = FnPtr { f: failing::VertexAttrib1d as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib1dv: FnPtr = FnPtr { f: failing::VertexAttrib1dv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib1f: FnPtr = FnPtr { f: failing::VertexAttrib1f as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib1fv: FnPtr = FnPtr { f: failing::VertexAttrib1fv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib1s: FnPtr = FnPtr { f: failing::VertexAttrib1s as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib1sv: FnPtr = FnPtr { f: failing::VertexAttrib1sv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib2d: FnPtr = FnPtr { f: failing::VertexAttrib2d as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib2dv: FnPtr = FnPtr { f: failing::VertexAttrib2dv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib2f: FnPtr = FnPtr { f: failing::VertexAttrib2f as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib2fv: FnPtr = FnPtr { f: failing::VertexAttrib2fv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib2s: FnPtr = FnPtr { f: failing::VertexAttrib2s as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib2sv: FnPtr = FnPtr { f: failing::VertexAttrib2sv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib3d: FnPtr = FnPtr { f: failing::VertexAttrib3d as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib3dv: FnPtr = FnPtr { f: failing::VertexAttrib3dv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib3f: FnPtr = FnPtr { f: failing::VertexAttrib3f as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib3fv: FnPtr = FnPtr { f: failing::VertexAttrib3fv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib3s: FnPtr = FnPtr { f: failing::VertexAttrib3s as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib3sv: FnPtr = FnPtr { f: failing::VertexAttrib3sv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4Nbv: FnPtr = FnPtr { f: failing::VertexAttrib4Nbv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4Niv: FnPtr = FnPtr { f: failing::VertexAttrib4Niv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4Nsv: FnPtr = FnPtr { f: failing::VertexAttrib4Nsv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4Nub: FnPtr = FnPtr { f: failing::VertexAttrib4Nub as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4Nubv: FnPtr = FnPtr { f: failing::VertexAttrib4Nubv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4Nuiv: FnPtr = FnPtr { f: failing::VertexAttrib4Nuiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4Nusv: FnPtr = FnPtr { f: failing::VertexAttrib4Nusv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4bv: FnPtr = FnPtr { f: failing::VertexAttrib4bv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4d: FnPtr = FnPtr { f: failing::VertexAttrib4d as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4dv: FnPtr = FnPtr { f: failing::VertexAttrib4dv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4f: FnPtr = FnPtr { f: failing::VertexAttrib4f as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4fv: FnPtr = FnPtr { f: failing::VertexAttrib4fv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4iv: FnPtr = FnPtr { f: failing::VertexAttrib4iv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4s: FnPtr = FnPtr { f: failing::VertexAttrib4s as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4sv: FnPtr = FnPtr { f: failing::VertexAttrib4sv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4ubv: FnPtr = FnPtr { f: failing::VertexAttrib4ubv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4uiv: FnPtr = FnPtr { f: failing::VertexAttrib4uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttrib4usv: FnPtr = FnPtr { f: failing::VertexAttrib4usv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribBinding: FnPtr = FnPtr { f: failing::VertexAttribBinding as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribDivisor: FnPtr = FnPtr { f: failing::VertexAttribDivisor as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribFormat: FnPtr = FnPtr { f: failing::VertexAttribFormat as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI1i: FnPtr = FnPtr { f: failing::VertexAttribI1i as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI1iv: FnPtr = FnPtr { f: failing::VertexAttribI1iv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI1ui: FnPtr = FnPtr { f: failing::VertexAttribI1ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI1uiv: FnPtr = FnPtr { f: failing::VertexAttribI1uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI2i: FnPtr = FnPtr { f: failing::VertexAttribI2i as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI2iv: FnPtr = FnPtr { f: failing::VertexAttribI2iv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI2ui: FnPtr = FnPtr { f: failing::VertexAttribI2ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI2uiv: FnPtr = FnPtr { f: failing::VertexAttribI2uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI3i: FnPtr = FnPtr { f: failing::VertexAttribI3i as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI3iv: FnPtr = FnPtr { f: failing::VertexAttribI3iv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI3ui: FnPtr = FnPtr { f: failing::VertexAttribI3ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI3uiv: FnPtr = FnPtr { f: failing::VertexAttribI3uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI4bv: FnPtr = FnPtr { f: failing::VertexAttribI4bv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI4i: FnPtr = FnPtr { f: failing::VertexAttribI4i as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI4iv: FnPtr = FnPtr { f: failing::VertexAttribI4iv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI4sv: FnPtr = FnPtr { f: failing::VertexAttribI4sv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI4ubv: FnPtr = FnPtr { f: failing::VertexAttribI4ubv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI4ui: FnPtr = FnPtr { f: failing::VertexAttribI4ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI4uiv: FnPtr = FnPtr { f: failing::VertexAttribI4uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribI4usv: FnPtr = FnPtr { f: failing::VertexAttribI4usv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribIFormat: FnPtr = FnPtr { f: failing::VertexAttribIFormat as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribIPointer: FnPtr = FnPtr { f: failing::VertexAttribIPointer as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribL1d: FnPtr = FnPtr { f: failing::VertexAttribL1d as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribL1dv: FnPtr = FnPtr { f: failing::VertexAttribL1dv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribL2d: FnPtr = FnPtr { f: failing::VertexAttribL2d as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribL2dv: FnPtr = FnPtr { f: failing::VertexAttribL2dv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribL3d: FnPtr = FnPtr { f: failing::VertexAttribL3d as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribL3dv: FnPtr = FnPtr { f: failing::VertexAttribL3dv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribL4d: FnPtr = FnPtr { f: failing::VertexAttribL4d as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribL4dv: FnPtr = FnPtr { f: failing::VertexAttribL4dv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribLFormat: FnPtr = FnPtr { f: failing::VertexAttribLFormat as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribLPointer: FnPtr = FnPtr { f: failing::VertexAttribLPointer as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribP1ui: FnPtr = FnPtr { f: failing::VertexAttribP1ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribP1uiv: FnPtr = FnPtr { f: failing::VertexAttribP1uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribP2ui: FnPtr = FnPtr { f: failing::VertexAttribP2ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribP2uiv: FnPtr = FnPtr { f: failing::VertexAttribP2uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribP3ui: FnPtr = FnPtr { f: failing::VertexAttribP3ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribP3uiv: FnPtr = FnPtr { f: failing::VertexAttribP3uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribP4ui: FnPtr = FnPtr { f: failing::VertexAttribP4ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribP4uiv: FnPtr = FnPtr { f: failing::VertexAttribP4uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexAttribPointer: FnPtr = FnPtr { f: failing::VertexAttribPointer as *const libc::c_void, is_loaded: false };
    pub static mut VertexBindingDivisor: FnPtr = FnPtr { f: failing::VertexBindingDivisor as *const libc::c_void, is_loaded: false };
    pub static mut VertexP2ui: FnPtr = FnPtr { f: failing::VertexP2ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexP2uiv: FnPtr = FnPtr { f: failing::VertexP2uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexP3ui: FnPtr = FnPtr { f: failing::VertexP3ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexP3uiv: FnPtr = FnPtr { f: failing::VertexP3uiv as *const libc::c_void, is_loaded: false };
    pub static mut VertexP4ui: FnPtr = FnPtr { f: failing::VertexP4ui as *const libc::c_void, is_loaded: false };
    pub static mut VertexP4uiv: FnPtr = FnPtr { f: failing::VertexP4uiv as *const libc::c_void, is_loaded: false };
    pub static mut Viewport: FnPtr = FnPtr { f: failing::Viewport as *const libc::c_void, is_loaded: false };
    pub static mut ViewportArrayv: FnPtr = FnPtr { f: failing::ViewportArrayv as *const libc::c_void, is_loaded: false };
    pub static mut ViewportIndexedf: FnPtr = FnPtr { f: failing::ViewportIndexedf as *const libc::c_void, is_loaded: false };
    pub static mut ViewportIndexedfv: FnPtr = FnPtr { f: failing::ViewportIndexedfv as *const libc::c_void, is_loaded: false };
    pub static mut WaitSync: FnPtr = FnPtr { f: failing::WaitSync as *const libc::c_void, is_loaded: false };
}

macro_rules! fn_mod {
    ($name:ident, $sym:expr) => {
        pub mod $name {
            #[inline]
            pub fn is_loaded() -> bool { unsafe { ::storage::$name.is_loaded } }
            
            pub fn load_with(loadfn: |symbol: &str| -> *const ::libc::c_void) {
                unsafe { ::storage::$name = ::FnPtr::new(loadfn($sym), ::failing::$name as *const ::libc::c_void) }
            }
        }
    }
}

fn_mod!(ActiveShaderProgram, "glActiveShaderProgram")
fn_mod!(ActiveTexture, "glActiveTexture")
fn_mod!(AttachShader, "glAttachShader")
fn_mod!(BeginConditionalRender, "glBeginConditionalRender")
fn_mod!(BeginQuery, "glBeginQuery")
fn_mod!(BeginQueryIndexed, "glBeginQueryIndexed")
fn_mod!(BeginTransformFeedback, "glBeginTransformFeedback")
fn_mod!(BindAttribLocation, "glBindAttribLocation")
fn_mod!(BindBuffer, "glBindBuffer")
fn_mod!(BindBufferBase, "glBindBufferBase")
fn_mod!(BindBufferRange, "glBindBufferRange")
fn_mod!(BindFragDataLocation, "glBindFragDataLocation")
fn_mod!(BindFragDataLocationIndexed, "glBindFragDataLocationIndexed")
fn_mod!(BindFramebuffer, "glBindFramebuffer")
fn_mod!(BindImageTexture, "glBindImageTexture")
fn_mod!(BindProgramPipeline, "glBindProgramPipeline")
fn_mod!(BindRenderbuffer, "glBindRenderbuffer")
fn_mod!(BindSampler, "glBindSampler")
fn_mod!(BindTexture, "glBindTexture")
fn_mod!(BindTransformFeedback, "glBindTransformFeedback")
fn_mod!(BindVertexArray, "glBindVertexArray")
fn_mod!(BindVertexBuffer, "glBindVertexBuffer")
fn_mod!(BlendColor, "glBlendColor")
fn_mod!(BlendEquation, "glBlendEquation")
fn_mod!(BlendEquationSeparate, "glBlendEquationSeparate")
fn_mod!(BlendEquationSeparatei, "glBlendEquationSeparatei")
fn_mod!(BlendEquationi, "glBlendEquationi")
fn_mod!(BlendFunc, "glBlendFunc")
fn_mod!(BlendFuncSeparate, "glBlendFuncSeparate")
fn_mod!(BlendFuncSeparatei, "glBlendFuncSeparatei")
fn_mod!(BlendFunci, "glBlendFunci")
fn_mod!(BlitFramebuffer, "glBlitFramebuffer")
fn_mod!(BufferData, "glBufferData")
fn_mod!(BufferSubData, "glBufferSubData")
fn_mod!(CheckFramebufferStatus, "glCheckFramebufferStatus")
fn_mod!(ClampColor, "glClampColor")
fn_mod!(Clear, "glClear")
fn_mod!(ClearBufferData, "glClearBufferData")
fn_mod!(ClearBufferSubData, "glClearBufferSubData")
fn_mod!(ClearBufferfi, "glClearBufferfi")
fn_mod!(ClearBufferfv, "glClearBufferfv")
fn_mod!(ClearBufferiv, "glClearBufferiv")
fn_mod!(ClearBufferuiv, "glClearBufferuiv")
fn_mod!(ClearColor, "glClearColor")
fn_mod!(ClearDepth, "glClearDepth")
fn_mod!(ClearDepthf, "glClearDepthf")
fn_mod!(ClearStencil, "glClearStencil")
fn_mod!(ClientWaitSync, "glClientWaitSync")
fn_mod!(ColorMask, "glColorMask")
fn_mod!(ColorMaski, "glColorMaski")
fn_mod!(ColorP3ui, "glColorP3ui")
fn_mod!(ColorP3uiv, "glColorP3uiv")
fn_mod!(ColorP4ui, "glColorP4ui")
fn_mod!(ColorP4uiv, "glColorP4uiv")
fn_mod!(CompileShader, "glCompileShader")
fn_mod!(CompressedTexImage1D, "glCompressedTexImage1D")
fn_mod!(CompressedTexImage2D, "glCompressedTexImage2D")
fn_mod!(CompressedTexImage3D, "glCompressedTexImage3D")
fn_mod!(CompressedTexSubImage1D, "glCompressedTexSubImage1D")
fn_mod!(CompressedTexSubImage2D, "glCompressedTexSubImage2D")
fn_mod!(CompressedTexSubImage3D, "glCompressedTexSubImage3D")
fn_mod!(CopyBufferSubData, "glCopyBufferSubData")
fn_mod!(CopyImageSubData, "glCopyImageSubData")
fn_mod!(CopyTexImage1D, "glCopyTexImage1D")
fn_mod!(CopyTexImage2D, "glCopyTexImage2D")
fn_mod!(CopyTexSubImage1D, "glCopyTexSubImage1D")
fn_mod!(CopyTexSubImage2D, "glCopyTexSubImage2D")
fn_mod!(CopyTexSubImage3D, "glCopyTexSubImage3D")
fn_mod!(CreateProgram, "glCreateProgram")
fn_mod!(CreateShader, "glCreateShader")
fn_mod!(CreateShaderProgramv, "glCreateShaderProgramv")
fn_mod!(CullFace, "glCullFace")
fn_mod!(DebugMessageCallback, "glDebugMessageCallback")
fn_mod!(DebugMessageControl, "glDebugMessageControl")
fn_mod!(DebugMessageInsert, "glDebugMessageInsert")
fn_mod!(DeleteBuffers, "glDeleteBuffers")
fn_mod!(DeleteFramebuffers, "glDeleteFramebuffers")
fn_mod!(DeleteProgram, "glDeleteProgram")
fn_mod!(DeleteProgramPipelines, "glDeleteProgramPipelines")
fn_mod!(DeleteQueries, "glDeleteQueries")
fn_mod!(DeleteRenderbuffers, "glDeleteRenderbuffers")
fn_mod!(DeleteSamplers, "glDeleteSamplers")
fn_mod!(DeleteShader, "glDeleteShader")
fn_mod!(DeleteSync, "glDeleteSync")
fn_mod!(DeleteTextures, "glDeleteTextures")
fn_mod!(DeleteTransformFeedbacks, "glDeleteTransformFeedbacks")
fn_mod!(DeleteVertexArrays, "glDeleteVertexArrays")
fn_mod!(DepthFunc, "glDepthFunc")
fn_mod!(DepthMask, "glDepthMask")
fn_mod!(DepthRange, "glDepthRange")
fn_mod!(DepthRangeArrayv, "glDepthRangeArrayv")
fn_mod!(DepthRangeIndexed, "glDepthRangeIndexed")
fn_mod!(DepthRangef, "glDepthRangef")
fn_mod!(DetachShader, "glDetachShader")
fn_mod!(Disable, "glDisable")
fn_mod!(DisableVertexAttribArray, "glDisableVertexAttribArray")
fn_mod!(Disablei, "glDisablei")
fn_mod!(DispatchCompute, "glDispatchCompute")
fn_mod!(DispatchComputeIndirect, "glDispatchComputeIndirect")
fn_mod!(DrawArrays, "glDrawArrays")
fn_mod!(DrawArraysIndirect, "glDrawArraysIndirect")
fn_mod!(DrawArraysInstanced, "glDrawArraysInstanced")
fn_mod!(DrawArraysInstancedBaseInstance, "glDrawArraysInstancedBaseInstance")
fn_mod!(DrawBuffer, "glDrawBuffer")
fn_mod!(DrawBuffers, "glDrawBuffers")
fn_mod!(DrawElements, "glDrawElements")
fn_mod!(DrawElementsBaseVertex, "glDrawElementsBaseVertex")
fn_mod!(DrawElementsIndirect, "glDrawElementsIndirect")
fn_mod!(DrawElementsInstanced, "glDrawElementsInstanced")
fn_mod!(DrawElementsInstancedBaseInstance, "glDrawElementsInstancedBaseInstance")
fn_mod!(DrawElementsInstancedBaseVertex, "glDrawElementsInstancedBaseVertex")
fn_mod!(DrawElementsInstancedBaseVertexBaseInstance, "glDrawElementsInstancedBaseVertexBaseInstance")
fn_mod!(DrawRangeElements, "glDrawRangeElements")
fn_mod!(DrawRangeElementsBaseVertex, "glDrawRangeElementsBaseVertex")
fn_mod!(DrawTransformFeedback, "glDrawTransformFeedback")
fn_mod!(DrawTransformFeedbackInstanced, "glDrawTransformFeedbackInstanced")
fn_mod!(DrawTransformFeedbackStream, "glDrawTransformFeedbackStream")
fn_mod!(DrawTransformFeedbackStreamInstanced, "glDrawTransformFeedbackStreamInstanced")
fn_mod!(Enable, "glEnable")
fn_mod!(EnableVertexAttribArray, "glEnableVertexAttribArray")
fn_mod!(Enablei, "glEnablei")
fn_mod!(EndConditionalRender, "glEndConditionalRender")
fn_mod!(EndQuery, "glEndQuery")
fn_mod!(EndQueryIndexed, "glEndQueryIndexed")
fn_mod!(EndTransformFeedback, "glEndTransformFeedback")
fn_mod!(FenceSync, "glFenceSync")
fn_mod!(Finish, "glFinish")
fn_mod!(Flush, "glFlush")
fn_mod!(FlushMappedBufferRange, "glFlushMappedBufferRange")
fn_mod!(FramebufferParameteri, "glFramebufferParameteri")
fn_mod!(FramebufferRenderbuffer, "glFramebufferRenderbuffer")
fn_mod!(FramebufferTexture, "glFramebufferTexture")
fn_mod!(FramebufferTexture1D, "glFramebufferTexture1D")
fn_mod!(FramebufferTexture2D, "glFramebufferTexture2D")
fn_mod!(FramebufferTexture3D, "glFramebufferTexture3D")
fn_mod!(FramebufferTextureLayer, "glFramebufferTextureLayer")
fn_mod!(FrontFace, "glFrontFace")
fn_mod!(GenBuffers, "glGenBuffers")
fn_mod!(GenFramebuffers, "glGenFramebuffers")
fn_mod!(GenProgramPipelines, "glGenProgramPipelines")
fn_mod!(GenQueries, "glGenQueries")
fn_mod!(GenRenderbuffers, "glGenRenderbuffers")
fn_mod!(GenSamplers, "glGenSamplers")
fn_mod!(GenTextures, "glGenTextures")
fn_mod!(GenTransformFeedbacks, "glGenTransformFeedbacks")
fn_mod!(GenVertexArrays, "glGenVertexArrays")
fn_mod!(GenerateMipmap, "glGenerateMipmap")
fn_mod!(GetActiveAtomicCounterBufferiv, "glGetActiveAtomicCounterBufferiv")
fn_mod!(GetActiveAttrib, "glGetActiveAttrib")
fn_mod!(GetActiveSubroutineName, "glGetActiveSubroutineName")
fn_mod!(GetActiveSubroutineUniformName, "glGetActiveSubroutineUniformName")
fn_mod!(GetActiveSubroutineUniformiv, "glGetActiveSubroutineUniformiv")
fn_mod!(GetActiveUniform, "glGetActiveUniform")
fn_mod!(GetActiveUniformBlockName, "glGetActiveUniformBlockName")
fn_mod!(GetActiveUniformBlockiv, "glGetActiveUniformBlockiv")
fn_mod!(GetActiveUniformName, "glGetActiveUniformName")
fn_mod!(GetActiveUniformsiv, "glGetActiveUniformsiv")
fn_mod!(GetAttachedShaders, "glGetAttachedShaders")
fn_mod!(GetAttribLocation, "glGetAttribLocation")
fn_mod!(GetBooleani_v, "glGetBooleani_v")
fn_mod!(GetBooleanv, "glGetBooleanv")
fn_mod!(GetBufferParameteri64v, "glGetBufferParameteri64v")
fn_mod!(GetBufferParameteriv, "glGetBufferParameteriv")
fn_mod!(GetBufferPointerv, "glGetBufferPointerv")
fn_mod!(GetBufferSubData, "glGetBufferSubData")
fn_mod!(GetCompressedTexImage, "glGetCompressedTexImage")
fn_mod!(GetDebugMessageLog, "glGetDebugMessageLog")
fn_mod!(GetDoublei_v, "glGetDoublei_v")
fn_mod!(GetDoublev, "glGetDoublev")
fn_mod!(GetError, "glGetError")
fn_mod!(GetFloati_v, "glGetFloati_v")
fn_mod!(GetFloatv, "glGetFloatv")
fn_mod!(GetFragDataIndex, "glGetFragDataIndex")
fn_mod!(GetFragDataLocation, "glGetFragDataLocation")
fn_mod!(GetFramebufferAttachmentParameteriv, "glGetFramebufferAttachmentParameteriv")
fn_mod!(GetFramebufferParameteriv, "glGetFramebufferParameteriv")
fn_mod!(GetInteger64i_v, "glGetInteger64i_v")
fn_mod!(GetInteger64v, "glGetInteger64v")
fn_mod!(GetIntegeri_v, "glGetIntegeri_v")
fn_mod!(GetIntegerv, "glGetIntegerv")
fn_mod!(GetInternalformati64v, "glGetInternalformati64v")
fn_mod!(GetInternalformativ, "glGetInternalformativ")
fn_mod!(GetMultisamplefv, "glGetMultisamplefv")
fn_mod!(GetObjectLabel, "glGetObjectLabel")
fn_mod!(GetObjectPtrLabel, "glGetObjectPtrLabel")
fn_mod!(GetProgramBinary, "glGetProgramBinary")
fn_mod!(GetProgramInfoLog, "glGetProgramInfoLog")
fn_mod!(GetProgramInterfaceiv, "glGetProgramInterfaceiv")
fn_mod!(GetProgramPipelineInfoLog, "glGetProgramPipelineInfoLog")
fn_mod!(GetProgramPipelineiv, "glGetProgramPipelineiv")
fn_mod!(GetProgramResourceIndex, "glGetProgramResourceIndex")
fn_mod!(GetProgramResourceLocation, "glGetProgramResourceLocation")
fn_mod!(GetProgramResourceLocationIndex, "glGetProgramResourceLocationIndex")
fn_mod!(GetProgramResourceName, "glGetProgramResourceName")
fn_mod!(GetProgramResourceiv, "glGetProgramResourceiv")
fn_mod!(GetProgramStageiv, "glGetProgramStageiv")
fn_mod!(GetProgramiv, "glGetProgramiv")
fn_mod!(GetQueryIndexediv, "glGetQueryIndexediv")
fn_mod!(GetQueryObjecti64v, "glGetQueryObjecti64v")
fn_mod!(GetQueryObjectiv, "glGetQueryObjectiv")
fn_mod!(GetQueryObjectui64v, "glGetQueryObjectui64v")
fn_mod!(GetQueryObjectuiv, "glGetQueryObjectuiv")
fn_mod!(GetQueryiv, "glGetQueryiv")
fn_mod!(GetRenderbufferParameteriv, "glGetRenderbufferParameteriv")
fn_mod!(GetSamplerParameterIiv, "glGetSamplerParameterIiv")
fn_mod!(GetSamplerParameterIuiv, "glGetSamplerParameterIuiv")
fn_mod!(GetSamplerParameterfv, "glGetSamplerParameterfv")
fn_mod!(GetSamplerParameteriv, "glGetSamplerParameteriv")
fn_mod!(GetShaderInfoLog, "glGetShaderInfoLog")
fn_mod!(GetShaderPrecisionFormat, "glGetShaderPrecisionFormat")
fn_mod!(GetShaderSource, "glGetShaderSource")
fn_mod!(GetShaderiv, "glGetShaderiv")
fn_mod!(GetString, "glGetString")
fn_mod!(GetStringi, "glGetStringi")
fn_mod!(GetSubroutineIndex, "glGetSubroutineIndex")
fn_mod!(GetSubroutineUniformLocation, "glGetSubroutineUniformLocation")
fn_mod!(GetSynciv, "glGetSynciv")
fn_mod!(GetTexImage, "glGetTexImage")
fn_mod!(GetTexLevelParameterfv, "glGetTexLevelParameterfv")
fn_mod!(GetTexLevelParameteriv, "glGetTexLevelParameteriv")
fn_mod!(GetTexParameterIiv, "glGetTexParameterIiv")
fn_mod!(GetTexParameterIuiv, "glGetTexParameterIuiv")
fn_mod!(GetTexParameterfv, "glGetTexParameterfv")
fn_mod!(GetTexParameteriv, "glGetTexParameteriv")
fn_mod!(GetTransformFeedbackVarying, "glGetTransformFeedbackVarying")
fn_mod!(GetUniformBlockIndex, "glGetUniformBlockIndex")
fn_mod!(GetUniformIndices, "glGetUniformIndices")
fn_mod!(GetUniformLocation, "glGetUniformLocation")
fn_mod!(GetUniformSubroutineuiv, "glGetUniformSubroutineuiv")
fn_mod!(GetUniformdv, "glGetUniformdv")
fn_mod!(GetUniformfv, "glGetUniformfv")
fn_mod!(GetUniformiv, "glGetUniformiv")
fn_mod!(GetUniformuiv, "glGetUniformuiv")
fn_mod!(GetVertexAttribIiv, "glGetVertexAttribIiv")
fn_mod!(GetVertexAttribIuiv, "glGetVertexAttribIuiv")
fn_mod!(GetVertexAttribLdv, "glGetVertexAttribLdv")
fn_mod!(GetVertexAttribPointerv, "glGetVertexAttribPointerv")
fn_mod!(GetVertexAttribdv, "glGetVertexAttribdv")
fn_mod!(GetVertexAttribfv, "glGetVertexAttribfv")
fn_mod!(GetVertexAttribiv, "glGetVertexAttribiv")
fn_mod!(Hint, "glHint")
fn_mod!(InvalidateBufferData, "glInvalidateBufferData")
fn_mod!(InvalidateBufferSubData, "glInvalidateBufferSubData")
fn_mod!(InvalidateFramebuffer, "glInvalidateFramebuffer")
fn_mod!(InvalidateSubFramebuffer, "glInvalidateSubFramebuffer")
fn_mod!(InvalidateTexImage, "glInvalidateTexImage")
fn_mod!(InvalidateTexSubImage, "glInvalidateTexSubImage")
fn_mod!(IsBuffer, "glIsBuffer")
fn_mod!(IsEnabled, "glIsEnabled")
fn_mod!(IsEnabledi, "glIsEnabledi")
fn_mod!(IsFramebuffer, "glIsFramebuffer")
fn_mod!(IsProgram, "glIsProgram")
fn_mod!(IsProgramPipeline, "glIsProgramPipeline")
fn_mod!(IsQuery, "glIsQuery")
fn_mod!(IsRenderbuffer, "glIsRenderbuffer")
fn_mod!(IsSampler, "glIsSampler")
fn_mod!(IsShader, "glIsShader")
fn_mod!(IsSync, "glIsSync")
fn_mod!(IsTexture, "glIsTexture")
fn_mod!(IsTransformFeedback, "glIsTransformFeedback")
fn_mod!(IsVertexArray, "glIsVertexArray")
fn_mod!(LineWidth, "glLineWidth")
fn_mod!(LinkProgram, "glLinkProgram")
fn_mod!(LogicOp, "glLogicOp")
fn_mod!(MapBuffer, "glMapBuffer")
fn_mod!(MapBufferRange, "glMapBufferRange")
fn_mod!(MemoryBarrier, "glMemoryBarrier")
fn_mod!(MinSampleShading, "glMinSampleShading")
fn_mod!(MultiDrawArrays, "glMultiDrawArrays")
fn_mod!(MultiDrawArraysIndirect, "glMultiDrawArraysIndirect")
fn_mod!(MultiDrawElements, "glMultiDrawElements")
fn_mod!(MultiDrawElementsBaseVertex, "glMultiDrawElementsBaseVertex")
fn_mod!(MultiDrawElementsIndirect, "glMultiDrawElementsIndirect")
fn_mod!(MultiTexCoordP1ui, "glMultiTexCoordP1ui")
fn_mod!(MultiTexCoordP1uiv, "glMultiTexCoordP1uiv")
fn_mod!(MultiTexCoordP2ui, "glMultiTexCoordP2ui")
fn_mod!(MultiTexCoordP2uiv, "glMultiTexCoordP2uiv")
fn_mod!(MultiTexCoordP3ui, "glMultiTexCoordP3ui")
fn_mod!(MultiTexCoordP3uiv, "glMultiTexCoordP3uiv")
fn_mod!(MultiTexCoordP4ui, "glMultiTexCoordP4ui")
fn_mod!(MultiTexCoordP4uiv, "glMultiTexCoordP4uiv")
fn_mod!(NormalP3ui, "glNormalP3ui")
fn_mod!(NormalP3uiv, "glNormalP3uiv")
fn_mod!(ObjectLabel, "glObjectLabel")
fn_mod!(ObjectPtrLabel, "glObjectPtrLabel")
fn_mod!(PatchParameterfv, "glPatchParameterfv")
fn_mod!(PatchParameteri, "glPatchParameteri")
fn_mod!(PauseTransformFeedback, "glPauseTransformFeedback")
fn_mod!(PixelStoref, "glPixelStoref")
fn_mod!(PixelStorei, "glPixelStorei")
fn_mod!(PointParameterf, "glPointParameterf")
fn_mod!(PointParameterfv, "glPointParameterfv")
fn_mod!(PointParameteri, "glPointParameteri")
fn_mod!(PointParameteriv, "glPointParameteriv")
fn_mod!(PointSize, "glPointSize")
fn_mod!(PolygonMode, "glPolygonMode")
fn_mod!(PolygonOffset, "glPolygonOffset")
fn_mod!(PopDebugGroup, "glPopDebugGroup")
fn_mod!(PrimitiveRestartIndex, "glPrimitiveRestartIndex")
fn_mod!(ProgramBinary, "glProgramBinary")
fn_mod!(ProgramParameteri, "glProgramParameteri")
fn_mod!(ProgramUniform1d, "glProgramUniform1d")
fn_mod!(ProgramUniform1dv, "glProgramUniform1dv")
fn_mod!(ProgramUniform1f, "glProgramUniform1f")
fn_mod!(ProgramUniform1fv, "glProgramUniform1fv")
fn_mod!(ProgramUniform1i, "glProgramUniform1i")
fn_mod!(ProgramUniform1iv, "glProgramUniform1iv")
fn_mod!(ProgramUniform1ui, "glProgramUniform1ui")
fn_mod!(ProgramUniform1uiv, "glProgramUniform1uiv")
fn_mod!(ProgramUniform2d, "glProgramUniform2d")
fn_mod!(ProgramUniform2dv, "glProgramUniform2dv")
fn_mod!(ProgramUniform2f, "glProgramUniform2f")
fn_mod!(ProgramUniform2fv, "glProgramUniform2fv")
fn_mod!(ProgramUniform2i, "glProgramUniform2i")
fn_mod!(ProgramUniform2iv, "glProgramUniform2iv")
fn_mod!(ProgramUniform2ui, "glProgramUniform2ui")
fn_mod!(ProgramUniform2uiv, "glProgramUniform2uiv")
fn_mod!(ProgramUniform3d, "glProgramUniform3d")
fn_mod!(ProgramUniform3dv, "glProgramUniform3dv")
fn_mod!(ProgramUniform3f, "glProgramUniform3f")
fn_mod!(ProgramUniform3fv, "glProgramUniform3fv")
fn_mod!(ProgramUniform3i, "glProgramUniform3i")
fn_mod!(ProgramUniform3iv, "glProgramUniform3iv")
fn_mod!(ProgramUniform3ui, "glProgramUniform3ui")
fn_mod!(ProgramUniform3uiv, "glProgramUniform3uiv")
fn_mod!(ProgramUniform4d, "glProgramUniform4d")
fn_mod!(ProgramUniform4dv, "glProgramUniform4dv")
fn_mod!(ProgramUniform4f, "glProgramUniform4f")
fn_mod!(ProgramUniform4fv, "glProgramUniform4fv")
fn_mod!(ProgramUniform4i, "glProgramUniform4i")
fn_mod!(ProgramUniform4iv, "glProgramUniform4iv")
fn_mod!(ProgramUniform4ui, "glProgramUniform4ui")
fn_mod!(ProgramUniform4uiv, "glProgramUniform4uiv")
fn_mod!(ProgramUniformMatrix2dv, "glProgramUniformMatrix2dv")
fn_mod!(ProgramUniformMatrix2fv, "glProgramUniformMatrix2fv")
fn_mod!(ProgramUniformMatrix2x3dv, "glProgramUniformMatrix2x3dv")
fn_mod!(ProgramUniformMatrix2x3fv, "glProgramUniformMatrix2x3fv")
fn_mod!(ProgramUniformMatrix2x4dv, "glProgramUniformMatrix2x4dv")
fn_mod!(ProgramUniformMatrix2x4fv, "glProgramUniformMatrix2x4fv")
fn_mod!(ProgramUniformMatrix3dv, "glProgramUniformMatrix3dv")
fn_mod!(ProgramUniformMatrix3fv, "glProgramUniformMatrix3fv")
fn_mod!(ProgramUniformMatrix3x2dv, "glProgramUniformMatrix3x2dv")
fn_mod!(ProgramUniformMatrix3x2fv, "glProgramUniformMatrix3x2fv")
fn_mod!(ProgramUniformMatrix3x4dv, "glProgramUniformMatrix3x4dv")
fn_mod!(ProgramUniformMatrix3x4fv, "glProgramUniformMatrix3x4fv")
fn_mod!(ProgramUniformMatrix4dv, "glProgramUniformMatrix4dv")
fn_mod!(ProgramUniformMatrix4fv, "glProgramUniformMatrix4fv")
fn_mod!(ProgramUniformMatrix4x2dv, "glProgramUniformMatrix4x2dv")
fn_mod!(ProgramUniformMatrix4x2fv, "glProgramUniformMatrix4x2fv")
fn_mod!(ProgramUniformMatrix4x3dv, "glProgramUniformMatrix4x3dv")
fn_mod!(ProgramUniformMatrix4x3fv, "glProgramUniformMatrix4x3fv")
fn_mod!(ProvokingVertex, "glProvokingVertex")
fn_mod!(PushDebugGroup, "glPushDebugGroup")
fn_mod!(QueryCounter, "glQueryCounter")
fn_mod!(ReadBuffer, "glReadBuffer")
fn_mod!(ReadPixels, "glReadPixels")
fn_mod!(ReleaseShaderCompiler, "glReleaseShaderCompiler")
fn_mod!(RenderbufferStorage, "glRenderbufferStorage")
fn_mod!(RenderbufferStorageMultisample, "glRenderbufferStorageMultisample")
fn_mod!(ResumeTransformFeedback, "glResumeTransformFeedback")
fn_mod!(SampleCoverage, "glSampleCoverage")
fn_mod!(SampleMaski, "glSampleMaski")
fn_mod!(SamplerParameterIiv, "glSamplerParameterIiv")
fn_mod!(SamplerParameterIuiv, "glSamplerParameterIuiv")
fn_mod!(SamplerParameterf, "glSamplerParameterf")
fn_mod!(SamplerParameterfv, "glSamplerParameterfv")
fn_mod!(SamplerParameteri, "glSamplerParameteri")
fn_mod!(SamplerParameteriv, "glSamplerParameteriv")
fn_mod!(Scissor, "glScissor")
fn_mod!(ScissorArrayv, "glScissorArrayv")
fn_mod!(ScissorIndexed, "glScissorIndexed")
fn_mod!(ScissorIndexedv, "glScissorIndexedv")
fn_mod!(SecondaryColorP3ui, "glSecondaryColorP3ui")
fn_mod!(SecondaryColorP3uiv, "glSecondaryColorP3uiv")
fn_mod!(ShaderBinary, "glShaderBinary")
fn_mod!(ShaderSource, "glShaderSource")
fn_mod!(ShaderStorageBlockBinding, "glShaderStorageBlockBinding")
fn_mod!(StencilFunc, "glStencilFunc")
fn_mod!(StencilFuncSeparate, "glStencilFuncSeparate")
fn_mod!(StencilMask, "glStencilMask")
fn_mod!(StencilMaskSeparate, "glStencilMaskSeparate")
fn_mod!(StencilOp, "glStencilOp")
fn_mod!(StencilOpSeparate, "glStencilOpSeparate")
fn_mod!(TexBuffer, "glTexBuffer")
fn_mod!(TexBufferRange, "glTexBufferRange")
fn_mod!(TexCoordP1ui, "glTexCoordP1ui")
fn_mod!(TexCoordP1uiv, "glTexCoordP1uiv")
fn_mod!(TexCoordP2ui, "glTexCoordP2ui")
fn_mod!(TexCoordP2uiv, "glTexCoordP2uiv")
fn_mod!(TexCoordP3ui, "glTexCoordP3ui")
fn_mod!(TexCoordP3uiv, "glTexCoordP3uiv")
fn_mod!(TexCoordP4ui, "glTexCoordP4ui")
fn_mod!(TexCoordP4uiv, "glTexCoordP4uiv")
fn_mod!(TexImage1D, "glTexImage1D")
fn_mod!(TexImage2D, "glTexImage2D")
fn_mod!(TexImage2DMultisample, "glTexImage2DMultisample")
fn_mod!(TexImage3D, "glTexImage3D")
fn_mod!(TexImage3DMultisample, "glTexImage3DMultisample")
fn_mod!(TexParameterIiv, "glTexParameterIiv")
fn_mod!(TexParameterIuiv, "glTexParameterIuiv")
fn_mod!(TexParameterf, "glTexParameterf")
fn_mod!(TexParameterfv, "glTexParameterfv")
fn_mod!(TexParameteri, "glTexParameteri")
fn_mod!(TexParameteriv, "glTexParameteriv")
fn_mod!(TexStorage1D, "glTexStorage1D")
fn_mod!(TexStorage2D, "glTexStorage2D")
fn_mod!(TexStorage2DMultisample, "glTexStorage2DMultisample")
fn_mod!(TexStorage3D, "glTexStorage3D")
fn_mod!(TexStorage3DMultisample, "glTexStorage3DMultisample")
fn_mod!(TexSubImage1D, "glTexSubImage1D")
fn_mod!(TexSubImage2D, "glTexSubImage2D")
fn_mod!(TexSubImage3D, "glTexSubImage3D")
fn_mod!(TextureView, "glTextureView")
fn_mod!(TransformFeedbackVaryings, "glTransformFeedbackVaryings")
fn_mod!(Uniform1d, "glUniform1d")
fn_mod!(Uniform1dv, "glUniform1dv")
fn_mod!(Uniform1f, "glUniform1f")
fn_mod!(Uniform1fv, "glUniform1fv")
fn_mod!(Uniform1i, "glUniform1i")
fn_mod!(Uniform1iv, "glUniform1iv")
fn_mod!(Uniform1ui, "glUniform1ui")
fn_mod!(Uniform1uiv, "glUniform1uiv")
fn_mod!(Uniform2d, "glUniform2d")
fn_mod!(Uniform2dv, "glUniform2dv")
fn_mod!(Uniform2f, "glUniform2f")
fn_mod!(Uniform2fv, "glUniform2fv")
fn_mod!(Uniform2i, "glUniform2i")
fn_mod!(Uniform2iv, "glUniform2iv")
fn_mod!(Uniform2ui, "glUniform2ui")
fn_mod!(Uniform2uiv, "glUniform2uiv")
fn_mod!(Uniform3d, "glUniform3d")
fn_mod!(Uniform3dv, "glUniform3dv")
fn_mod!(Uniform3f, "glUniform3f")
fn_mod!(Uniform3fv, "glUniform3fv")
fn_mod!(Uniform3i, "glUniform3i")
fn_mod!(Uniform3iv, "glUniform3iv")
fn_mod!(Uniform3ui, "glUniform3ui")
fn_mod!(Uniform3uiv, "glUniform3uiv")
fn_mod!(Uniform4d, "glUniform4d")
fn_mod!(Uniform4dv, "glUniform4dv")
fn_mod!(Uniform4f, "glUniform4f")
fn_mod!(Uniform4fv, "glUniform4fv")
fn_mod!(Uniform4i, "glUniform4i")
fn_mod!(Uniform4iv, "glUniform4iv")
fn_mod!(Uniform4ui, "glUniform4ui")
fn_mod!(Uniform4uiv, "glUniform4uiv")
fn_mod!(UniformBlockBinding, "glUniformBlockBinding")
fn_mod!(UniformMatrix2dv, "glUniformMatrix2dv")
fn_mod!(UniformMatrix2fv, "glUniformMatrix2fv")
fn_mod!(UniformMatrix2x3dv, "glUniformMatrix2x3dv")
fn_mod!(UniformMatrix2x3fv, "glUniformMatrix2x3fv")
fn_mod!(UniformMatrix2x4dv, "glUniformMatrix2x4dv")
fn_mod!(UniformMatrix2x4fv, "glUniformMatrix2x4fv")
fn_mod!(UniformMatrix3dv, "glUniformMatrix3dv")
fn_mod!(UniformMatrix3fv, "glUniformMatrix3fv")
fn_mod!(UniformMatrix3x2dv, "glUniformMatrix3x2dv")
fn_mod!(UniformMatrix3x2fv, "glUniformMatrix3x2fv")
fn_mod!(UniformMatrix3x4dv, "glUniformMatrix3x4dv")
fn_mod!(UniformMatrix3x4fv, "glUniformMatrix3x4fv")
fn_mod!(UniformMatrix4dv, "glUniformMatrix4dv")
fn_mod!(UniformMatrix4fv, "glUniformMatrix4fv")
fn_mod!(UniformMatrix4x2dv, "glUniformMatrix4x2dv")
fn_mod!(UniformMatrix4x2fv, "glUniformMatrix4x2fv")
fn_mod!(UniformMatrix4x3dv, "glUniformMatrix4x3dv")
fn_mod!(UniformMatrix4x3fv, "glUniformMatrix4x3fv")
fn_mod!(UniformSubroutinesuiv, "glUniformSubroutinesuiv")
fn_mod!(UnmapBuffer, "glUnmapBuffer")
fn_mod!(UseProgram, "glUseProgram")
fn_mod!(UseProgramStages, "glUseProgramStages")
fn_mod!(ValidateProgram, "glValidateProgram")
fn_mod!(ValidateProgramPipeline, "glValidateProgramPipeline")
fn_mod!(VertexAttrib1d, "glVertexAttrib1d")
fn_mod!(VertexAttrib1dv, "glVertexAttrib1dv")
fn_mod!(VertexAttrib1f, "glVertexAttrib1f")
fn_mod!(VertexAttrib1fv, "glVertexAttrib1fv")
fn_mod!(VertexAttrib1s, "glVertexAttrib1s")
fn_mod!(VertexAttrib1sv, "glVertexAttrib1sv")
fn_mod!(VertexAttrib2d, "glVertexAttrib2d")
fn_mod!(VertexAttrib2dv, "glVertexAttrib2dv")
fn_mod!(VertexAttrib2f, "glVertexAttrib2f")
fn_mod!(VertexAttrib2fv, "glVertexAttrib2fv")
fn_mod!(VertexAttrib2s, "glVertexAttrib2s")
fn_mod!(VertexAttrib2sv, "glVertexAttrib2sv")
fn_mod!(VertexAttrib3d, "glVertexAttrib3d")
fn_mod!(VertexAttrib3dv, "glVertexAttrib3dv")
fn_mod!(VertexAttrib3f, "glVertexAttrib3f")
fn_mod!(VertexAttrib3fv, "glVertexAttrib3fv")
fn_mod!(VertexAttrib3s, "glVertexAttrib3s")
fn_mod!(VertexAttrib3sv, "glVertexAttrib3sv")
fn_mod!(VertexAttrib4Nbv, "glVertexAttrib4Nbv")
fn_mod!(VertexAttrib4Niv, "glVertexAttrib4Niv")
fn_mod!(VertexAttrib4Nsv, "glVertexAttrib4Nsv")
fn_mod!(VertexAttrib4Nub, "glVertexAttrib4Nub")
fn_mod!(VertexAttrib4Nubv, "glVertexAttrib4Nubv")
fn_mod!(VertexAttrib4Nuiv, "glVertexAttrib4Nuiv")
fn_mod!(VertexAttrib4Nusv, "glVertexAttrib4Nusv")
fn_mod!(VertexAttrib4bv, "glVertexAttrib4bv")
fn_mod!(VertexAttrib4d, "glVertexAttrib4d")
fn_mod!(VertexAttrib4dv, "glVertexAttrib4dv")
fn_mod!(VertexAttrib4f, "glVertexAttrib4f")
fn_mod!(VertexAttrib4fv, "glVertexAttrib4fv")
fn_mod!(VertexAttrib4iv, "glVertexAttrib4iv")
fn_mod!(VertexAttrib4s, "glVertexAttrib4s")
fn_mod!(VertexAttrib4sv, "glVertexAttrib4sv")
fn_mod!(VertexAttrib4ubv, "glVertexAttrib4ubv")
fn_mod!(VertexAttrib4uiv, "glVertexAttrib4uiv")
fn_mod!(VertexAttrib4usv, "glVertexAttrib4usv")
fn_mod!(VertexAttribBinding, "glVertexAttribBinding")
fn_mod!(VertexAttribDivisor, "glVertexAttribDivisor")
fn_mod!(VertexAttribFormat, "glVertexAttribFormat")
fn_mod!(VertexAttribI1i, "glVertexAttribI1i")
fn_mod!(VertexAttribI1iv, "glVertexAttribI1iv")
fn_mod!(VertexAttribI1ui, "glVertexAttribI1ui")
fn_mod!(VertexAttribI1uiv, "glVertexAttribI1uiv")
fn_mod!(VertexAttribI2i, "glVertexAttribI2i")
fn_mod!(VertexAttribI2iv, "glVertexAttribI2iv")
fn_mod!(VertexAttribI2ui, "glVertexAttribI2ui")
fn_mod!(VertexAttribI2uiv, "glVertexAttribI2uiv")
fn_mod!(VertexAttribI3i, "glVertexAttribI3i")
fn_mod!(VertexAttribI3iv, "glVertexAttribI3iv")
fn_mod!(VertexAttribI3ui, "glVertexAttribI3ui")
fn_mod!(VertexAttribI3uiv, "glVertexAttribI3uiv")
fn_mod!(VertexAttribI4bv, "glVertexAttribI4bv")
fn_mod!(VertexAttribI4i, "glVertexAttribI4i")
fn_mod!(VertexAttribI4iv, "glVertexAttribI4iv")
fn_mod!(VertexAttribI4sv, "glVertexAttribI4sv")
fn_mod!(VertexAttribI4ubv, "glVertexAttribI4ubv")
fn_mod!(VertexAttribI4ui, "glVertexAttribI4ui")
fn_mod!(VertexAttribI4uiv, "glVertexAttribI4uiv")
fn_mod!(VertexAttribI4usv, "glVertexAttribI4usv")
fn_mod!(VertexAttribIFormat, "glVertexAttribIFormat")
fn_mod!(VertexAttribIPointer, "glVertexAttribIPointer")
fn_mod!(VertexAttribL1d, "glVertexAttribL1d")
fn_mod!(VertexAttribL1dv, "glVertexAttribL1dv")
fn_mod!(VertexAttribL2d, "glVertexAttribL2d")
fn_mod!(VertexAttribL2dv, "glVertexAttribL2dv")
fn_mod!(VertexAttribL3d, "glVertexAttribL3d")
fn_mod!(VertexAttribL3dv, "glVertexAttribL3dv")
fn_mod!(VertexAttribL4d, "glVertexAttribL4d")
fn_mod!(VertexAttribL4dv, "glVertexAttribL4dv")
fn_mod!(VertexAttribLFormat, "glVertexAttribLFormat")
fn_mod!(VertexAttribLPointer, "glVertexAttribLPointer")
fn_mod!(VertexAttribP1ui, "glVertexAttribP1ui")
fn_mod!(VertexAttribP1uiv, "glVertexAttribP1uiv")
fn_mod!(VertexAttribP2ui, "glVertexAttribP2ui")
fn_mod!(VertexAttribP2uiv, "glVertexAttribP2uiv")
fn_mod!(VertexAttribP3ui, "glVertexAttribP3ui")
fn_mod!(VertexAttribP3uiv, "glVertexAttribP3uiv")
fn_mod!(VertexAttribP4ui, "glVertexAttribP4ui")
fn_mod!(VertexAttribP4uiv, "glVertexAttribP4uiv")
fn_mod!(VertexAttribPointer, "glVertexAttribPointer")
fn_mod!(VertexBindingDivisor, "glVertexBindingDivisor")
fn_mod!(VertexP2ui, "glVertexP2ui")
fn_mod!(VertexP2uiv, "glVertexP2uiv")
fn_mod!(VertexP3ui, "glVertexP3ui")
fn_mod!(VertexP3uiv, "glVertexP3uiv")
fn_mod!(VertexP4ui, "glVertexP4ui")
fn_mod!(VertexP4uiv, "glVertexP4uiv")
fn_mod!(Viewport, "glViewport")
fn_mod!(ViewportArrayv, "glViewportArrayv")
fn_mod!(ViewportIndexedf, "glViewportIndexedf")
fn_mod!(ViewportIndexedfv, "glViewportIndexedfv")
fn_mod!(WaitSync, "glWaitSync")

mod failing {
    use libc::*;
    use super::types::*;
    
    pub extern "system" fn ActiveShaderProgram(pipeline: GLuint, program: GLuint) { fail!("`ActiveShaderProgram` was not loaded") }
    pub extern "system" fn ActiveTexture(texture: GLenum) { fail!("`ActiveTexture` was not loaded") }
    pub extern "system" fn AttachShader(program: GLuint, shader: GLuint) { fail!("`AttachShader` was not loaded") }
    pub extern "system" fn BeginConditionalRender(id: GLuint, mode: GLenum) { fail!("`BeginConditionalRender` was not loaded") }
    pub extern "system" fn BeginQuery(target: GLenum, id: GLuint) { fail!("`BeginQuery` was not loaded") }
    pub extern "system" fn BeginQueryIndexed(target: GLenum, index: GLuint, id: GLuint) { fail!("`BeginQueryIndexed` was not loaded") }
    pub extern "system" fn BeginTransformFeedback(primitiveMode: GLenum) { fail!("`BeginTransformFeedback` was not loaded") }
    pub extern "system" fn BindAttribLocation(program: GLuint, index: GLuint, name: *const GLchar) { fail!("`BindAttribLocation` was not loaded") }
    pub extern "system" fn BindBuffer(target: GLenum, buffer: GLuint) { fail!("`BindBuffer` was not loaded") }
    pub extern "system" fn BindBufferBase(target: GLenum, index: GLuint, buffer: GLuint) { fail!("`BindBufferBase` was not loaded") }
    pub extern "system" fn BindBufferRange(target: GLenum, index: GLuint, buffer: GLuint, offset: GLintptr, size: GLsizeiptr) { fail!("`BindBufferRange` was not loaded") }
    pub extern "system" fn BindFragDataLocation(program: GLuint, color: GLuint, name: *const GLchar) { fail!("`BindFragDataLocation` was not loaded") }
    pub extern "system" fn BindFragDataLocationIndexed(program: GLuint, colorNumber: GLuint, index: GLuint, name: *const GLchar) { fail!("`BindFragDataLocationIndexed` was not loaded") }
    pub extern "system" fn BindFramebuffer(target: GLenum, framebuffer: GLuint) { fail!("`BindFramebuffer` was not loaded") }
    pub extern "system" fn BindImageTexture(unit: GLuint, texture: GLuint, level: GLint, layered: GLboolean, layer: GLint, access: GLenum, format: GLenum) { fail!("`BindImageTexture` was not loaded") }
    pub extern "system" fn BindProgramPipeline(pipeline: GLuint) { fail!("`BindProgramPipeline` was not loaded") }
    pub extern "system" fn BindRenderbuffer(target: GLenum, renderbuffer: GLuint) { fail!("`BindRenderbuffer` was not loaded") }
    pub extern "system" fn BindSampler(unit: GLuint, sampler: GLuint) { fail!("`BindSampler` was not loaded") }
    pub extern "system" fn BindTexture(target: GLenum, texture: GLuint) { fail!("`BindTexture` was not loaded") }
    pub extern "system" fn BindTransformFeedback(target: GLenum, id: GLuint) { fail!("`BindTransformFeedback` was not loaded") }
    pub extern "system" fn BindVertexArray(array: GLuint) { fail!("`BindVertexArray` was not loaded") }
    pub extern "system" fn BindVertexBuffer(bindingindex: GLuint, buffer: GLuint, offset: GLintptr, stride: GLsizei) { fail!("`BindVertexBuffer` was not loaded") }
    pub extern "system" fn BlendColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) { fail!("`BlendColor` was not loaded") }
    pub extern "system" fn BlendEquation(mode: GLenum) { fail!("`BlendEquation` was not loaded") }
    pub extern "system" fn BlendEquationSeparate(modeRGB: GLenum, modeAlpha: GLenum) { fail!("`BlendEquationSeparate` was not loaded") }
    pub extern "system" fn BlendEquationSeparatei(buf: GLuint, modeRGB: GLenum, modeAlpha: GLenum) { fail!("`BlendEquationSeparatei` was not loaded") }
    pub extern "system" fn BlendEquationi(buf: GLuint, mode: GLenum) { fail!("`BlendEquationi` was not loaded") }
    pub extern "system" fn BlendFunc(sfactor: GLenum, dfactor: GLenum) { fail!("`BlendFunc` was not loaded") }
    pub extern "system" fn BlendFuncSeparate(sfactorRGB: GLenum, dfactorRGB: GLenum, sfactorAlpha: GLenum, dfactorAlpha: GLenum) { fail!("`BlendFuncSeparate` was not loaded") }
    pub extern "system" fn BlendFuncSeparatei(buf: GLuint, srcRGB: GLenum, dstRGB: GLenum, srcAlpha: GLenum, dstAlpha: GLenum) { fail!("`BlendFuncSeparatei` was not loaded") }
    pub extern "system" fn BlendFunci(buf: GLuint, src: GLenum, dst: GLenum) { fail!("`BlendFunci` was not loaded") }
    pub extern "system" fn BlitFramebuffer(srcX0: GLint, srcY0: GLint, srcX1: GLint, srcY1: GLint, dstX0: GLint, dstY0: GLint, dstX1: GLint, dstY1: GLint, mask: GLbitfield, filter: GLenum) { fail!("`BlitFramebuffer` was not loaded") }
    pub extern "system" fn BufferData(target: GLenum, size: GLsizeiptr, data: *const c_void, usage: GLenum) { fail!("`BufferData` was not loaded") }
    pub extern "system" fn BufferSubData(target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *const c_void) { fail!("`BufferSubData` was not loaded") }
    pub extern "system" fn CheckFramebufferStatus(target: GLenum) -> GLenum { fail!("`CheckFramebufferStatus` was not loaded") }
    pub extern "system" fn ClampColor(target: GLenum, clamp: GLenum) { fail!("`ClampColor` was not loaded") }
    pub extern "system" fn Clear(mask: GLbitfield) { fail!("`Clear` was not loaded") }
    pub extern "system" fn ClearBufferData(target: GLenum, internalformat: GLenum, format: GLenum, type_: GLenum, data: *const c_void) { fail!("`ClearBufferData` was not loaded") }
    pub extern "system" fn ClearBufferSubData(target: GLenum, internalformat: GLenum, offset: GLintptr, size: GLsizeiptr, format: GLenum, type_: GLenum, data: *const c_void) { fail!("`ClearBufferSubData` was not loaded") }
    pub extern "system" fn ClearBufferfi(buffer: GLenum, drawbuffer: GLint, depth: GLfloat, stencil: GLint) { fail!("`ClearBufferfi` was not loaded") }
    pub extern "system" fn ClearBufferfv(buffer: GLenum, drawbuffer: GLint, value: *const GLfloat) { fail!("`ClearBufferfv` was not loaded") }
    pub extern "system" fn ClearBufferiv(buffer: GLenum, drawbuffer: GLint, value: *const GLint) { fail!("`ClearBufferiv` was not loaded") }
    pub extern "system" fn ClearBufferuiv(buffer: GLenum, drawbuffer: GLint, value: *const GLuint) { fail!("`ClearBufferuiv` was not loaded") }
    pub extern "system" fn ClearColor(red: GLfloat, green: GLfloat, blue: GLfloat, alpha: GLfloat) { fail!("`ClearColor` was not loaded") }
    pub extern "system" fn ClearDepth(depth: GLdouble) { fail!("`ClearDepth` was not loaded") }
    pub extern "system" fn ClearDepthf(d: GLfloat) { fail!("`ClearDepthf` was not loaded") }
    pub extern "system" fn ClearStencil(s: GLint) { fail!("`ClearStencil` was not loaded") }
    pub extern "system" fn ClientWaitSync(sync: GLsync, flags: GLbitfield, timeout: GLuint64) -> GLenum { fail!("`ClientWaitSync` was not loaded") }
    pub extern "system" fn ColorMask(red: GLboolean, green: GLboolean, blue: GLboolean, alpha: GLboolean) { fail!("`ColorMask` was not loaded") }
    pub extern "system" fn ColorMaski(index: GLuint, r: GLboolean, g: GLboolean, b: GLboolean, a: GLboolean) { fail!("`ColorMaski` was not loaded") }
    pub extern "system" fn ColorP3ui(type_: GLenum, color: GLuint) { fail!("`ColorP3ui` was not loaded") }
    pub extern "system" fn ColorP3uiv(type_: GLenum, color: *const GLuint) { fail!("`ColorP3uiv` was not loaded") }
    pub extern "system" fn ColorP4ui(type_: GLenum, color: GLuint) { fail!("`ColorP4ui` was not loaded") }
    pub extern "system" fn ColorP4uiv(type_: GLenum, color: *const GLuint) { fail!("`ColorP4uiv` was not loaded") }
    pub extern "system" fn CompileShader(shader: GLuint) { fail!("`CompileShader` was not loaded") }
    pub extern "system" fn CompressedTexImage1D(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) { fail!("`CompressedTexImage1D` was not loaded") }
    pub extern "system" fn CompressedTexImage2D(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, height: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) { fail!("`CompressedTexImage2D` was not loaded") }
    pub extern "system" fn CompressedTexImage3D(target: GLenum, level: GLint, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei, border: GLint, imageSize: GLsizei, data: *const c_void) { fail!("`CompressedTexImage3D` was not loaded") }
    pub extern "system" fn CompressedTexSubImage1D(target: GLenum, level: GLint, xoffset: GLint, width: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) { fail!("`CompressedTexSubImage1D` was not loaded") }
    pub extern "system" fn CompressedTexSubImage2D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, width: GLsizei, height: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) { fail!("`CompressedTexSubImage2D` was not loaded") }
    pub extern "system" fn CompressedTexSubImage3D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, format: GLenum, imageSize: GLsizei, data: *const c_void) { fail!("`CompressedTexSubImage3D` was not loaded") }
    pub extern "system" fn CopyBufferSubData(readTarget: GLenum, writeTarget: GLenum, readOffset: GLintptr, writeOffset: GLintptr, size: GLsizeiptr) { fail!("`CopyBufferSubData` was not loaded") }
    pub extern "system" fn CopyImageSubData(srcName: GLuint, srcTarget: GLenum, srcLevel: GLint, srcX: GLint, srcY: GLint, srcZ: GLint, dstName: GLuint, dstTarget: GLenum, dstLevel: GLint, dstX: GLint, dstY: GLint, dstZ: GLint, srcWidth: GLsizei, srcHeight: GLsizei, srcDepth: GLsizei) { fail!("`CopyImageSubData` was not loaded") }
    pub extern "system" fn CopyTexImage1D(target: GLenum, level: GLint, internalformat: GLenum, x: GLint, y: GLint, width: GLsizei, border: GLint) { fail!("`CopyTexImage1D` was not loaded") }
    pub extern "system" fn CopyTexImage2D(target: GLenum, level: GLint, internalformat: GLenum, x: GLint, y: GLint, width: GLsizei, height: GLsizei, border: GLint) { fail!("`CopyTexImage2D` was not loaded") }
    pub extern "system" fn CopyTexSubImage1D(target: GLenum, level: GLint, xoffset: GLint, x: GLint, y: GLint, width: GLsizei) { fail!("`CopyTexSubImage1D` was not loaded") }
    pub extern "system" fn CopyTexSubImage2D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, x: GLint, y: GLint, width: GLsizei, height: GLsizei) { fail!("`CopyTexSubImage2D` was not loaded") }
    pub extern "system" fn CopyTexSubImage3D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, x: GLint, y: GLint, width: GLsizei, height: GLsizei) { fail!("`CopyTexSubImage3D` was not loaded") }
    pub extern "system" fn CreateProgram() -> GLuint { fail!("`CreateProgram` was not loaded") }
    pub extern "system" fn CreateShader(type_: GLenum) -> GLuint { fail!("`CreateShader` was not loaded") }
    pub extern "system" fn CreateShaderProgramv(type_: GLenum, count: GLsizei, strings: *const *const GLchar) -> GLuint { fail!("`CreateShaderProgramv` was not loaded") }
    pub extern "system" fn CullFace(mode: GLenum) { fail!("`CullFace` was not loaded") }
    pub extern "system" fn DebugMessageCallback(callback: GLDEBUGPROC, userParam: *const c_void) { fail!("`DebugMessageCallback` was not loaded") }
    pub extern "system" fn DebugMessageControl(source: GLenum, type_: GLenum, severity: GLenum, count: GLsizei, ids: *const GLuint, enabled: GLboolean) { fail!("`DebugMessageControl` was not loaded") }
    pub extern "system" fn DebugMessageInsert(source: GLenum, type_: GLenum, id: GLuint, severity: GLenum, length: GLsizei, buf: *const GLchar) { fail!("`DebugMessageInsert` was not loaded") }
    pub extern "system" fn DeleteBuffers(n: GLsizei, buffers: *const GLuint) { fail!("`DeleteBuffers` was not loaded") }
    pub extern "system" fn DeleteFramebuffers(n: GLsizei, framebuffers: *const GLuint) { fail!("`DeleteFramebuffers` was not loaded") }
    pub extern "system" fn DeleteProgram(program: GLuint) { fail!("`DeleteProgram` was not loaded") }
    pub extern "system" fn DeleteProgramPipelines(n: GLsizei, pipelines: *const GLuint) { fail!("`DeleteProgramPipelines` was not loaded") }
    pub extern "system" fn DeleteQueries(n: GLsizei, ids: *const GLuint) { fail!("`DeleteQueries` was not loaded") }
    pub extern "system" fn DeleteRenderbuffers(n: GLsizei, renderbuffers: *const GLuint) { fail!("`DeleteRenderbuffers` was not loaded") }
    pub extern "system" fn DeleteSamplers(count: GLsizei, samplers: *const GLuint) { fail!("`DeleteSamplers` was not loaded") }
    pub extern "system" fn DeleteShader(shader: GLuint) { fail!("`DeleteShader` was not loaded") }
    pub extern "system" fn DeleteSync(sync: GLsync) { fail!("`DeleteSync` was not loaded") }
    pub extern "system" fn DeleteTextures(n: GLsizei, textures: *const GLuint) { fail!("`DeleteTextures` was not loaded") }
    pub extern "system" fn DeleteTransformFeedbacks(n: GLsizei, ids: *const GLuint) { fail!("`DeleteTransformFeedbacks` was not loaded") }
    pub extern "system" fn DeleteVertexArrays(n: GLsizei, arrays: *const GLuint) { fail!("`DeleteVertexArrays` was not loaded") }
    pub extern "system" fn DepthFunc(func: GLenum) { fail!("`DepthFunc` was not loaded") }
    pub extern "system" fn DepthMask(flag: GLboolean) { fail!("`DepthMask` was not loaded") }
    pub extern "system" fn DepthRange(near: GLdouble, far: GLdouble) { fail!("`DepthRange` was not loaded") }
    pub extern "system" fn DepthRangeArrayv(first: GLuint, count: GLsizei, v: *const GLdouble) { fail!("`DepthRangeArrayv` was not loaded") }
    pub extern "system" fn DepthRangeIndexed(index: GLuint, n: GLdouble, f: GLdouble) { fail!("`DepthRangeIndexed` was not loaded") }
    pub extern "system" fn DepthRangef(n: GLfloat, f: GLfloat) { fail!("`DepthRangef` was not loaded") }
    pub extern "system" fn DetachShader(program: GLuint, shader: GLuint) { fail!("`DetachShader` was not loaded") }
    pub extern "system" fn Disable(cap: GLenum) { fail!("`Disable` was not loaded") }
    pub extern "system" fn DisableVertexAttribArray(index: GLuint) { fail!("`DisableVertexAttribArray` was not loaded") }
    pub extern "system" fn Disablei(target: GLenum, index: GLuint) { fail!("`Disablei` was not loaded") }
    pub extern "system" fn DispatchCompute(num_groups_x: GLuint, num_groups_y: GLuint, num_groups_z: GLuint) { fail!("`DispatchCompute` was not loaded") }
    pub extern "system" fn DispatchComputeIndirect(indirect: GLintptr) { fail!("`DispatchComputeIndirect` was not loaded") }
    pub extern "system" fn DrawArrays(mode: GLenum, first: GLint, count: GLsizei) { fail!("`DrawArrays` was not loaded") }
    pub extern "system" fn DrawArraysIndirect(mode: GLenum, indirect: *const c_void) { fail!("`DrawArraysIndirect` was not loaded") }
    pub extern "system" fn DrawArraysInstanced(mode: GLenum, first: GLint, count: GLsizei, instancecount: GLsizei) { fail!("`DrawArraysInstanced` was not loaded") }
    pub extern "system" fn DrawArraysInstancedBaseInstance(mode: GLenum, first: GLint, count: GLsizei, instancecount: GLsizei, baseinstance: GLuint) { fail!("`DrawArraysInstancedBaseInstance` was not loaded") }
    pub extern "system" fn DrawBuffer(mode: GLenum) { fail!("`DrawBuffer` was not loaded") }
    pub extern "system" fn DrawBuffers(n: GLsizei, bufs: *const GLenum) { fail!("`DrawBuffers` was not loaded") }
    pub extern "system" fn DrawElements(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void) { fail!("`DrawElements` was not loaded") }
    pub extern "system" fn DrawElementsBaseVertex(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, basevertex: GLint) { fail!("`DrawElementsBaseVertex` was not loaded") }
    pub extern "system" fn DrawElementsIndirect(mode: GLenum, type_: GLenum, indirect: *const c_void) { fail!("`DrawElementsIndirect` was not loaded") }
    pub extern "system" fn DrawElementsInstanced(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei) { fail!("`DrawElementsInstanced` was not loaded") }
    pub extern "system" fn DrawElementsInstancedBaseInstance(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, baseinstance: GLuint) { fail!("`DrawElementsInstancedBaseInstance` was not loaded") }
    pub extern "system" fn DrawElementsInstancedBaseVertex(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, basevertex: GLint) { fail!("`DrawElementsInstancedBaseVertex` was not loaded") }
    pub extern "system" fn DrawElementsInstancedBaseVertexBaseInstance(mode: GLenum, count: GLsizei, type_: GLenum, indices: *const c_void, instancecount: GLsizei, basevertex: GLint, baseinstance: GLuint) { fail!("`DrawElementsInstancedBaseVertexBaseInstance` was not loaded") }
    pub extern "system" fn DrawRangeElements(mode: GLenum, start: GLuint, end: GLuint, count: GLsizei, type_: GLenum, indices: *const c_void) { fail!("`DrawRangeElements` was not loaded") }
    pub extern "system" fn DrawRangeElementsBaseVertex(mode: GLenum, start: GLuint, end: GLuint, count: GLsizei, type_: GLenum, indices: *const c_void, basevertex: GLint) { fail!("`DrawRangeElementsBaseVertex` was not loaded") }
    pub extern "system" fn DrawTransformFeedback(mode: GLenum, id: GLuint) { fail!("`DrawTransformFeedback` was not loaded") }
    pub extern "system" fn DrawTransformFeedbackInstanced(mode: GLenum, id: GLuint, instancecount: GLsizei) { fail!("`DrawTransformFeedbackInstanced` was not loaded") }
    pub extern "system" fn DrawTransformFeedbackStream(mode: GLenum, id: GLuint, stream: GLuint) { fail!("`DrawTransformFeedbackStream` was not loaded") }
    pub extern "system" fn DrawTransformFeedbackStreamInstanced(mode: GLenum, id: GLuint, stream: GLuint, instancecount: GLsizei) { fail!("`DrawTransformFeedbackStreamInstanced` was not loaded") }
    pub extern "system" fn Enable(cap: GLenum) { fail!("`Enable` was not loaded") }
    pub extern "system" fn EnableVertexAttribArray(index: GLuint) { fail!("`EnableVertexAttribArray` was not loaded") }
    pub extern "system" fn Enablei(target: GLenum, index: GLuint) { fail!("`Enablei` was not loaded") }
    pub extern "system" fn EndConditionalRender() { fail!("`EndConditionalRender` was not loaded") }
    pub extern "system" fn EndQuery(target: GLenum) { fail!("`EndQuery` was not loaded") }
    pub extern "system" fn EndQueryIndexed(target: GLenum, index: GLuint) { fail!("`EndQueryIndexed` was not loaded") }
    pub extern "system" fn EndTransformFeedback() { fail!("`EndTransformFeedback` was not loaded") }
    pub extern "system" fn FenceSync(condition: GLenum, flags: GLbitfield) -> GLsync { fail!("`FenceSync` was not loaded") }
    pub extern "system" fn Finish() { fail!("`Finish` was not loaded") }
    pub extern "system" fn Flush() { fail!("`Flush` was not loaded") }
    pub extern "system" fn FlushMappedBufferRange(target: GLenum, offset: GLintptr, length: GLsizeiptr) { fail!("`FlushMappedBufferRange` was not loaded") }
    pub extern "system" fn FramebufferParameteri(target: GLenum, pname: GLenum, param: GLint) { fail!("`FramebufferParameteri` was not loaded") }
    pub extern "system" fn FramebufferRenderbuffer(target: GLenum, attachment: GLenum, renderbuffertarget: GLenum, renderbuffer: GLuint) { fail!("`FramebufferRenderbuffer` was not loaded") }
    pub extern "system" fn FramebufferTexture(target: GLenum, attachment: GLenum, texture: GLuint, level: GLint) { fail!("`FramebufferTexture` was not loaded") }
    pub extern "system" fn FramebufferTexture1D(target: GLenum, attachment: GLenum, textarget: GLenum, texture: GLuint, level: GLint) { fail!("`FramebufferTexture1D` was not loaded") }
    pub extern "system" fn FramebufferTexture2D(target: GLenum, attachment: GLenum, textarget: GLenum, texture: GLuint, level: GLint) { fail!("`FramebufferTexture2D` was not loaded") }
    pub extern "system" fn FramebufferTexture3D(target: GLenum, attachment: GLenum, textarget: GLenum, texture: GLuint, level: GLint, zoffset: GLint) { fail!("`FramebufferTexture3D` was not loaded") }
    pub extern "system" fn FramebufferTextureLayer(target: GLenum, attachment: GLenum, texture: GLuint, level: GLint, layer: GLint) { fail!("`FramebufferTextureLayer` was not loaded") }
    pub extern "system" fn FrontFace(mode: GLenum) { fail!("`FrontFace` was not loaded") }
    pub extern "system" fn GenBuffers(n: GLsizei, buffers: *mut GLuint) { fail!("`GenBuffers` was not loaded") }
    pub extern "system" fn GenFramebuffers(n: GLsizei, framebuffers: *mut GLuint) { fail!("`GenFramebuffers` was not loaded") }
    pub extern "system" fn GenProgramPipelines(n: GLsizei, pipelines: *mut GLuint) { fail!("`GenProgramPipelines` was not loaded") }
    pub extern "system" fn GenQueries(n: GLsizei, ids: *mut GLuint) { fail!("`GenQueries` was not loaded") }
    pub extern "system" fn GenRenderbuffers(n: GLsizei, renderbuffers: *mut GLuint) { fail!("`GenRenderbuffers` was not loaded") }
    pub extern "system" fn GenSamplers(count: GLsizei, samplers: *mut GLuint) { fail!("`GenSamplers` was not loaded") }
    pub extern "system" fn GenTextures(n: GLsizei, textures: *mut GLuint) { fail!("`GenTextures` was not loaded") }
    pub extern "system" fn GenTransformFeedbacks(n: GLsizei, ids: *mut GLuint) { fail!("`GenTransformFeedbacks` was not loaded") }
    pub extern "system" fn GenVertexArrays(n: GLsizei, arrays: *mut GLuint) { fail!("`GenVertexArrays` was not loaded") }
    pub extern "system" fn GenerateMipmap(target: GLenum) { fail!("`GenerateMipmap` was not loaded") }
    pub extern "system" fn GetActiveAtomicCounterBufferiv(program: GLuint, bufferIndex: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetActiveAtomicCounterBufferiv` was not loaded") }
    pub extern "system" fn GetActiveAttrib(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLint, type_: *mut GLenum, name: *mut GLchar) { fail!("`GetActiveAttrib` was not loaded") }
    pub extern "system" fn GetActiveSubroutineName(program: GLuint, shadertype: GLenum, index: GLuint, bufsize: GLsizei, length: *mut GLsizei, name: *mut GLchar) { fail!("`GetActiveSubroutineName` was not loaded") }
    pub extern "system" fn GetActiveSubroutineUniformName(program: GLuint, shadertype: GLenum, index: GLuint, bufsize: GLsizei, length: *mut GLsizei, name: *mut GLchar) { fail!("`GetActiveSubroutineUniformName` was not loaded") }
    pub extern "system" fn GetActiveSubroutineUniformiv(program: GLuint, shadertype: GLenum, index: GLuint, pname: GLenum, values: *mut GLint) { fail!("`GetActiveSubroutineUniformiv` was not loaded") }
    pub extern "system" fn GetActiveUniform(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLint, type_: *mut GLenum, name: *mut GLchar) { fail!("`GetActiveUniform` was not loaded") }
    pub extern "system" fn GetActiveUniformBlockName(program: GLuint, uniformBlockIndex: GLuint, bufSize: GLsizei, length: *mut GLsizei, uniformBlockName: *mut GLchar) { fail!("`GetActiveUniformBlockName` was not loaded") }
    pub extern "system" fn GetActiveUniformBlockiv(program: GLuint, uniformBlockIndex: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetActiveUniformBlockiv` was not loaded") }
    pub extern "system" fn GetActiveUniformName(program: GLuint, uniformIndex: GLuint, bufSize: GLsizei, length: *mut GLsizei, uniformName: *mut GLchar) { fail!("`GetActiveUniformName` was not loaded") }
    pub extern "system" fn GetActiveUniformsiv(program: GLuint, uniformCount: GLsizei, uniformIndices: *const GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetActiveUniformsiv` was not loaded") }
    pub extern "system" fn GetAttachedShaders(program: GLuint, maxCount: GLsizei, count: *mut GLsizei, shaders: *mut GLuint) { fail!("`GetAttachedShaders` was not loaded") }
    pub extern "system" fn GetAttribLocation(program: GLuint, name: *const GLchar) -> GLint { fail!("`GetAttribLocation` was not loaded") }
    pub extern "system" fn GetBooleani_v(target: GLenum, index: GLuint, data: *mut GLboolean) { fail!("`GetBooleani_v` was not loaded") }
    pub extern "system" fn GetBooleanv(pname: GLenum, data: *mut GLboolean) { fail!("`GetBooleanv` was not loaded") }
    pub extern "system" fn GetBufferParameteri64v(target: GLenum, pname: GLenum, params: *mut GLint64) { fail!("`GetBufferParameteri64v` was not loaded") }
    pub extern "system" fn GetBufferParameteriv(target: GLenum, pname: GLenum, params: *mut GLint) { fail!("`GetBufferParameteriv` was not loaded") }
    pub extern "system" fn GetBufferPointerv(target: GLenum, pname: GLenum, params: *const *mut c_void) { fail!("`GetBufferPointerv` was not loaded") }
    pub extern "system" fn GetBufferSubData(target: GLenum, offset: GLintptr, size: GLsizeiptr, data: *mut c_void) { fail!("`GetBufferSubData` was not loaded") }
    pub extern "system" fn GetCompressedTexImage(target: GLenum, level: GLint, img: *mut c_void) { fail!("`GetCompressedTexImage` was not loaded") }
    pub extern "system" fn GetDebugMessageLog(count: GLuint, bufSize: GLsizei, sources: *mut GLenum, types: *mut GLenum, ids: *mut GLuint, severities: *mut GLenum, lengths: *mut GLsizei, messageLog: *mut GLchar) -> GLuint { fail!("`GetDebugMessageLog` was not loaded") }
    pub extern "system" fn GetDoublei_v(target: GLenum, index: GLuint, data: *mut GLdouble) { fail!("`GetDoublei_v` was not loaded") }
    pub extern "system" fn GetDoublev(pname: GLenum, data: *mut GLdouble) { fail!("`GetDoublev` was not loaded") }
    pub extern "system" fn GetError() -> GLenum { fail!("`GetError` was not loaded") }
    pub extern "system" fn GetFloati_v(target: GLenum, index: GLuint, data: *mut GLfloat) { fail!("`GetFloati_v` was not loaded") }
    pub extern "system" fn GetFloatv(pname: GLenum, data: *mut GLfloat) { fail!("`GetFloatv` was not loaded") }
    pub extern "system" fn GetFragDataIndex(program: GLuint, name: *const GLchar) -> GLint { fail!("`GetFragDataIndex` was not loaded") }
    pub extern "system" fn GetFragDataLocation(program: GLuint, name: *const GLchar) -> GLint { fail!("`GetFragDataLocation` was not loaded") }
    pub extern "system" fn GetFramebufferAttachmentParameteriv(target: GLenum, attachment: GLenum, pname: GLenum, params: *mut GLint) { fail!("`GetFramebufferAttachmentParameteriv` was not loaded") }
    pub extern "system" fn GetFramebufferParameteriv(target: GLenum, pname: GLenum, params: *mut GLint) { fail!("`GetFramebufferParameteriv` was not loaded") }
    pub extern "system" fn GetInteger64i_v(target: GLenum, index: GLuint, data: *mut GLint64) { fail!("`GetInteger64i_v` was not loaded") }
    pub extern "system" fn GetInteger64v(pname: GLenum, data: *mut GLint64) { fail!("`GetInteger64v` was not loaded") }
    pub extern "system" fn GetIntegeri_v(target: GLenum, index: GLuint, data: *mut GLint) { fail!("`GetIntegeri_v` was not loaded") }
    pub extern "system" fn GetIntegerv(pname: GLenum, data: *mut GLint) { fail!("`GetIntegerv` was not loaded") }
    pub extern "system" fn GetInternalformati64v(target: GLenum, internalformat: GLenum, pname: GLenum, bufSize: GLsizei, params: *mut GLint64) { fail!("`GetInternalformati64v` was not loaded") }
    pub extern "system" fn GetInternalformativ(target: GLenum, internalformat: GLenum, pname: GLenum, bufSize: GLsizei, params: *mut GLint) { fail!("`GetInternalformativ` was not loaded") }
    pub extern "system" fn GetMultisamplefv(pname: GLenum, index: GLuint, val: *mut GLfloat) { fail!("`GetMultisamplefv` was not loaded") }
    pub extern "system" fn GetObjectLabel(identifier: GLenum, name: GLuint, bufSize: GLsizei, length: *mut GLsizei, label: *mut GLchar) { fail!("`GetObjectLabel` was not loaded") }
    pub extern "system" fn GetObjectPtrLabel(ptr: *const c_void, bufSize: GLsizei, length: *mut GLsizei, label: *mut GLchar) { fail!("`GetObjectPtrLabel` was not loaded") }
    pub extern "system" fn GetProgramBinary(program: GLuint, bufSize: GLsizei, length: *mut GLsizei, binaryFormat: *mut GLenum, binary: *mut c_void) { fail!("`GetProgramBinary` was not loaded") }
    pub extern "system" fn GetProgramInfoLog(program: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) { fail!("`GetProgramInfoLog` was not loaded") }
    pub extern "system" fn GetProgramInterfaceiv(program: GLuint, programInterface: GLenum, pname: GLenum, params: *mut GLint) { fail!("`GetProgramInterfaceiv` was not loaded") }
    pub extern "system" fn GetProgramPipelineInfoLog(pipeline: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) { fail!("`GetProgramPipelineInfoLog` was not loaded") }
    pub extern "system" fn GetProgramPipelineiv(pipeline: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetProgramPipelineiv` was not loaded") }
    pub extern "system" fn GetProgramResourceIndex(program: GLuint, programInterface: GLenum, name: *const GLchar) -> GLuint { fail!("`GetProgramResourceIndex` was not loaded") }
    pub extern "system" fn GetProgramResourceLocation(program: GLuint, programInterface: GLenum, name: *const GLchar) -> GLint { fail!("`GetProgramResourceLocation` was not loaded") }
    pub extern "system" fn GetProgramResourceLocationIndex(program: GLuint, programInterface: GLenum, name: *const GLchar) -> GLint { fail!("`GetProgramResourceLocationIndex` was not loaded") }
    pub extern "system" fn GetProgramResourceName(program: GLuint, programInterface: GLenum, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, name: *mut GLchar) { fail!("`GetProgramResourceName` was not loaded") }
    pub extern "system" fn GetProgramResourceiv(program: GLuint, programInterface: GLenum, index: GLuint, propCount: GLsizei, props: *const GLenum, bufSize: GLsizei, length: *mut GLsizei, params: *mut GLint) { fail!("`GetProgramResourceiv` was not loaded") }
    pub extern "system" fn GetProgramStageiv(program: GLuint, shadertype: GLenum, pname: GLenum, values: *mut GLint) { fail!("`GetProgramStageiv` was not loaded") }
    pub extern "system" fn GetProgramiv(program: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetProgramiv` was not loaded") }
    pub extern "system" fn GetQueryIndexediv(target: GLenum, index: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetQueryIndexediv` was not loaded") }
    pub extern "system" fn GetQueryObjecti64v(id: GLuint, pname: GLenum, params: *mut GLint64) { fail!("`GetQueryObjecti64v` was not loaded") }
    pub extern "system" fn GetQueryObjectiv(id: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetQueryObjectiv` was not loaded") }
    pub extern "system" fn GetQueryObjectui64v(id: GLuint, pname: GLenum, params: *mut GLuint64) { fail!("`GetQueryObjectui64v` was not loaded") }
    pub extern "system" fn GetQueryObjectuiv(id: GLuint, pname: GLenum, params: *mut GLuint) { fail!("`GetQueryObjectuiv` was not loaded") }
    pub extern "system" fn GetQueryiv(target: GLenum, pname: GLenum, params: *mut GLint) { fail!("`GetQueryiv` was not loaded") }
    pub extern "system" fn GetRenderbufferParameteriv(target: GLenum, pname: GLenum, params: *mut GLint) { fail!("`GetRenderbufferParameteriv` was not loaded") }
    pub extern "system" fn GetSamplerParameterIiv(sampler: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetSamplerParameterIiv` was not loaded") }
    pub extern "system" fn GetSamplerParameterIuiv(sampler: GLuint, pname: GLenum, params: *mut GLuint) { fail!("`GetSamplerParameterIuiv` was not loaded") }
    pub extern "system" fn GetSamplerParameterfv(sampler: GLuint, pname: GLenum, params: *mut GLfloat) { fail!("`GetSamplerParameterfv` was not loaded") }
    pub extern "system" fn GetSamplerParameteriv(sampler: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetSamplerParameteriv` was not loaded") }
    pub extern "system" fn GetShaderInfoLog(shader: GLuint, bufSize: GLsizei, length: *mut GLsizei, infoLog: *mut GLchar) { fail!("`GetShaderInfoLog` was not loaded") }
    pub extern "system" fn GetShaderPrecisionFormat(shadertype: GLenum, precisiontype: GLenum, range: *mut GLint, precision: *mut GLint) { fail!("`GetShaderPrecisionFormat` was not loaded") }
    pub extern "system" fn GetShaderSource(shader: GLuint, bufSize: GLsizei, length: *mut GLsizei, source: *mut GLchar) { fail!("`GetShaderSource` was not loaded") }
    pub extern "system" fn GetShaderiv(shader: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetShaderiv` was not loaded") }
    pub extern "system" fn GetString(name: GLenum) -> *const GLubyte { fail!("`GetString` was not loaded") }
    pub extern "system" fn GetStringi(name: GLenum, index: GLuint) -> *const GLubyte { fail!("`GetStringi` was not loaded") }
    pub extern "system" fn GetSubroutineIndex(program: GLuint, shadertype: GLenum, name: *const GLchar) -> GLuint { fail!("`GetSubroutineIndex` was not loaded") }
    pub extern "system" fn GetSubroutineUniformLocation(program: GLuint, shadertype: GLenum, name: *const GLchar) -> GLint { fail!("`GetSubroutineUniformLocation` was not loaded") }
    pub extern "system" fn GetSynciv(sync: GLsync, pname: GLenum, bufSize: GLsizei, length: *mut GLsizei, values: *mut GLint) { fail!("`GetSynciv` was not loaded") }
    pub extern "system" fn GetTexImage(target: GLenum, level: GLint, format: GLenum, type_: GLenum, pixels: *mut c_void) { fail!("`GetTexImage` was not loaded") }
    pub extern "system" fn GetTexLevelParameterfv(target: GLenum, level: GLint, pname: GLenum, params: *mut GLfloat) { fail!("`GetTexLevelParameterfv` was not loaded") }
    pub extern "system" fn GetTexLevelParameteriv(target: GLenum, level: GLint, pname: GLenum, params: *mut GLint) { fail!("`GetTexLevelParameteriv` was not loaded") }
    pub extern "system" fn GetTexParameterIiv(target: GLenum, pname: GLenum, params: *mut GLint) { fail!("`GetTexParameterIiv` was not loaded") }
    pub extern "system" fn GetTexParameterIuiv(target: GLenum, pname: GLenum, params: *mut GLuint) { fail!("`GetTexParameterIuiv` was not loaded") }
    pub extern "system" fn GetTexParameterfv(target: GLenum, pname: GLenum, params: *mut GLfloat) { fail!("`GetTexParameterfv` was not loaded") }
    pub extern "system" fn GetTexParameteriv(target: GLenum, pname: GLenum, params: *mut GLint) { fail!("`GetTexParameteriv` was not loaded") }
    pub extern "system" fn GetTransformFeedbackVarying(program: GLuint, index: GLuint, bufSize: GLsizei, length: *mut GLsizei, size: *mut GLsizei, type_: *mut GLenum, name: *mut GLchar) { fail!("`GetTransformFeedbackVarying` was not loaded") }
    pub extern "system" fn GetUniformBlockIndex(program: GLuint, uniformBlockName: *const GLchar) -> GLuint { fail!("`GetUniformBlockIndex` was not loaded") }
    pub extern "system" fn GetUniformIndices(program: GLuint, uniformCount: GLsizei, uniformNames: *const *const GLchar, uniformIndices: *mut GLuint) { fail!("`GetUniformIndices` was not loaded") }
    pub extern "system" fn GetUniformLocation(program: GLuint, name: *const GLchar) -> GLint { fail!("`GetUniformLocation` was not loaded") }
    pub extern "system" fn GetUniformSubroutineuiv(shadertype: GLenum, location: GLint, params: *mut GLuint) { fail!("`GetUniformSubroutineuiv` was not loaded") }
    pub extern "system" fn GetUniformdv(program: GLuint, location: GLint, params: *mut GLdouble) { fail!("`GetUniformdv` was not loaded") }
    pub extern "system" fn GetUniformfv(program: GLuint, location: GLint, params: *mut GLfloat) { fail!("`GetUniformfv` was not loaded") }
    pub extern "system" fn GetUniformiv(program: GLuint, location: GLint, params: *mut GLint) { fail!("`GetUniformiv` was not loaded") }
    pub extern "system" fn GetUniformuiv(program: GLuint, location: GLint, params: *mut GLuint) { fail!("`GetUniformuiv` was not loaded") }
    pub extern "system" fn GetVertexAttribIiv(index: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetVertexAttribIiv` was not loaded") }
    pub extern "system" fn GetVertexAttribIuiv(index: GLuint, pname: GLenum, params: *mut GLuint) { fail!("`GetVertexAttribIuiv` was not loaded") }
    pub extern "system" fn GetVertexAttribLdv(index: GLuint, pname: GLenum, params: *mut GLdouble) { fail!("`GetVertexAttribLdv` was not loaded") }
    pub extern "system" fn GetVertexAttribPointerv(index: GLuint, pname: GLenum, pointer: *const *mut c_void) { fail!("`GetVertexAttribPointerv` was not loaded") }
    pub extern "system" fn GetVertexAttribdv(index: GLuint, pname: GLenum, params: *mut GLdouble) { fail!("`GetVertexAttribdv` was not loaded") }
    pub extern "system" fn GetVertexAttribfv(index: GLuint, pname: GLenum, params: *mut GLfloat) { fail!("`GetVertexAttribfv` was not loaded") }
    pub extern "system" fn GetVertexAttribiv(index: GLuint, pname: GLenum, params: *mut GLint) { fail!("`GetVertexAttribiv` was not loaded") }
    pub extern "system" fn Hint(target: GLenum, mode: GLenum) { fail!("`Hint` was not loaded") }
    pub extern "system" fn InvalidateBufferData(buffer: GLuint) { fail!("`InvalidateBufferData` was not loaded") }
    pub extern "system" fn InvalidateBufferSubData(buffer: GLuint, offset: GLintptr, length: GLsizeiptr) { fail!("`InvalidateBufferSubData` was not loaded") }
    pub extern "system" fn InvalidateFramebuffer(target: GLenum, numAttachments: GLsizei, attachments: *const GLenum) { fail!("`InvalidateFramebuffer` was not loaded") }
    pub extern "system" fn InvalidateSubFramebuffer(target: GLenum, numAttachments: GLsizei, attachments: *const GLenum, x: GLint, y: GLint, width: GLsizei, height: GLsizei) { fail!("`InvalidateSubFramebuffer` was not loaded") }
    pub extern "system" fn InvalidateTexImage(texture: GLuint, level: GLint) { fail!("`InvalidateTexImage` was not loaded") }
    pub extern "system" fn InvalidateTexSubImage(texture: GLuint, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, width: GLsizei, height: GLsizei, depth: GLsizei) { fail!("`InvalidateTexSubImage` was not loaded") }
    pub extern "system" fn IsBuffer(buffer: GLuint) -> GLboolean { fail!("`IsBuffer` was not loaded") }
    pub extern "system" fn IsEnabled(cap: GLenum) -> GLboolean { fail!("`IsEnabled` was not loaded") }
    pub extern "system" fn IsEnabledi(target: GLenum, index: GLuint) -> GLboolean { fail!("`IsEnabledi` was not loaded") }
    pub extern "system" fn IsFramebuffer(framebuffer: GLuint) -> GLboolean { fail!("`IsFramebuffer` was not loaded") }
    pub extern "system" fn IsProgram(program: GLuint) -> GLboolean { fail!("`IsProgram` was not loaded") }
    pub extern "system" fn IsProgramPipeline(pipeline: GLuint) -> GLboolean { fail!("`IsProgramPipeline` was not loaded") }
    pub extern "system" fn IsQuery(id: GLuint) -> GLboolean { fail!("`IsQuery` was not loaded") }
    pub extern "system" fn IsRenderbuffer(renderbuffer: GLuint) -> GLboolean { fail!("`IsRenderbuffer` was not loaded") }
    pub extern "system" fn IsSampler(sampler: GLuint) -> GLboolean { fail!("`IsSampler` was not loaded") }
    pub extern "system" fn IsShader(shader: GLuint) -> GLboolean { fail!("`IsShader` was not loaded") }
    pub extern "system" fn IsSync(sync: GLsync) -> GLboolean { fail!("`IsSync` was not loaded") }
    pub extern "system" fn IsTexture(texture: GLuint) -> GLboolean { fail!("`IsTexture` was not loaded") }
    pub extern "system" fn IsTransformFeedback(id: GLuint) -> GLboolean { fail!("`IsTransformFeedback` was not loaded") }
    pub extern "system" fn IsVertexArray(array: GLuint) -> GLboolean { fail!("`IsVertexArray` was not loaded") }
    pub extern "system" fn LineWidth(width: GLfloat) { fail!("`LineWidth` was not loaded") }
    pub extern "system" fn LinkProgram(program: GLuint) { fail!("`LinkProgram` was not loaded") }
    pub extern "system" fn LogicOp(opcode: GLenum) { fail!("`LogicOp` was not loaded") }
    pub extern "system" fn MapBuffer(target: GLenum, access: GLenum) -> *const c_void { fail!("`MapBuffer` was not loaded") }
    pub extern "system" fn MapBufferRange(target: GLenum, offset: GLintptr, length: GLsizeiptr, access: GLbitfield) -> *const c_void { fail!("`MapBufferRange` was not loaded") }
    pub extern "system" fn MemoryBarrier(barriers: GLbitfield) { fail!("`MemoryBarrier` was not loaded") }
    pub extern "system" fn MinSampleShading(value: GLfloat) { fail!("`MinSampleShading` was not loaded") }
    pub extern "system" fn MultiDrawArrays(mode: GLenum, first: *const GLint, count: *const GLsizei, drawcount: GLsizei) { fail!("`MultiDrawArrays` was not loaded") }
    pub extern "system" fn MultiDrawArraysIndirect(mode: GLenum, indirect: *const c_void, drawcount: GLsizei, stride: GLsizei) { fail!("`MultiDrawArraysIndirect` was not loaded") }
    pub extern "system" fn MultiDrawElements(mode: GLenum, count: *const GLsizei, type_: GLenum, indices: *const *const c_void, drawcount: GLsizei) { fail!("`MultiDrawElements` was not loaded") }
    pub extern "system" fn MultiDrawElementsBaseVertex(mode: GLenum, count: *const GLsizei, type_: GLenum, indices: *const *const c_void, drawcount: GLsizei, basevertex: *const GLint) { fail!("`MultiDrawElementsBaseVertex` was not loaded") }
    pub extern "system" fn MultiDrawElementsIndirect(mode: GLenum, type_: GLenum, indirect: *const c_void, drawcount: GLsizei, stride: GLsizei) { fail!("`MultiDrawElementsIndirect` was not loaded") }
    pub extern "system" fn MultiTexCoordP1ui(texture: GLenum, type_: GLenum, coords: GLuint) { fail!("`MultiTexCoordP1ui` was not loaded") }
    pub extern "system" fn MultiTexCoordP1uiv(texture: GLenum, type_: GLenum, coords: *const GLuint) { fail!("`MultiTexCoordP1uiv` was not loaded") }
    pub extern "system" fn MultiTexCoordP2ui(texture: GLenum, type_: GLenum, coords: GLuint) { fail!("`MultiTexCoordP2ui` was not loaded") }
    pub extern "system" fn MultiTexCoordP2uiv(texture: GLenum, type_: GLenum, coords: *const GLuint) { fail!("`MultiTexCoordP2uiv` was not loaded") }
    pub extern "system" fn MultiTexCoordP3ui(texture: GLenum, type_: GLenum, coords: GLuint) { fail!("`MultiTexCoordP3ui` was not loaded") }
    pub extern "system" fn MultiTexCoordP3uiv(texture: GLenum, type_: GLenum, coords: *const GLuint) { fail!("`MultiTexCoordP3uiv` was not loaded") }
    pub extern "system" fn MultiTexCoordP4ui(texture: GLenum, type_: GLenum, coords: GLuint) { fail!("`MultiTexCoordP4ui` was not loaded") }
    pub extern "system" fn MultiTexCoordP4uiv(texture: GLenum, type_: GLenum, coords: *const GLuint) { fail!("`MultiTexCoordP4uiv` was not loaded") }
    pub extern "system" fn NormalP3ui(type_: GLenum, coords: GLuint) { fail!("`NormalP3ui` was not loaded") }
    pub extern "system" fn NormalP3uiv(type_: GLenum, coords: *const GLuint) { fail!("`NormalP3uiv` was not loaded") }
    pub extern "system" fn ObjectLabel(identifier: GLenum, name: GLuint, length: GLsizei, label: *const GLchar) { fail!("`ObjectLabel` was not loaded") }
    pub extern "system" fn ObjectPtrLabel(ptr: *const c_void, length: GLsizei, label: *const GLchar) { fail!("`ObjectPtrLabel` was not loaded") }
    pub extern "system" fn PatchParameterfv(pname: GLenum, values: *const GLfloat) { fail!("`PatchParameterfv` was not loaded") }
    pub extern "system" fn PatchParameteri(pname: GLenum, value: GLint) { fail!("`PatchParameteri` was not loaded") }
    pub extern "system" fn PauseTransformFeedback() { fail!("`PauseTransformFeedback` was not loaded") }
    pub extern "system" fn PixelStoref(pname: GLenum, param: GLfloat) { fail!("`PixelStoref` was not loaded") }
    pub extern "system" fn PixelStorei(pname: GLenum, param: GLint) { fail!("`PixelStorei` was not loaded") }
    pub extern "system" fn PointParameterf(pname: GLenum, param: GLfloat) { fail!("`PointParameterf` was not loaded") }
    pub extern "system" fn PointParameterfv(pname: GLenum, params: *const GLfloat) { fail!("`PointParameterfv` was not loaded") }
    pub extern "system" fn PointParameteri(pname: GLenum, param: GLint) { fail!("`PointParameteri` was not loaded") }
    pub extern "system" fn PointParameteriv(pname: GLenum, params: *const GLint) { fail!("`PointParameteriv` was not loaded") }
    pub extern "system" fn PointSize(size: GLfloat) { fail!("`PointSize` was not loaded") }
    pub extern "system" fn PolygonMode(face: GLenum, mode: GLenum) { fail!("`PolygonMode` was not loaded") }
    pub extern "system" fn PolygonOffset(factor: GLfloat, units: GLfloat) { fail!("`PolygonOffset` was not loaded") }
    pub extern "system" fn PopDebugGroup() { fail!("`PopDebugGroup` was not loaded") }
    pub extern "system" fn PrimitiveRestartIndex(index: GLuint) { fail!("`PrimitiveRestartIndex` was not loaded") }
    pub extern "system" fn ProgramBinary(program: GLuint, binaryFormat: GLenum, binary: *const c_void, length: GLsizei) { fail!("`ProgramBinary` was not loaded") }
    pub extern "system" fn ProgramParameteri(program: GLuint, pname: GLenum, value: GLint) { fail!("`ProgramParameteri` was not loaded") }
    pub extern "system" fn ProgramUniform1d(program: GLuint, location: GLint, v0: GLdouble) { fail!("`ProgramUniform1d` was not loaded") }
    pub extern "system" fn ProgramUniform1dv(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) { fail!("`ProgramUniform1dv` was not loaded") }
    pub extern "system" fn ProgramUniform1f(program: GLuint, location: GLint, v0: GLfloat) { fail!("`ProgramUniform1f` was not loaded") }
    pub extern "system" fn ProgramUniform1fv(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) { fail!("`ProgramUniform1fv` was not loaded") }
    pub extern "system" fn ProgramUniform1i(program: GLuint, location: GLint, v0: GLint) { fail!("`ProgramUniform1i` was not loaded") }
    pub extern "system" fn ProgramUniform1iv(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) { fail!("`ProgramUniform1iv` was not loaded") }
    pub extern "system" fn ProgramUniform1ui(program: GLuint, location: GLint, v0: GLuint) { fail!("`ProgramUniform1ui` was not loaded") }
    pub extern "system" fn ProgramUniform1uiv(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) { fail!("`ProgramUniform1uiv` was not loaded") }
    pub extern "system" fn ProgramUniform2d(program: GLuint, location: GLint, v0: GLdouble, v1: GLdouble) { fail!("`ProgramUniform2d` was not loaded") }
    pub extern "system" fn ProgramUniform2dv(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) { fail!("`ProgramUniform2dv` was not loaded") }
    pub extern "system" fn ProgramUniform2f(program: GLuint, location: GLint, v0: GLfloat, v1: GLfloat) { fail!("`ProgramUniform2f` was not loaded") }
    pub extern "system" fn ProgramUniform2fv(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) { fail!("`ProgramUniform2fv` was not loaded") }
    pub extern "system" fn ProgramUniform2i(program: GLuint, location: GLint, v0: GLint, v1: GLint) { fail!("`ProgramUniform2i` was not loaded") }
    pub extern "system" fn ProgramUniform2iv(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) { fail!("`ProgramUniform2iv` was not loaded") }
    pub extern "system" fn ProgramUniform2ui(program: GLuint, location: GLint, v0: GLuint, v1: GLuint) { fail!("`ProgramUniform2ui` was not loaded") }
    pub extern "system" fn ProgramUniform2uiv(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) { fail!("`ProgramUniform2uiv` was not loaded") }
    pub extern "system" fn ProgramUniform3d(program: GLuint, location: GLint, v0: GLdouble, v1: GLdouble, v2: GLdouble) { fail!("`ProgramUniform3d` was not loaded") }
    pub extern "system" fn ProgramUniform3dv(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) { fail!("`ProgramUniform3dv` was not loaded") }
    pub extern "system" fn ProgramUniform3f(program: GLuint, location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat) { fail!("`ProgramUniform3f` was not loaded") }
    pub extern "system" fn ProgramUniform3fv(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) { fail!("`ProgramUniform3fv` was not loaded") }
    pub extern "system" fn ProgramUniform3i(program: GLuint, location: GLint, v0: GLint, v1: GLint, v2: GLint) { fail!("`ProgramUniform3i` was not loaded") }
    pub extern "system" fn ProgramUniform3iv(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) { fail!("`ProgramUniform3iv` was not loaded") }
    pub extern "system" fn ProgramUniform3ui(program: GLuint, location: GLint, v0: GLuint, v1: GLuint, v2: GLuint) { fail!("`ProgramUniform3ui` was not loaded") }
    pub extern "system" fn ProgramUniform3uiv(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) { fail!("`ProgramUniform3uiv` was not loaded") }
    pub extern "system" fn ProgramUniform4d(program: GLuint, location: GLint, v0: GLdouble, v1: GLdouble, v2: GLdouble, v3: GLdouble) { fail!("`ProgramUniform4d` was not loaded") }
    pub extern "system" fn ProgramUniform4dv(program: GLuint, location: GLint, count: GLsizei, value: *const GLdouble) { fail!("`ProgramUniform4dv` was not loaded") }
    pub extern "system" fn ProgramUniform4f(program: GLuint, location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat, v3: GLfloat) { fail!("`ProgramUniform4f` was not loaded") }
    pub extern "system" fn ProgramUniform4fv(program: GLuint, location: GLint, count: GLsizei, value: *const GLfloat) { fail!("`ProgramUniform4fv` was not loaded") }
    pub extern "system" fn ProgramUniform4i(program: GLuint, location: GLint, v0: GLint, v1: GLint, v2: GLint, v3: GLint) { fail!("`ProgramUniform4i` was not loaded") }
    pub extern "system" fn ProgramUniform4iv(program: GLuint, location: GLint, count: GLsizei, value: *const GLint) { fail!("`ProgramUniform4iv` was not loaded") }
    pub extern "system" fn ProgramUniform4ui(program: GLuint, location: GLint, v0: GLuint, v1: GLuint, v2: GLuint, v3: GLuint) { fail!("`ProgramUniform4ui` was not loaded") }
    pub extern "system" fn ProgramUniform4uiv(program: GLuint, location: GLint, count: GLsizei, value: *const GLuint) { fail!("`ProgramUniform4uiv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix2dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix2dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix2fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix2fv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix2x3dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix2x3dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix2x3fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix2x3fv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix2x4dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix2x4dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix2x4fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix2x4fv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix3dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix3dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix3fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix3fv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix3x2dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix3x2dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix3x2fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix3x2fv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix3x4dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix3x4dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix3x4fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix3x4fv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix4dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix4dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix4fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix4fv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix4x2dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix4x2dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix4x2fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix4x2fv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix4x3dv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`ProgramUniformMatrix4x3dv` was not loaded") }
    pub extern "system" fn ProgramUniformMatrix4x3fv(program: GLuint, location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`ProgramUniformMatrix4x3fv` was not loaded") }
    pub extern "system" fn ProvokingVertex(mode: GLenum) { fail!("`ProvokingVertex` was not loaded") }
    pub extern "system" fn PushDebugGroup(source: GLenum, id: GLuint, length: GLsizei, message: *const GLchar) { fail!("`PushDebugGroup` was not loaded") }
    pub extern "system" fn QueryCounter(id: GLuint, target: GLenum) { fail!("`QueryCounter` was not loaded") }
    pub extern "system" fn ReadBuffer(mode: GLenum) { fail!("`ReadBuffer` was not loaded") }
    pub extern "system" fn ReadPixels(x: GLint, y: GLint, width: GLsizei, height: GLsizei, format: GLenum, type_: GLenum, pixels: *mut c_void) { fail!("`ReadPixels` was not loaded") }
    pub extern "system" fn ReleaseShaderCompiler() { fail!("`ReleaseShaderCompiler` was not loaded") }
    pub extern "system" fn RenderbufferStorage(target: GLenum, internalformat: GLenum, width: GLsizei, height: GLsizei) { fail!("`RenderbufferStorage` was not loaded") }
    pub extern "system" fn RenderbufferStorageMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei) { fail!("`RenderbufferStorageMultisample` was not loaded") }
    pub extern "system" fn ResumeTransformFeedback() { fail!("`ResumeTransformFeedback` was not loaded") }
    pub extern "system" fn SampleCoverage(value: GLfloat, invert: GLboolean) { fail!("`SampleCoverage` was not loaded") }
    pub extern "system" fn SampleMaski(maskNumber: GLuint, mask: GLbitfield) { fail!("`SampleMaski` was not loaded") }
    pub extern "system" fn SamplerParameterIiv(sampler: GLuint, pname: GLenum, param: *const GLint) { fail!("`SamplerParameterIiv` was not loaded") }
    pub extern "system" fn SamplerParameterIuiv(sampler: GLuint, pname: GLenum, param: *const GLuint) { fail!("`SamplerParameterIuiv` was not loaded") }
    pub extern "system" fn SamplerParameterf(sampler: GLuint, pname: GLenum, param: GLfloat) { fail!("`SamplerParameterf` was not loaded") }
    pub extern "system" fn SamplerParameterfv(sampler: GLuint, pname: GLenum, param: *const GLfloat) { fail!("`SamplerParameterfv` was not loaded") }
    pub extern "system" fn SamplerParameteri(sampler: GLuint, pname: GLenum, param: GLint) { fail!("`SamplerParameteri` was not loaded") }
    pub extern "system" fn SamplerParameteriv(sampler: GLuint, pname: GLenum, param: *const GLint) { fail!("`SamplerParameteriv` was not loaded") }
    pub extern "system" fn Scissor(x: GLint, y: GLint, width: GLsizei, height: GLsizei) { fail!("`Scissor` was not loaded") }
    pub extern "system" fn ScissorArrayv(first: GLuint, count: GLsizei, v: *const GLint) { fail!("`ScissorArrayv` was not loaded") }
    pub extern "system" fn ScissorIndexed(index: GLuint, left: GLint, bottom: GLint, width: GLsizei, height: GLsizei) { fail!("`ScissorIndexed` was not loaded") }
    pub extern "system" fn ScissorIndexedv(index: GLuint, v: *const GLint) { fail!("`ScissorIndexedv` was not loaded") }
    pub extern "system" fn SecondaryColorP3ui(type_: GLenum, color: GLuint) { fail!("`SecondaryColorP3ui` was not loaded") }
    pub extern "system" fn SecondaryColorP3uiv(type_: GLenum, color: *const GLuint) { fail!("`SecondaryColorP3uiv` was not loaded") }
    pub extern "system" fn ShaderBinary(count: GLsizei, shaders: *const GLuint, binaryformat: GLenum, binary: *const c_void, length: GLsizei) { fail!("`ShaderBinary` was not loaded") }
    pub extern "system" fn ShaderSource(shader: GLuint, count: GLsizei, string: *const *const GLchar, length: *const GLint) { fail!("`ShaderSource` was not loaded") }
    pub extern "system" fn ShaderStorageBlockBinding(program: GLuint, storageBlockIndex: GLuint, storageBlockBinding: GLuint) { fail!("`ShaderStorageBlockBinding` was not loaded") }
    pub extern "system" fn StencilFunc(func: GLenum, ref_: GLint, mask: GLuint) { fail!("`StencilFunc` was not loaded") }
    pub extern "system" fn StencilFuncSeparate(face: GLenum, func: GLenum, ref_: GLint, mask: GLuint) { fail!("`StencilFuncSeparate` was not loaded") }
    pub extern "system" fn StencilMask(mask: GLuint) { fail!("`StencilMask` was not loaded") }
    pub extern "system" fn StencilMaskSeparate(face: GLenum, mask: GLuint) { fail!("`StencilMaskSeparate` was not loaded") }
    pub extern "system" fn StencilOp(fail: GLenum, zfail: GLenum, zpass: GLenum) { fail!("`StencilOp` was not loaded") }
    pub extern "system" fn StencilOpSeparate(face: GLenum, sfail: GLenum, dpfail: GLenum, dppass: GLenum) { fail!("`StencilOpSeparate` was not loaded") }
    pub extern "system" fn TexBuffer(target: GLenum, internalformat: GLenum, buffer: GLuint) { fail!("`TexBuffer` was not loaded") }
    pub extern "system" fn TexBufferRange(target: GLenum, internalformat: GLenum, buffer: GLuint, offset: GLintptr, size: GLsizeiptr) { fail!("`TexBufferRange` was not loaded") }
    pub extern "system" fn TexCoordP1ui(type_: GLenum, coords: GLuint) { fail!("`TexCoordP1ui` was not loaded") }
    pub extern "system" fn TexCoordP1uiv(type_: GLenum, coords: *const GLuint) { fail!("`TexCoordP1uiv` was not loaded") }
    pub extern "system" fn TexCoordP2ui(type_: GLenum, coords: GLuint) { fail!("`TexCoordP2ui` was not loaded") }
    pub extern "system" fn TexCoordP2uiv(type_: GLenum, coords: *const GLuint) { fail!("`TexCoordP2uiv` was not loaded") }
    pub extern "system" fn TexCoordP3ui(type_: GLenum, coords: GLuint) { fail!("`TexCoordP3ui` was not loaded") }
    pub extern "system" fn TexCoordP3uiv(type_: GLenum, coords: *const GLuint) { fail!("`TexCoordP3uiv` was not loaded") }
    pub extern "system" fn TexCoordP4ui(type_: GLenum, coords: GLuint) { fail!("`TexCoordP4ui` was not loaded") }
    pub extern "system" fn TexCoordP4uiv(type_: GLenum, coords: *const GLuint) { fail!("`TexCoordP4uiv` was not loaded") }
    pub extern "system" fn TexImage1D(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) { fail!("`TexImage1D` was not loaded") }
    pub extern "system" fn TexImage2D(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, height: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) { fail!("`TexImage2D` was not loaded") }
    pub extern "system" fn TexImage2DMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, fixedsamplelocations: GLboolean) { fail!("`TexImage2DMultisample` was not loaded") }
    pub extern "system" fn TexImage3D(target: GLenum, level: GLint, internalformat: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, border: GLint, format: GLenum, type_: GLenum, pixels: *const c_void) { fail!("`TexImage3D` was not loaded") }
    pub extern "system" fn TexImage3DMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei, fixedsamplelocations: GLboolean) { fail!("`TexImage3DMultisample` was not loaded") }
    pub extern "system" fn TexParameterIiv(target: GLenum, pname: GLenum, params: *const GLint) { fail!("`TexParameterIiv` was not loaded") }
    pub extern "system" fn TexParameterIuiv(target: GLenum, pname: GLenum, params: *const GLuint) { fail!("`TexParameterIuiv` was not loaded") }
    pub extern "system" fn TexParameterf(target: GLenum, pname: GLenum, param: GLfloat) { fail!("`TexParameterf` was not loaded") }
    pub extern "system" fn TexParameterfv(target: GLenum, pname: GLenum, params: *const GLfloat) { fail!("`TexParameterfv` was not loaded") }
    pub extern "system" fn TexParameteri(target: GLenum, pname: GLenum, param: GLint) { fail!("`TexParameteri` was not loaded") }
    pub extern "system" fn TexParameteriv(target: GLenum, pname: GLenum, params: *const GLint) { fail!("`TexParameteriv` was not loaded") }
    pub extern "system" fn TexStorage1D(target: GLenum, levels: GLsizei, internalformat: GLenum, width: GLsizei) { fail!("`TexStorage1D` was not loaded") }
    pub extern "system" fn TexStorage2D(target: GLenum, levels: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei) { fail!("`TexStorage2D` was not loaded") }
    pub extern "system" fn TexStorage2DMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, fixedsamplelocations: GLboolean) { fail!("`TexStorage2DMultisample` was not loaded") }
    pub extern "system" fn TexStorage3D(target: GLenum, levels: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei) { fail!("`TexStorage3D` was not loaded") }
    pub extern "system" fn TexStorage3DMultisample(target: GLenum, samples: GLsizei, internalformat: GLenum, width: GLsizei, height: GLsizei, depth: GLsizei, fixedsamplelocations: GLboolean) { fail!("`TexStorage3DMultisample` was not loaded") }
    pub extern "system" fn TexSubImage1D(target: GLenum, level: GLint, xoffset: GLint, width: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) { fail!("`TexSubImage1D` was not loaded") }
    pub extern "system" fn TexSubImage2D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, width: GLsizei, height: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) { fail!("`TexSubImage2D` was not loaded") }
    pub extern "system" fn TexSubImage3D(target: GLenum, level: GLint, xoffset: GLint, yoffset: GLint, zoffset: GLint, width: GLsizei, height: GLsizei, depth: GLsizei, format: GLenum, type_: GLenum, pixels: *const c_void) { fail!("`TexSubImage3D` was not loaded") }
    pub extern "system" fn TextureView(texture: GLuint, target: GLenum, origtexture: GLuint, internalformat: GLenum, minlevel: GLuint, numlevels: GLuint, minlayer: GLuint, numlayers: GLuint) { fail!("`TextureView` was not loaded") }
    pub extern "system" fn TransformFeedbackVaryings(program: GLuint, count: GLsizei, varyings: *const *const GLchar, bufferMode: GLenum) { fail!("`TransformFeedbackVaryings` was not loaded") }
    pub extern "system" fn Uniform1d(location: GLint, x: GLdouble) { fail!("`Uniform1d` was not loaded") }
    pub extern "system" fn Uniform1dv(location: GLint, count: GLsizei, value: *const GLdouble) { fail!("`Uniform1dv` was not loaded") }
    pub extern "system" fn Uniform1f(location: GLint, v0: GLfloat) { fail!("`Uniform1f` was not loaded") }
    pub extern "system" fn Uniform1fv(location: GLint, count: GLsizei, value: *const GLfloat) { fail!("`Uniform1fv` was not loaded") }
    pub extern "system" fn Uniform1i(location: GLint, v0: GLint) { fail!("`Uniform1i` was not loaded") }
    pub extern "system" fn Uniform1iv(location: GLint, count: GLsizei, value: *const GLint) { fail!("`Uniform1iv` was not loaded") }
    pub extern "system" fn Uniform1ui(location: GLint, v0: GLuint) { fail!("`Uniform1ui` was not loaded") }
    pub extern "system" fn Uniform1uiv(location: GLint, count: GLsizei, value: *const GLuint) { fail!("`Uniform1uiv` was not loaded") }
    pub extern "system" fn Uniform2d(location: GLint, x: GLdouble, y: GLdouble) { fail!("`Uniform2d` was not loaded") }
    pub extern "system" fn Uniform2dv(location: GLint, count: GLsizei, value: *const GLdouble) { fail!("`Uniform2dv` was not loaded") }
    pub extern "system" fn Uniform2f(location: GLint, v0: GLfloat, v1: GLfloat) { fail!("`Uniform2f` was not loaded") }
    pub extern "system" fn Uniform2fv(location: GLint, count: GLsizei, value: *const GLfloat) { fail!("`Uniform2fv` was not loaded") }
    pub extern "system" fn Uniform2i(location: GLint, v0: GLint, v1: GLint) { fail!("`Uniform2i` was not loaded") }
    pub extern "system" fn Uniform2iv(location: GLint, count: GLsizei, value: *const GLint) { fail!("`Uniform2iv` was not loaded") }
    pub extern "system" fn Uniform2ui(location: GLint, v0: GLuint, v1: GLuint) { fail!("`Uniform2ui` was not loaded") }
    pub extern "system" fn Uniform2uiv(location: GLint, count: GLsizei, value: *const GLuint) { fail!("`Uniform2uiv` was not loaded") }
    pub extern "system" fn Uniform3d(location: GLint, x: GLdouble, y: GLdouble, z: GLdouble) { fail!("`Uniform3d` was not loaded") }
    pub extern "system" fn Uniform3dv(location: GLint, count: GLsizei, value: *const GLdouble) { fail!("`Uniform3dv` was not loaded") }
    pub extern "system" fn Uniform3f(location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat) { fail!("`Uniform3f` was not loaded") }
    pub extern "system" fn Uniform3fv(location: GLint, count: GLsizei, value: *const GLfloat) { fail!("`Uniform3fv` was not loaded") }
    pub extern "system" fn Uniform3i(location: GLint, v0: GLint, v1: GLint, v2: GLint) { fail!("`Uniform3i` was not loaded") }
    pub extern "system" fn Uniform3iv(location: GLint, count: GLsizei, value: *const GLint) { fail!("`Uniform3iv` was not loaded") }
    pub extern "system" fn Uniform3ui(location: GLint, v0: GLuint, v1: GLuint, v2: GLuint) { fail!("`Uniform3ui` was not loaded") }
    pub extern "system" fn Uniform3uiv(location: GLint, count: GLsizei, value: *const GLuint) { fail!("`Uniform3uiv` was not loaded") }
    pub extern "system" fn Uniform4d(location: GLint, x: GLdouble, y: GLdouble, z: GLdouble, w: GLdouble) { fail!("`Uniform4d` was not loaded") }
    pub extern "system" fn Uniform4dv(location: GLint, count: GLsizei, value: *const GLdouble) { fail!("`Uniform4dv` was not loaded") }
    pub extern "system" fn Uniform4f(location: GLint, v0: GLfloat, v1: GLfloat, v2: GLfloat, v3: GLfloat) { fail!("`Uniform4f` was not loaded") }
    pub extern "system" fn Uniform4fv(location: GLint, count: GLsizei, value: *const GLfloat) { fail!("`Uniform4fv` was not loaded") }
    pub extern "system" fn Uniform4i(location: GLint, v0: GLint, v1: GLint, v2: GLint, v3: GLint) { fail!("`Uniform4i` was not loaded") }
    pub extern "system" fn Uniform4iv(location: GLint, count: GLsizei, value: *const GLint) { fail!("`Uniform4iv` was not loaded") }
    pub extern "system" fn Uniform4ui(location: GLint, v0: GLuint, v1: GLuint, v2: GLuint, v3: GLuint) { fail!("`Uniform4ui` was not loaded") }
    pub extern "system" fn Uniform4uiv(location: GLint, count: GLsizei, value: *const GLuint) { fail!("`Uniform4uiv` was not loaded") }
    pub extern "system" fn UniformBlockBinding(program: GLuint, uniformBlockIndex: GLuint, uniformBlockBinding: GLuint) { fail!("`UniformBlockBinding` was not loaded") }
    pub extern "system" fn UniformMatrix2dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix2dv` was not loaded") }
    pub extern "system" fn UniformMatrix2fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix2fv` was not loaded") }
    pub extern "system" fn UniformMatrix2x3dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix2x3dv` was not loaded") }
    pub extern "system" fn UniformMatrix2x3fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix2x3fv` was not loaded") }
    pub extern "system" fn UniformMatrix2x4dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix2x4dv` was not loaded") }
    pub extern "system" fn UniformMatrix2x4fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix2x4fv` was not loaded") }
    pub extern "system" fn UniformMatrix3dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix3dv` was not loaded") }
    pub extern "system" fn UniformMatrix3fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix3fv` was not loaded") }
    pub extern "system" fn UniformMatrix3x2dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix3x2dv` was not loaded") }
    pub extern "system" fn UniformMatrix3x2fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix3x2fv` was not loaded") }
    pub extern "system" fn UniformMatrix3x4dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix3x4dv` was not loaded") }
    pub extern "system" fn UniformMatrix3x4fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix3x4fv` was not loaded") }
    pub extern "system" fn UniformMatrix4dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix4dv` was not loaded") }
    pub extern "system" fn UniformMatrix4fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix4fv` was not loaded") }
    pub extern "system" fn UniformMatrix4x2dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix4x2dv` was not loaded") }
    pub extern "system" fn UniformMatrix4x2fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix4x2fv` was not loaded") }
    pub extern "system" fn UniformMatrix4x3dv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLdouble) { fail!("`UniformMatrix4x3dv` was not loaded") }
    pub extern "system" fn UniformMatrix4x3fv(location: GLint, count: GLsizei, transpose: GLboolean, value: *const GLfloat) { fail!("`UniformMatrix4x3fv` was not loaded") }
    pub extern "system" fn UniformSubroutinesuiv(shadertype: GLenum, count: GLsizei, indices: *const GLuint) { fail!("`UniformSubroutinesuiv` was not loaded") }
    pub extern "system" fn UnmapBuffer(target: GLenum) -> GLboolean { fail!("`UnmapBuffer` was not loaded") }
    pub extern "system" fn UseProgram(program: GLuint) { fail!("`UseProgram` was not loaded") }
    pub extern "system" fn UseProgramStages(pipeline: GLuint, stages: GLbitfield, program: GLuint) { fail!("`UseProgramStages` was not loaded") }
    pub extern "system" fn ValidateProgram(program: GLuint) { fail!("`ValidateProgram` was not loaded") }
    pub extern "system" fn ValidateProgramPipeline(pipeline: GLuint) { fail!("`ValidateProgramPipeline` was not loaded") }
    pub extern "system" fn VertexAttrib1d(index: GLuint, x: GLdouble) { fail!("`VertexAttrib1d` was not loaded") }
    pub extern "system" fn VertexAttrib1dv(index: GLuint, v: *const GLdouble) { fail!("`VertexAttrib1dv` was not loaded") }
    pub extern "system" fn VertexAttrib1f(index: GLuint, x: GLfloat) { fail!("`VertexAttrib1f` was not loaded") }
    pub extern "system" fn VertexAttrib1fv(index: GLuint, v: *const GLfloat) { fail!("`VertexAttrib1fv` was not loaded") }
    pub extern "system" fn VertexAttrib1s(index: GLuint, x: GLshort) { fail!("`VertexAttrib1s` was not loaded") }
    pub extern "system" fn VertexAttrib1sv(index: GLuint, v: *const GLshort) { fail!("`VertexAttrib1sv` was not loaded") }
    pub extern "system" fn VertexAttrib2d(index: GLuint, x: GLdouble, y: GLdouble) { fail!("`VertexAttrib2d` was not loaded") }
    pub extern "system" fn VertexAttrib2dv(index: GLuint, v: *const GLdouble) { fail!("`VertexAttrib2dv` was not loaded") }
    pub extern "system" fn VertexAttrib2f(index: GLuint, x: GLfloat, y: GLfloat) { fail!("`VertexAttrib2f` was not loaded") }
    pub extern "system" fn VertexAttrib2fv(index: GLuint, v: *const GLfloat) { fail!("`VertexAttrib2fv` was not loaded") }
    pub extern "system" fn VertexAttrib2s(index: GLuint, x: GLshort, y: GLshort) { fail!("`VertexAttrib2s` was not loaded") }
    pub extern "system" fn VertexAttrib2sv(index: GLuint, v: *const GLshort) { fail!("`VertexAttrib2sv` was not loaded") }
    pub extern "system" fn VertexAttrib3d(index: GLuint, x: GLdouble, y: GLdouble, z: GLdouble) { fail!("`VertexAttrib3d` was not loaded") }
    pub extern "system" fn VertexAttrib3dv(index: GLuint, v: *const GLdouble) { fail!("`VertexAttrib3dv` was not loaded") }
    pub extern "system" fn VertexAttrib3f(index: GLuint, x: GLfloat, y: GLfloat, z: GLfloat) { fail!("`VertexAttrib3f` was not loaded") }
    pub extern "system" fn VertexAttrib3fv(index: GLuint, v: *const GLfloat) { fail!("`VertexAttrib3fv` was not loaded") }
    pub extern "system" fn VertexAttrib3s(index: GLuint, x: GLshort, y: GLshort, z: GLshort) { fail!("`VertexAttrib3s` was not loaded") }
    pub extern "system" fn VertexAttrib3sv(index: GLuint, v: *const GLshort) { fail!("`VertexAttrib3sv` was not loaded") }
    pub extern "system" fn VertexAttrib4Nbv(index: GLuint, v: *const GLbyte) { fail!("`VertexAttrib4Nbv` was not loaded") }
    pub extern "system" fn VertexAttrib4Niv(index: GLuint, v: *const GLint) { fail!("`VertexAttrib4Niv` was not loaded") }
    pub extern "system" fn VertexAttrib4Nsv(index: GLuint, v: *const GLshort) { fail!("`VertexAttrib4Nsv` was not loaded") }
    pub extern "system" fn VertexAttrib4Nub(index: GLuint, x: GLubyte, y: GLubyte, z: GLubyte, w: GLubyte) { fail!("`VertexAttrib4Nub` was not loaded") }
    pub extern "system" fn VertexAttrib4Nubv(index: GLuint, v: *const GLubyte) { fail!("`VertexAttrib4Nubv` was not loaded") }
    pub extern "system" fn VertexAttrib4Nuiv(index: GLuint, v: *const GLuint) { fail!("`VertexAttrib4Nuiv` was not loaded") }
    pub extern "system" fn VertexAttrib4Nusv(index: GLuint, v: *const GLushort) { fail!("`VertexAttrib4Nusv` was not loaded") }
    pub extern "system" fn VertexAttrib4bv(index: GLuint, v: *const GLbyte) { fail!("`VertexAttrib4bv` was not loaded") }
    pub extern "system" fn VertexAttrib4d(index: GLuint, x: GLdouble, y: GLdouble, z: GLdouble, w: GLdouble) { fail!("`VertexAttrib4d` was not loaded") }
    pub extern "system" fn VertexAttrib4dv(index: GLuint, v: *const GLdouble) { fail!("`VertexAttrib4dv` was not loaded") }
    pub extern "system" fn VertexAttrib4f(index: GLuint, x: GLfloat, y: GLfloat, z: GLfloat, w: GLfloat) { fail!("`VertexAttrib4f` was not loaded") }
    pub extern "system" fn VertexAttrib4fv(index: GLuint, v: *const GLfloat) { fail!("`VertexAttrib4fv` was not loaded") }
    pub extern "system" fn VertexAttrib4iv(index: GLuint, v: *const GLint) { fail!("`VertexAttrib4iv` was not loaded") }
    pub extern "system" fn VertexAttrib4s(index: GLuint, x: GLshort, y: GLshort, z: GLshort, w: GLshort) { fail!("`VertexAttrib4s` was not loaded") }
    pub extern "system" fn VertexAttrib4sv(index: GLuint, v: *const GLshort) { fail!("`VertexAttrib4sv` was not loaded") }
    pub extern "system" fn VertexAttrib4ubv(index: GLuint, v: *const GLubyte) { fail!("`VertexAttrib4ubv` was not loaded") }
    pub extern "system" fn VertexAttrib4uiv(index: GLuint, v: *const GLuint) { fail!("`VertexAttrib4uiv` was not loaded") }
    pub extern "system" fn VertexAttrib4usv(index: GLuint, v: *const GLushort) { fail!("`VertexAttrib4usv` was not loaded") }
    pub extern "system" fn VertexAttribBinding(attribindex: GLuint, bindingindex: GLuint) { fail!("`VertexAttribBinding` was not loaded") }
    pub extern "system" fn VertexAttribDivisor(index: GLuint, divisor: GLuint) { fail!("`VertexAttribDivisor` was not loaded") }
    pub extern "system" fn VertexAttribFormat(attribindex: GLuint, size: GLint, type_: GLenum, normalized: GLboolean, relativeoffset: GLuint) { fail!("`VertexAttribFormat` was not loaded") }
    pub extern "system" fn VertexAttribI1i(index: GLuint, x: GLint) { fail!("`VertexAttribI1i` was not loaded") }
    pub extern "system" fn VertexAttribI1iv(index: GLuint, v: *const GLint) { fail!("`VertexAttribI1iv` was not loaded") }
    pub extern "system" fn VertexAttribI1ui(index: GLuint, x: GLuint) { fail!("`VertexAttribI1ui` was not loaded") }
    pub extern "system" fn VertexAttribI1uiv(index: GLuint, v: *const GLuint) { fail!("`VertexAttribI1uiv` was not loaded") }
    pub extern "system" fn VertexAttribI2i(index: GLuint, x: GLint, y: GLint) { fail!("`VertexAttribI2i` was not loaded") }
    pub extern "system" fn VertexAttribI2iv(index: GLuint, v: *const GLint) { fail!("`VertexAttribI2iv` was not loaded") }
    pub extern "system" fn VertexAttribI2ui(index: GLuint, x: GLuint, y: GLuint) { fail!("`VertexAttribI2ui` was not loaded") }
    pub extern "system" fn VertexAttribI2uiv(index: GLuint, v: *const GLuint) { fail!("`VertexAttribI2uiv` was not loaded") }
    pub extern "system" fn VertexAttribI3i(index: GLuint, x: GLint, y: GLint, z: GLint) { fail!("`VertexAttribI3i` was not loaded") }
    pub extern "system" fn VertexAttribI3iv(index: GLuint, v: *const GLint) { fail!("`VertexAttribI3iv` was not loaded") }
    pub extern "system" fn VertexAttribI3ui(index: GLuint, x: GLuint, y: GLuint, z: GLuint) { fail!("`VertexAttribI3ui` was not loaded") }
    pub extern "system" fn VertexAttribI3uiv(index: GLuint, v: *const GLuint) { fail!("`VertexAttribI3uiv` was not loaded") }
    pub extern "system" fn VertexAttribI4bv(index: GLuint, v: *const GLbyte) { fail!("`VertexAttribI4bv` was not loaded") }
    pub extern "system" fn VertexAttribI4i(index: GLuint, x: GLint, y: GLint, z: GLint, w: GLint) { fail!("`VertexAttribI4i` was not loaded") }
    pub extern "system" fn VertexAttribI4iv(index: GLuint, v: *const GLint) { fail!("`VertexAttribI4iv` was not loaded") }
    pub extern "system" fn VertexAttribI4sv(index: GLuint, v: *const GLshort) { fail!("`VertexAttribI4sv` was not loaded") }
    pub extern "system" fn VertexAttribI4ubv(index: GLuint, v: *const GLubyte) { fail!("`VertexAttribI4ubv` was not loaded") }
    pub extern "system" fn VertexAttribI4ui(index: GLuint, x: GLuint, y: GLuint, z: GLuint, w: GLuint) { fail!("`VertexAttribI4ui` was not loaded") }
    pub extern "system" fn VertexAttribI4uiv(index: GLuint, v: *const GLuint) { fail!("`VertexAttribI4uiv` was not loaded") }
    pub extern "system" fn VertexAttribI4usv(index: GLuint, v: *const GLushort) { fail!("`VertexAttribI4usv` was not loaded") }
    pub extern "system" fn VertexAttribIFormat(attribindex: GLuint, size: GLint, type_: GLenum, relativeoffset: GLuint) { fail!("`VertexAttribIFormat` was not loaded") }
    pub extern "system" fn VertexAttribIPointer(index: GLuint, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const c_void) { fail!("`VertexAttribIPointer` was not loaded") }
    pub extern "system" fn VertexAttribL1d(index: GLuint, x: GLdouble) { fail!("`VertexAttribL1d` was not loaded") }
    pub extern "system" fn VertexAttribL1dv(index: GLuint, v: *const GLdouble) { fail!("`VertexAttribL1dv` was not loaded") }
    pub extern "system" fn VertexAttribL2d(index: GLuint, x: GLdouble, y: GLdouble) { fail!("`VertexAttribL2d` was not loaded") }
    pub extern "system" fn VertexAttribL2dv(index: GLuint, v: *const GLdouble) { fail!("`VertexAttribL2dv` was not loaded") }
    pub extern "system" fn VertexAttribL3d(index: GLuint, x: GLdouble, y: GLdouble, z: GLdouble) { fail!("`VertexAttribL3d` was not loaded") }
    pub extern "system" fn VertexAttribL3dv(index: GLuint, v: *const GLdouble) { fail!("`VertexAttribL3dv` was not loaded") }
    pub extern "system" fn VertexAttribL4d(index: GLuint, x: GLdouble, y: GLdouble, z: GLdouble, w: GLdouble) { fail!("`VertexAttribL4d` was not loaded") }
    pub extern "system" fn VertexAttribL4dv(index: GLuint, v: *const GLdouble) { fail!("`VertexAttribL4dv` was not loaded") }
    pub extern "system" fn VertexAttribLFormat(attribindex: GLuint, size: GLint, type_: GLenum, relativeoffset: GLuint) { fail!("`VertexAttribLFormat` was not loaded") }
    pub extern "system" fn VertexAttribLPointer(index: GLuint, size: GLint, type_: GLenum, stride: GLsizei, pointer: *const c_void) { fail!("`VertexAttribLPointer` was not loaded") }
    pub extern "system" fn VertexAttribP1ui(index: GLuint, type_: GLenum, normalized: GLboolean, value: GLuint) { fail!("`VertexAttribP1ui` was not loaded") }
    pub extern "system" fn VertexAttribP1uiv(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) { fail!("`VertexAttribP1uiv` was not loaded") }
    pub extern "system" fn VertexAttribP2ui(index: GLuint, type_: GLenum, normalized: GLboolean, value: GLuint) { fail!("`VertexAttribP2ui` was not loaded") }
    pub extern "system" fn VertexAttribP2uiv(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) { fail!("`VertexAttribP2uiv` was not loaded") }
    pub extern "system" fn VertexAttribP3ui(index: GLuint, type_: GLenum, normalized: GLboolean, value: GLuint) { fail!("`VertexAttribP3ui` was not loaded") }
    pub extern "system" fn VertexAttribP3uiv(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) { fail!("`VertexAttribP3uiv` was not loaded") }
    pub extern "system" fn VertexAttribP4ui(index: GLuint, type_: GLenum, normalized: GLboolean, value: GLuint) { fail!("`VertexAttribP4ui` was not loaded") }
    pub extern "system" fn VertexAttribP4uiv(index: GLuint, type_: GLenum, normalized: GLboolean, value: *const GLuint) { fail!("`VertexAttribP4uiv` was not loaded") }
    pub extern "system" fn VertexAttribPointer(index: GLuint, size: GLint, type_: GLenum, normalized: GLboolean, stride: GLsizei, pointer: *const c_void) { fail!("`VertexAttribPointer` was not loaded") }
    pub extern "system" fn VertexBindingDivisor(bindingindex: GLuint, divisor: GLuint) { fail!("`VertexBindingDivisor` was not loaded") }
    pub extern "system" fn VertexP2ui(type_: GLenum, value: GLuint) { fail!("`VertexP2ui` was not loaded") }
    pub extern "system" fn VertexP2uiv(type_: GLenum, value: *const GLuint) { fail!("`VertexP2uiv` was not loaded") }
    pub extern "system" fn VertexP3ui(type_: GLenum, value: GLuint) { fail!("`VertexP3ui` was not loaded") }
    pub extern "system" fn VertexP3uiv(type_: GLenum, value: *const GLuint) { fail!("`VertexP3uiv` was not loaded") }
    pub extern "system" fn VertexP4ui(type_: GLenum, value: GLuint) { fail!("`VertexP4ui` was not loaded") }
    pub extern "system" fn VertexP4uiv(type_: GLenum, value: *const GLuint) { fail!("`VertexP4uiv` was not loaded") }
    pub extern "system" fn Viewport(x: GLint, y: GLint, width: GLsizei, height: GLsizei) { fail!("`Viewport` was not loaded") }
    pub extern "system" fn ViewportArrayv(first: GLuint, count: GLsizei, v: *const GLfloat) { fail!("`ViewportArrayv` was not loaded") }
    pub extern "system" fn ViewportIndexedf(index: GLuint, x: GLfloat, y: GLfloat, w: GLfloat, h: GLfloat) { fail!("`ViewportIndexedf` was not loaded") }
    pub extern "system" fn ViewportIndexedfv(index: GLuint, v: *const GLfloat) { fail!("`ViewportIndexedfv` was not loaded") }
    pub extern "system" fn WaitSync(sync: GLsync, flags: GLbitfield, timeout: GLuint64) { fail!("`WaitSync` was not loaded") }
}

/// Load each OpenGL symbol using a custom load function. This allows for the
/// use of functions like `glfwGetProcAddress` or `SDL_GL_GetProcAddress`.
///
/// ~~~
/// let gl = gl::load_with(glfw::get_proc_address);
/// ~~~
pub fn load_with(loadfn: |symbol: &str| -> *const libc::c_void) {
    ActiveShaderProgram::load_with(|s| loadfn(s));
    ActiveTexture::load_with(|s| loadfn(s));
    AttachShader::load_with(|s| loadfn(s));
    BeginConditionalRender::load_with(|s| loadfn(s));
    BeginQuery::load_with(|s| loadfn(s));
    BeginQueryIndexed::load_with(|s| loadfn(s));
    BeginTransformFeedback::load_with(|s| loadfn(s));
    BindAttribLocation::load_with(|s| loadfn(s));
    BindBuffer::load_with(|s| loadfn(s));
    BindBufferBase::load_with(|s| loadfn(s));
    BindBufferRange::load_with(|s| loadfn(s));
    BindFragDataLocation::load_with(|s| loadfn(s));
    BindFragDataLocationIndexed::load_with(|s| loadfn(s));
    BindFramebuffer::load_with(|s| loadfn(s));
    BindImageTexture::load_with(|s| loadfn(s));
    BindProgramPipeline::load_with(|s| loadfn(s));
    BindRenderbuffer::load_with(|s| loadfn(s));
    BindSampler::load_with(|s| loadfn(s));
    BindTexture::load_with(|s| loadfn(s));
    BindTransformFeedback::load_with(|s| loadfn(s));
    BindVertexArray::load_with(|s| loadfn(s));
    BindVertexBuffer::load_with(|s| loadfn(s));
    BlendColor::load_with(|s| loadfn(s));
    BlendEquation::load_with(|s| loadfn(s));
    BlendEquationSeparate::load_with(|s| loadfn(s));
    BlendEquationSeparatei::load_with(|s| loadfn(s));
    BlendEquationi::load_with(|s| loadfn(s));
    BlendFunc::load_with(|s| loadfn(s));
    BlendFuncSeparate::load_with(|s| loadfn(s));
    BlendFuncSeparatei::load_with(|s| loadfn(s));
    BlendFunci::load_with(|s| loadfn(s));
    BlitFramebuffer::load_with(|s| loadfn(s));
    BufferData::load_with(|s| loadfn(s));
    BufferSubData::load_with(|s| loadfn(s));
    CheckFramebufferStatus::load_with(|s| loadfn(s));
    ClampColor::load_with(|s| loadfn(s));
    Clear::load_with(|s| loadfn(s));
    ClearBufferData::load_with(|s| loadfn(s));
    ClearBufferSubData::load_with(|s| loadfn(s));
    ClearBufferfi::load_with(|s| loadfn(s));
    ClearBufferfv::load_with(|s| loadfn(s));
    ClearBufferiv::load_with(|s| loadfn(s));
    ClearBufferuiv::load_with(|s| loadfn(s));
    ClearColor::load_with(|s| loadfn(s));
    ClearDepth::load_with(|s| loadfn(s));
    ClearDepthf::load_with(|s| loadfn(s));
    ClearStencil::load_with(|s| loadfn(s));
    ClientWaitSync::load_with(|s| loadfn(s));
    ColorMask::load_with(|s| loadfn(s));
    ColorMaski::load_with(|s| loadfn(s));
    ColorP3ui::load_with(|s| loadfn(s));
    ColorP3uiv::load_with(|s| loadfn(s));
    ColorP4ui::load_with(|s| loadfn(s));
    ColorP4uiv::load_with(|s| loadfn(s));
    CompileShader::load_with(|s| loadfn(s));
    CompressedTexImage1D::load_with(|s| loadfn(s));
    CompressedTexImage2D::load_with(|s| loadfn(s));
    CompressedTexImage3D::load_with(|s| loadfn(s));
    CompressedTexSubImage1D::load_with(|s| loadfn(s));
    CompressedTexSubImage2D::load_with(|s| loadfn(s));
    CompressedTexSubImage3D::load_with(|s| loadfn(s));
    CopyBufferSubData::load_with(|s| loadfn(s));
    CopyImageSubData::load_with(|s| loadfn(s));
    CopyTexImage1D::load_with(|s| loadfn(s));
    CopyTexImage2D::load_with(|s| loadfn(s));
    CopyTexSubImage1D::load_with(|s| loadfn(s));
    CopyTexSubImage2D::load_with(|s| loadfn(s));
    CopyTexSubImage3D::load_with(|s| loadfn(s));
    CreateProgram::load_with(|s| loadfn(s));
    CreateShader::load_with(|s| loadfn(s));
    CreateShaderProgramv::load_with(|s| loadfn(s));
    CullFace::load_with(|s| loadfn(s));
    DebugMessageCallback::load_with(|s| loadfn(s));
    DebugMessageControl::load_with(|s| loadfn(s));
    DebugMessageInsert::load_with(|s| loadfn(s));
    DeleteBuffers::load_with(|s| loadfn(s));
    DeleteFramebuffers::load_with(|s| loadfn(s));
    DeleteProgram::load_with(|s| loadfn(s));
    DeleteProgramPipelines::load_with(|s| loadfn(s));
    DeleteQueries::load_with(|s| loadfn(s));
    DeleteRenderbuffers::load_with(|s| loadfn(s));
    DeleteSamplers::load_with(|s| loadfn(s));
    DeleteShader::load_with(|s| loadfn(s));
    DeleteSync::load_with(|s| loadfn(s));
    DeleteTextures::load_with(|s| loadfn(s));
    DeleteTransformFeedbacks::load_with(|s| loadfn(s));
    DeleteVertexArrays::load_with(|s| loadfn(s));
    DepthFunc::load_with(|s| loadfn(s));
    DepthMask::load_with(|s| loadfn(s));
    DepthRange::load_with(|s| loadfn(s));
    DepthRangeArrayv::load_with(|s| loadfn(s));
    DepthRangeIndexed::load_with(|s| loadfn(s));
    DepthRangef::load_with(|s| loadfn(s));
    DetachShader::load_with(|s| loadfn(s));
    Disable::load_with(|s| loadfn(s));
    DisableVertexAttribArray::load_with(|s| loadfn(s));
    Disablei::load_with(|s| loadfn(s));
    DispatchCompute::load_with(|s| loadfn(s));
    DispatchComputeIndirect::load_with(|s| loadfn(s));
    DrawArrays::load_with(|s| loadfn(s));
    DrawArraysIndirect::load_with(|s| loadfn(s));
    DrawArraysInstanced::load_with(|s| loadfn(s));
    DrawArraysInstancedBaseInstance::load_with(|s| loadfn(s));
    DrawBuffer::load_with(|s| loadfn(s));
    DrawBuffers::load_with(|s| loadfn(s));
    DrawElements::load_with(|s| loadfn(s));
    DrawElementsBaseVertex::load_with(|s| loadfn(s));
    DrawElementsIndirect::load_with(|s| loadfn(s));
    DrawElementsInstanced::load_with(|s| loadfn(s));
    DrawElementsInstancedBaseInstance::load_with(|s| loadfn(s));
    DrawElementsInstancedBaseVertex::load_with(|s| loadfn(s));
    DrawElementsInstancedBaseVertexBaseInstance::load_with(|s| loadfn(s));
    DrawRangeElements::load_with(|s| loadfn(s));
    DrawRangeElementsBaseVertex::load_with(|s| loadfn(s));
    DrawTransformFeedback::load_with(|s| loadfn(s));
    DrawTransformFeedbackInstanced::load_with(|s| loadfn(s));
    DrawTransformFeedbackStream::load_with(|s| loadfn(s));
    DrawTransformFeedbackStreamInstanced::load_with(|s| loadfn(s));
    Enable::load_with(|s| loadfn(s));
    EnableVertexAttribArray::load_with(|s| loadfn(s));
    Enablei::load_with(|s| loadfn(s));
    EndConditionalRender::load_with(|s| loadfn(s));
    EndQuery::load_with(|s| loadfn(s));
    EndQueryIndexed::load_with(|s| loadfn(s));
    EndTransformFeedback::load_with(|s| loadfn(s));
    FenceSync::load_with(|s| loadfn(s));
    Finish::load_with(|s| loadfn(s));
    Flush::load_with(|s| loadfn(s));
    FlushMappedBufferRange::load_with(|s| loadfn(s));
    FramebufferParameteri::load_with(|s| loadfn(s));
    FramebufferRenderbuffer::load_with(|s| loadfn(s));
    FramebufferTexture::load_with(|s| loadfn(s));
    FramebufferTexture1D::load_with(|s| loadfn(s));
    FramebufferTexture2D::load_with(|s| loadfn(s));
    FramebufferTexture3D::load_with(|s| loadfn(s));
    FramebufferTextureLayer::load_with(|s| loadfn(s));
    FrontFace::load_with(|s| loadfn(s));
    GenBuffers::load_with(|s| loadfn(s));
    GenFramebuffers::load_with(|s| loadfn(s));
    GenProgramPipelines::load_with(|s| loadfn(s));
    GenQueries::load_with(|s| loadfn(s));
    GenRenderbuffers::load_with(|s| loadfn(s));
    GenSamplers::load_with(|s| loadfn(s));
    GenTextures::load_with(|s| loadfn(s));
    GenTransformFeedbacks::load_with(|s| loadfn(s));
    GenVertexArrays::load_with(|s| loadfn(s));
    GenerateMipmap::load_with(|s| loadfn(s));
    GetActiveAtomicCounterBufferiv::load_with(|s| loadfn(s));
    GetActiveAttrib::load_with(|s| loadfn(s));
    GetActiveSubroutineName::load_with(|s| loadfn(s));
    GetActiveSubroutineUniformName::load_with(|s| loadfn(s));
    GetActiveSubroutineUniformiv::load_with(|s| loadfn(s));
    GetActiveUniform::load_with(|s| loadfn(s));
    GetActiveUniformBlockName::load_with(|s| loadfn(s));
    GetActiveUniformBlockiv::load_with(|s| loadfn(s));
    GetActiveUniformName::load_with(|s| loadfn(s));
    GetActiveUniformsiv::load_with(|s| loadfn(s));
    GetAttachedShaders::load_with(|s| loadfn(s));
    GetAttribLocation::load_with(|s| loadfn(s));
    GetBooleani_v::load_with(|s| loadfn(s));
    GetBooleanv::load_with(|s| loadfn(s));
    GetBufferParameteri64v::load_with(|s| loadfn(s));
    GetBufferParameteriv::load_with(|s| loadfn(s));
    GetBufferPointerv::load_with(|s| loadfn(s));
    GetBufferSubData::load_with(|s| loadfn(s));
    GetCompressedTexImage::load_with(|s| loadfn(s));
    GetDebugMessageLog::load_with(|s| loadfn(s));
    GetDoublei_v::load_with(|s| loadfn(s));
    GetDoublev::load_with(|s| loadfn(s));
    GetError::load_with(|s| loadfn(s));
    GetFloati_v::load_with(|s| loadfn(s));
    GetFloatv::load_with(|s| loadfn(s));
    GetFragDataIndex::load_with(|s| loadfn(s));
    GetFragDataLocation::load_with(|s| loadfn(s));
    GetFramebufferAttachmentParameteriv::load_with(|s| loadfn(s));
    GetFramebufferParameteriv::load_with(|s| loadfn(s));
    GetInteger64i_v::load_with(|s| loadfn(s));
    GetInteger64v::load_with(|s| loadfn(s));
    GetIntegeri_v::load_with(|s| loadfn(s));
    GetIntegerv::load_with(|s| loadfn(s));
    GetInternalformati64v::load_with(|s| loadfn(s));
    GetInternalformativ::load_with(|s| loadfn(s));
    GetMultisamplefv::load_with(|s| loadfn(s));
    GetObjectLabel::load_with(|s| loadfn(s));
    GetObjectPtrLabel::load_with(|s| loadfn(s));
    GetProgramBinary::load_with(|s| loadfn(s));
    GetProgramInfoLog::load_with(|s| loadfn(s));
    GetProgramInterfaceiv::load_with(|s| loadfn(s));
    GetProgramPipelineInfoLog::load_with(|s| loadfn(s));
    GetProgramPipelineiv::load_with(|s| loadfn(s));
    GetProgramResourceIndex::load_with(|s| loadfn(s));
    GetProgramResourceLocation::load_with(|s| loadfn(s));
    GetProgramResourceLocationIndex::load_with(|s| loadfn(s));
    GetProgramResourceName::load_with(|s| loadfn(s));
    GetProgramResourceiv::load_with(|s| loadfn(s));
    GetProgramStageiv::load_with(|s| loadfn(s));
    GetProgramiv::load_with(|s| loadfn(s));
    GetQueryIndexediv::load_with(|s| loadfn(s));
    GetQueryObjecti64v::load_with(|s| loadfn(s));
    GetQueryObjectiv::load_with(|s| loadfn(s));
    GetQueryObjectui64v::load_with(|s| loadfn(s));
    GetQueryObjectuiv::load_with(|s| loadfn(s));
    GetQueryiv::load_with(|s| loadfn(s));
    GetRenderbufferParameteriv::load_with(|s| loadfn(s));
    GetSamplerParameterIiv::load_with(|s| loadfn(s));
    GetSamplerParameterIuiv::load_with(|s| loadfn(s));
    GetSamplerParameterfv::load_with(|s| loadfn(s));
    GetSamplerParameteriv::load_with(|s| loadfn(s));
    GetShaderInfoLog::load_with(|s| loadfn(s));
    GetShaderPrecisionFormat::load_with(|s| loadfn(s));
    GetShaderSource::load_with(|s| loadfn(s));
    GetShaderiv::load_with(|s| loadfn(s));
    GetString::load_with(|s| loadfn(s));
    GetStringi::load_with(|s| loadfn(s));
    GetSubroutineIndex::load_with(|s| loadfn(s));
    GetSubroutineUniformLocation::load_with(|s| loadfn(s));
    GetSynciv::load_with(|s| loadfn(s));
    GetTexImage::load_with(|s| loadfn(s));
    GetTexLevelParameterfv::load_with(|s| loadfn(s));
    GetTexLevelParameteriv::load_with(|s| loadfn(s));
    GetTexParameterIiv::load_with(|s| loadfn(s));
    GetTexParameterIuiv::load_with(|s| loadfn(s));
    GetTexParameterfv::load_with(|s| loadfn(s));
    GetTexParameteriv::load_with(|s| loadfn(s));
    GetTransformFeedbackVarying::load_with(|s| loadfn(s));
    GetUniformBlockIndex::load_with(|s| loadfn(s));
    GetUniformIndices::load_with(|s| loadfn(s));
    GetUniformLocation::load_with(|s| loadfn(s));
    GetUniformSubroutineuiv::load_with(|s| loadfn(s));
    GetUniformdv::load_with(|s| loadfn(s));
    GetUniformfv::load_with(|s| loadfn(s));
    GetUniformiv::load_with(|s| loadfn(s));
    GetUniformuiv::load_with(|s| loadfn(s));
    GetVertexAttribIiv::load_with(|s| loadfn(s));
    GetVertexAttribIuiv::load_with(|s| loadfn(s));
    GetVertexAttribLdv::load_with(|s| loadfn(s));
    GetVertexAttribPointerv::load_with(|s| loadfn(s));
    GetVertexAttribdv::load_with(|s| loadfn(s));
    GetVertexAttribfv::load_with(|s| loadfn(s));
    GetVertexAttribiv::load_with(|s| loadfn(s));
    Hint::load_with(|s| loadfn(s));
    InvalidateBufferData::load_with(|s| loadfn(s));
    InvalidateBufferSubData::load_with(|s| loadfn(s));
    InvalidateFramebuffer::load_with(|s| loadfn(s));
    InvalidateSubFramebuffer::load_with(|s| loadfn(s));
    InvalidateTexImage::load_with(|s| loadfn(s));
    InvalidateTexSubImage::load_with(|s| loadfn(s));
    IsBuffer::load_with(|s| loadfn(s));
    IsEnabled::load_with(|s| loadfn(s));
    IsEnabledi::load_with(|s| loadfn(s));
    IsFramebuffer::load_with(|s| loadfn(s));
    IsProgram::load_with(|s| loadfn(s));
    IsProgramPipeline::load_with(|s| loadfn(s));
    IsQuery::load_with(|s| loadfn(s));
    IsRenderbuffer::load_with(|s| loadfn(s));
    IsSampler::load_with(|s| loadfn(s));
    IsShader::load_with(|s| loadfn(s));
    IsSync::load_with(|s| loadfn(s));
    IsTexture::load_with(|s| loadfn(s));
    IsTransformFeedback::load_with(|s| loadfn(s));
    IsVertexArray::load_with(|s| loadfn(s));
    LineWidth::load_with(|s| loadfn(s));
    LinkProgram::load_with(|s| loadfn(s));
    LogicOp::load_with(|s| loadfn(s));
    MapBuffer::load_with(|s| loadfn(s));
    MapBufferRange::load_with(|s| loadfn(s));
    MemoryBarrier::load_with(|s| loadfn(s));
    MinSampleShading::load_with(|s| loadfn(s));
    MultiDrawArrays::load_with(|s| loadfn(s));
    MultiDrawArraysIndirect::load_with(|s| loadfn(s));
    MultiDrawElements::load_with(|s| loadfn(s));
    MultiDrawElementsBaseVertex::load_with(|s| loadfn(s));
    MultiDrawElementsIndirect::load_with(|s| loadfn(s));
    MultiTexCoordP1ui::load_with(|s| loadfn(s));
    MultiTexCoordP1uiv::load_with(|s| loadfn(s));
    MultiTexCoordP2ui::load_with(|s| loadfn(s));
    MultiTexCoordP2uiv::load_with(|s| loadfn(s));
    MultiTexCoordP3ui::load_with(|s| loadfn(s));
    MultiTexCoordP3uiv::load_with(|s| loadfn(s));
    MultiTexCoordP4ui::load_with(|s| loadfn(s));
    MultiTexCoordP4uiv::load_with(|s| loadfn(s));
    NormalP3ui::load_with(|s| loadfn(s));
    NormalP3uiv::load_with(|s| loadfn(s));
    ObjectLabel::load_with(|s| loadfn(s));
    ObjectPtrLabel::load_with(|s| loadfn(s));
    PatchParameterfv::load_with(|s| loadfn(s));
    PatchParameteri::load_with(|s| loadfn(s));
    PauseTransformFeedback::load_with(|s| loadfn(s));
    PixelStoref::load_with(|s| loadfn(s));
    PixelStorei::load_with(|s| loadfn(s));
    PointParameterf::load_with(|s| loadfn(s));
    PointParameterfv::load_with(|s| loadfn(s));
    PointParameteri::load_with(|s| loadfn(s));
    PointParameteriv::load_with(|s| loadfn(s));
    PointSize::load_with(|s| loadfn(s));
    PolygonMode::load_with(|s| loadfn(s));
    PolygonOffset::load_with(|s| loadfn(s));
    PopDebugGroup::load_with(|s| loadfn(s));
    PrimitiveRestartIndex::load_with(|s| loadfn(s));
    ProgramBinary::load_with(|s| loadfn(s));
    ProgramParameteri::load_with(|s| loadfn(s));
    ProgramUniform1d::load_with(|s| loadfn(s));
    ProgramUniform1dv::load_with(|s| loadfn(s));
    ProgramUniform1f::load_with(|s| loadfn(s));
    ProgramUniform1fv::load_with(|s| loadfn(s));
    ProgramUniform1i::load_with(|s| loadfn(s));
    ProgramUniform1iv::load_with(|s| loadfn(s));
    ProgramUniform1ui::load_with(|s| loadfn(s));
    ProgramUniform1uiv::load_with(|s| loadfn(s));
    ProgramUniform2d::load_with(|s| loadfn(s));
    ProgramUniform2dv::load_with(|s| loadfn(s));
    ProgramUniform2f::load_with(|s| loadfn(s));
    ProgramUniform2fv::load_with(|s| loadfn(s));
    ProgramUniform2i::load_with(|s| loadfn(s));
    ProgramUniform2iv::load_with(|s| loadfn(s));
    ProgramUniform2ui::load_with(|s| loadfn(s));
    ProgramUniform2uiv::load_with(|s| loadfn(s));
    ProgramUniform3d::load_with(|s| loadfn(s));
    ProgramUniform3dv::load_with(|s| loadfn(s));
    ProgramUniform3f::load_with(|s| loadfn(s));
    ProgramUniform3fv::load_with(|s| loadfn(s));
    ProgramUniform3i::load_with(|s| loadfn(s));
    ProgramUniform3iv::load_with(|s| loadfn(s));
    ProgramUniform3ui::load_with(|s| loadfn(s));
    ProgramUniform3uiv::load_with(|s| loadfn(s));
    ProgramUniform4d::load_with(|s| loadfn(s));
    ProgramUniform4dv::load_with(|s| loadfn(s));
    ProgramUniform4f::load_with(|s| loadfn(s));
    ProgramUniform4fv::load_with(|s| loadfn(s));
    ProgramUniform4i::load_with(|s| loadfn(s));
    ProgramUniform4iv::load_with(|s| loadfn(s));
    ProgramUniform4ui::load_with(|s| loadfn(s));
    ProgramUniform4uiv::load_with(|s| loadfn(s));
    ProgramUniformMatrix2dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix2fv::load_with(|s| loadfn(s));
    ProgramUniformMatrix2x3dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix2x3fv::load_with(|s| loadfn(s));
    ProgramUniformMatrix2x4dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix2x4fv::load_with(|s| loadfn(s));
    ProgramUniformMatrix3dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix3fv::load_with(|s| loadfn(s));
    ProgramUniformMatrix3x2dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix3x2fv::load_with(|s| loadfn(s));
    ProgramUniformMatrix3x4dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix3x4fv::load_with(|s| loadfn(s));
    ProgramUniformMatrix4dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix4fv::load_with(|s| loadfn(s));
    ProgramUniformMatrix4x2dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix4x2fv::load_with(|s| loadfn(s));
    ProgramUniformMatrix4x3dv::load_with(|s| loadfn(s));
    ProgramUniformMatrix4x3fv::load_with(|s| loadfn(s));
    ProvokingVertex::load_with(|s| loadfn(s));
    PushDebugGroup::load_with(|s| loadfn(s));
    QueryCounter::load_with(|s| loadfn(s));
    ReadBuffer::load_with(|s| loadfn(s));
    ReadPixels::load_with(|s| loadfn(s));
    ReleaseShaderCompiler::load_with(|s| loadfn(s));
    RenderbufferStorage::load_with(|s| loadfn(s));
    RenderbufferStorageMultisample::load_with(|s| loadfn(s));
    ResumeTransformFeedback::load_with(|s| loadfn(s));
    SampleCoverage::load_with(|s| loadfn(s));
    SampleMaski::load_with(|s| loadfn(s));
    SamplerParameterIiv::load_with(|s| loadfn(s));
    SamplerParameterIuiv::load_with(|s| loadfn(s));
    SamplerParameterf::load_with(|s| loadfn(s));
    SamplerParameterfv::load_with(|s| loadfn(s));
    SamplerParameteri::load_with(|s| loadfn(s));
    SamplerParameteriv::load_with(|s| loadfn(s));
    Scissor::load_with(|s| loadfn(s));
    ScissorArrayv::load_with(|s| loadfn(s));
    ScissorIndexed::load_with(|s| loadfn(s));
    ScissorIndexedv::load_with(|s| loadfn(s));
    SecondaryColorP3ui::load_with(|s| loadfn(s));
    SecondaryColorP3uiv::load_with(|s| loadfn(s));
    ShaderBinary::load_with(|s| loadfn(s));
    ShaderSource::load_with(|s| loadfn(s));
    ShaderStorageBlockBinding::load_with(|s| loadfn(s));
    StencilFunc::load_with(|s| loadfn(s));
    StencilFuncSeparate::load_with(|s| loadfn(s));
    StencilMask::load_with(|s| loadfn(s));
    StencilMaskSeparate::load_with(|s| loadfn(s));
    StencilOp::load_with(|s| loadfn(s));
    StencilOpSeparate::load_with(|s| loadfn(s));
    TexBuffer::load_with(|s| loadfn(s));
    TexBufferRange::load_with(|s| loadfn(s));
    TexCoordP1ui::load_with(|s| loadfn(s));
    TexCoordP1uiv::load_with(|s| loadfn(s));
    TexCoordP2ui::load_with(|s| loadfn(s));
    TexCoordP2uiv::load_with(|s| loadfn(s));
    TexCoordP3ui::load_with(|s| loadfn(s));
    TexCoordP3uiv::load_with(|s| loadfn(s));
    TexCoordP4ui::load_with(|s| loadfn(s));
    TexCoordP4uiv::load_with(|s| loadfn(s));
    TexImage1D::load_with(|s| loadfn(s));
    TexImage2D::load_with(|s| loadfn(s));
    TexImage2DMultisample::load_with(|s| loadfn(s));
    TexImage3D::load_with(|s| loadfn(s));
    TexImage3DMultisample::load_with(|s| loadfn(s));
    TexParameterIiv::load_with(|s| loadfn(s));
    TexParameterIuiv::load_with(|s| loadfn(s));
    TexParameterf::load_with(|s| loadfn(s));
    TexParameterfv::load_with(|s| loadfn(s));
    TexParameteri::load_with(|s| loadfn(s));
    TexParameteriv::load_with(|s| loadfn(s));
    TexStorage1D::load_with(|s| loadfn(s));
    TexStorage2D::load_with(|s| loadfn(s));
    TexStorage2DMultisample::load_with(|s| loadfn(s));
    TexStorage3D::load_with(|s| loadfn(s));
    TexStorage3DMultisample::load_with(|s| loadfn(s));
    TexSubImage1D::load_with(|s| loadfn(s));
    TexSubImage2D::load_with(|s| loadfn(s));
    TexSubImage3D::load_with(|s| loadfn(s));
    TextureView::load_with(|s| loadfn(s));
    TransformFeedbackVaryings::load_with(|s| loadfn(s));
    Uniform1d::load_with(|s| loadfn(s));
    Uniform1dv::load_with(|s| loadfn(s));
    Uniform1f::load_with(|s| loadfn(s));
    Uniform1fv::load_with(|s| loadfn(s));
    Uniform1i::load_with(|s| loadfn(s));
    Uniform1iv::load_with(|s| loadfn(s));
    Uniform1ui::load_with(|s| loadfn(s));
    Uniform1uiv::load_with(|s| loadfn(s));
    Uniform2d::load_with(|s| loadfn(s));
    Uniform2dv::load_with(|s| loadfn(s));
    Uniform2f::load_with(|s| loadfn(s));
    Uniform2fv::load_with(|s| loadfn(s));
    Uniform2i::load_with(|s| loadfn(s));
    Uniform2iv::load_with(|s| loadfn(s));
    Uniform2ui::load_with(|s| loadfn(s));
    Uniform2uiv::load_with(|s| loadfn(s));
    Uniform3d::load_with(|s| loadfn(s));
    Uniform3dv::load_with(|s| loadfn(s));
    Uniform3f::load_with(|s| loadfn(s));
    Uniform3fv::load_with(|s| loadfn(s));
    Uniform3i::load_with(|s| loadfn(s));
    Uniform3iv::load_with(|s| loadfn(s));
    Uniform3ui::load_with(|s| loadfn(s));
    Uniform3uiv::load_with(|s| loadfn(s));
    Uniform4d::load_with(|s| loadfn(s));
    Uniform4dv::load_with(|s| loadfn(s));
    Uniform4f::load_with(|s| loadfn(s));
    Uniform4fv::load_with(|s| loadfn(s));
    Uniform4i::load_with(|s| loadfn(s));
    Uniform4iv::load_with(|s| loadfn(s));
    Uniform4ui::load_with(|s| loadfn(s));
    Uniform4uiv::load_with(|s| loadfn(s));
    UniformBlockBinding::load_with(|s| loadfn(s));
    UniformMatrix2dv::load_with(|s| loadfn(s));
    UniformMatrix2fv::load_with(|s| loadfn(s));
    UniformMatrix2x3dv::load_with(|s| loadfn(s));
    UniformMatrix2x3fv::load_with(|s| loadfn(s));
    UniformMatrix2x4dv::load_with(|s| loadfn(s));
    UniformMatrix2x4fv::load_with(|s| loadfn(s));
    UniformMatrix3dv::load_with(|s| loadfn(s));
    UniformMatrix3fv::load_with(|s| loadfn(s));
    UniformMatrix3x2dv::load_with(|s| loadfn(s));
    UniformMatrix3x2fv::load_with(|s| loadfn(s));
    UniformMatrix3x4dv::load_with(|s| loadfn(s));
    UniformMatrix3x4fv::load_with(|s| loadfn(s));
    UniformMatrix4dv::load_with(|s| loadfn(s));
    UniformMatrix4fv::load_with(|s| loadfn(s));
    UniformMatrix4x2dv::load_with(|s| loadfn(s));
    UniformMatrix4x2fv::load_with(|s| loadfn(s));
    UniformMatrix4x3dv::load_with(|s| loadfn(s));
    UniformMatrix4x3fv::load_with(|s| loadfn(s));
    UniformSubroutinesuiv::load_with(|s| loadfn(s));
    UnmapBuffer::load_with(|s| loadfn(s));
    UseProgram::load_with(|s| loadfn(s));
    UseProgramStages::load_with(|s| loadfn(s));
    ValidateProgram::load_with(|s| loadfn(s));
    ValidateProgramPipeline::load_with(|s| loadfn(s));
    VertexAttrib1d::load_with(|s| loadfn(s));
    VertexAttrib1dv::load_with(|s| loadfn(s));
    VertexAttrib1f::load_with(|s| loadfn(s));
    VertexAttrib1fv::load_with(|s| loadfn(s));
    VertexAttrib1s::load_with(|s| loadfn(s));
    VertexAttrib1sv::load_with(|s| loadfn(s));
    VertexAttrib2d::load_with(|s| loadfn(s));
    VertexAttrib2dv::load_with(|s| loadfn(s));
    VertexAttrib2f::load_with(|s| loadfn(s));
    VertexAttrib2fv::load_with(|s| loadfn(s));
    VertexAttrib2s::load_with(|s| loadfn(s));
    VertexAttrib2sv::load_with(|s| loadfn(s));
    VertexAttrib3d::load_with(|s| loadfn(s));
    VertexAttrib3dv::load_with(|s| loadfn(s));
    VertexAttrib3f::load_with(|s| loadfn(s));
    VertexAttrib3fv::load_with(|s| loadfn(s));
    VertexAttrib3s::load_with(|s| loadfn(s));
    VertexAttrib3sv::load_with(|s| loadfn(s));
    VertexAttrib4Nbv::load_with(|s| loadfn(s));
    VertexAttrib4Niv::load_with(|s| loadfn(s));
    VertexAttrib4Nsv::load_with(|s| loadfn(s));
    VertexAttrib4Nub::load_with(|s| loadfn(s));
    VertexAttrib4Nubv::load_with(|s| loadfn(s));
    VertexAttrib4Nuiv::load_with(|s| loadfn(s));
    VertexAttrib4Nusv::load_with(|s| loadfn(s));
    VertexAttrib4bv::load_with(|s| loadfn(s));
    VertexAttrib4d::load_with(|s| loadfn(s));
    VertexAttrib4dv::load_with(|s| loadfn(s));
    VertexAttrib4f::load_with(|s| loadfn(s));
    VertexAttrib4fv::load_with(|s| loadfn(s));
    VertexAttrib4iv::load_with(|s| loadfn(s));
    VertexAttrib4s::load_with(|s| loadfn(s));
    VertexAttrib4sv::load_with(|s| loadfn(s));
    VertexAttrib4ubv::load_with(|s| loadfn(s));
    VertexAttrib4uiv::load_with(|s| loadfn(s));
    VertexAttrib4usv::load_with(|s| loadfn(s));
    VertexAttribBinding::load_with(|s| loadfn(s));
    VertexAttribDivisor::load_with(|s| loadfn(s));
    VertexAttribFormat::load_with(|s| loadfn(s));
    VertexAttribI1i::load_with(|s| loadfn(s));
    VertexAttribI1iv::load_with(|s| loadfn(s));
    VertexAttribI1ui::load_with(|s| loadfn(s));
    VertexAttribI1uiv::load_with(|s| loadfn(s));
    VertexAttribI2i::load_with(|s| loadfn(s));
    VertexAttribI2iv::load_with(|s| loadfn(s));
    VertexAttribI2ui::load_with(|s| loadfn(s));
    VertexAttribI2uiv::load_with(|s| loadfn(s));
    VertexAttribI3i::load_with(|s| loadfn(s));
    VertexAttribI3iv::load_with(|s| loadfn(s));
    VertexAttribI3ui::load_with(|s| loadfn(s));
    VertexAttribI3uiv::load_with(|s| loadfn(s));
    VertexAttribI4bv::load_with(|s| loadfn(s));
    VertexAttribI4i::load_with(|s| loadfn(s));
    VertexAttribI4iv::load_with(|s| loadfn(s));
    VertexAttribI4sv::load_with(|s| loadfn(s));
    VertexAttribI4ubv::load_with(|s| loadfn(s));
    VertexAttribI4ui::load_with(|s| loadfn(s));
    VertexAttribI4uiv::load_with(|s| loadfn(s));
    VertexAttribI4usv::load_with(|s| loadfn(s));
    VertexAttribIFormat::load_with(|s| loadfn(s));
    VertexAttribIPointer::load_with(|s| loadfn(s));
    VertexAttribL1d::load_with(|s| loadfn(s));
    VertexAttribL1dv::load_with(|s| loadfn(s));
    VertexAttribL2d::load_with(|s| loadfn(s));
    VertexAttribL2dv::load_with(|s| loadfn(s));
    VertexAttribL3d::load_with(|s| loadfn(s));
    VertexAttribL3dv::load_with(|s| loadfn(s));
    VertexAttribL4d::load_with(|s| loadfn(s));
    VertexAttribL4dv::load_with(|s| loadfn(s));
    VertexAttribLFormat::load_with(|s| loadfn(s));
    VertexAttribLPointer::load_with(|s| loadfn(s));
    VertexAttribP1ui::load_with(|s| loadfn(s));
    VertexAttribP1uiv::load_with(|s| loadfn(s));
    VertexAttribP2ui::load_with(|s| loadfn(s));
    VertexAttribP2uiv::load_with(|s| loadfn(s));
    VertexAttribP3ui::load_with(|s| loadfn(s));
    VertexAttribP3uiv::load_with(|s| loadfn(s));
    VertexAttribP4ui::load_with(|s| loadfn(s));
    VertexAttribP4uiv::load_with(|s| loadfn(s));
    VertexAttribPointer::load_with(|s| loadfn(s));
    VertexBindingDivisor::load_with(|s| loadfn(s));
    VertexP2ui::load_with(|s| loadfn(s));
    VertexP2uiv::load_with(|s| loadfn(s));
    VertexP3ui::load_with(|s| loadfn(s));
    VertexP3uiv::load_with(|s| loadfn(s));
    VertexP4ui::load_with(|s| loadfn(s));
    VertexP4uiv::load_with(|s| loadfn(s));
    Viewport::load_with(|s| loadfn(s));
    ViewportArrayv::load_with(|s| loadfn(s));
    ViewportIndexedf::load_with(|s| loadfn(s));
    ViewportIndexedfv::load_with(|s| loadfn(s));
    WaitSync::load_with(|s| loadfn(s));
}

