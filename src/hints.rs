use std::default::Default;

#[deriving(Clone,Show)]
#[deprecated = "Will be removed soon (it's not supported anyway)"]
pub struct Hints {
    pub resizable: bool,
    pub visible: bool,
    pub decorated: bool,
    pub red_bits: u8,
    pub green_bits: u8,
    pub blue_bits: u8,
    pub alpha_bits: u8,
    pub depth_bits: u8,
    pub stencil_bits: u8,
    pub accum_red_bits: u8,
    pub accum_green_bits: u8,
    pub accum_blue_bits: u8,
    pub accum_alpha_bits: u8,
    pub aux_buffers: u8,
    pub samples: u8,
    pub refresh_rate: u8,
    pub stereo: bool,
    pub srgb_capable: bool,
    pub client_api: ClientAPI,
    pub context_version: (u8, u8),
    //pub robustness: ,
    pub opengl_forward_compat: bool,
    pub opengl_debug_context: bool,
    pub opengl_profile: Profile,
}

#[deriving(Clone, Show)]
pub enum ClientAPI {
    OpenGL,
    OpenGLES,
}

#[deriving(Clone, Show)]
pub enum Profile {
    AnyProfile,
    CompatProfile,
    CoreProfile,
}

impl Default for Hints {
    fn default() -> Hints {
        Hints {
            resizable: true,
            visible: true,
            decorated: true,
            red_bits: 8,
            green_bits: 8,
            blue_bits: 8,
            alpha_bits: 8,
            depth_bits: 24,
            stencil_bits: 8,
            accum_red_bits: 0,
            accum_green_bits: 0,
            accum_blue_bits: 0,
            accum_alpha_bits: 0,
            aux_buffers: 0,
            samples: 0,
            refresh_rate: 0,
            stereo: false,
            srgb_capable: false,
            client_api: OpenGL,
            context_version: (1, 0),
            //robustness: ,
            opengl_forward_compat: false,
            opengl_debug_context: false,
            opengl_profile: AnyProfile,
        }
    }
}
