//! SCTK environment setup.

use sctk::reexports::client::protocol::wl_compositor::WlCompositor;
use sctk::reexports::client::protocol::wl_output::WlOutput;
use sctk::reexports::protocols::unstable::xdg_shell::v6::client::zxdg_shell_v6::ZxdgShellV6;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::protocols::unstable::xdg_decoration::v1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use sctk::reexports::client::protocol::wl_shell::WlShell;
use sctk::reexports::client::protocol::wl_subcompositor::WlSubcompositor;
use sctk::reexports::client::{Attached, DispatchData};
use sctk::reexports::client::protocol::wl_shm::WlShm;
use sctk::reexports::protocols::xdg_shell::client::xdg_wm_base::XdgWmBase;
use sctk::reexports::protocols::unstable::relative_pointer::v1::client::zwp_relative_pointer_manager_v1::ZwpRelativePointerManagerV1;
use sctk::reexports::protocols::unstable::pointer_constraints::v1::client::zwp_pointer_constraints_v1::ZwpPointerConstraintsV1;
use sctk::reexports::protocols::unstable::text_input::v3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use sctk::reexports::protocols::staging::xdg_activation::v1::client::xdg_activation_v1::XdgActivationV1;
use sctk::reexports::protocols::viewporter::client::wp_viewporter::WpViewporter;

use sctk::environment::{Environment, SimpleGlobal};
use sctk::output::{OutputHandler, OutputHandling, OutputInfo, OutputStatusListener};
use sctk::seat::{SeatData, SeatHandler, SeatHandling, SeatListener};
use sctk::shell::{Shell, ShellHandler, ShellHandling};
use sctk::shm::ShmHandler;

use crate::platform_impl::wayland::protocols::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;

/// Set of extra features that are supported by the compositor.
#[derive(Debug, Clone, Copy)]
pub struct WindowingFeatures {
    pointer_constraints: bool,
    xdg_activation: bool,
}

impl WindowingFeatures {
    /// Create `WindowingFeatures` based on the presented interfaces.
    pub fn new(env: &Environment<WinitEnv>) -> Self {
        let pointer_constraints = env.get_global::<ZwpPointerConstraintsV1>().is_some();
        let xdg_activation = env.get_global::<XdgActivationV1>().is_some();
        Self {
            pointer_constraints,
            xdg_activation,
        }
    }

    pub fn pointer_constraints(&self) -> bool {
        self.pointer_constraints
    }

    pub fn xdg_activation(&self) -> bool {
        self.xdg_activation
    }
}

sctk::environment!(WinitEnv,
    singles = [
        WlShm => shm,
        WlCompositor => compositor,
        WlSubcompositor => subcompositor,
        WlShell => shell,
        XdgWmBase => shell,
        ZxdgShellV6 => shell,
        ZxdgDecorationManagerV1 => decoration_manager,
        ZwpRelativePointerManagerV1 => relative_pointer_manager,
        ZwpPointerConstraintsV1 => pointer_constraints,
        ZwpTextInputManagerV3 => text_input_manager,
        XdgActivationV1 => xdg_activation,
        WpFractionalScaleManagerV1 => fractional_scale_manager,
        WpViewporter => viewporter,
    ],
    multis = [
        WlSeat => seats,
        WlOutput => outputs,
    ]
);

/// The environment that we utilize.
pub struct WinitEnv {
    seats: SeatHandler,

    outputs: OutputHandler,

    shm: ShmHandler,

    compositor: SimpleGlobal<WlCompositor>,

    subcompositor: SimpleGlobal<WlSubcompositor>,

    shell: ShellHandler,

    relative_pointer_manager: SimpleGlobal<ZwpRelativePointerManagerV1>,

    pointer_constraints: SimpleGlobal<ZwpPointerConstraintsV1>,

    text_input_manager: SimpleGlobal<ZwpTextInputManagerV3>,

    decoration_manager: SimpleGlobal<ZxdgDecorationManagerV1>,

    xdg_activation: SimpleGlobal<XdgActivationV1>,

    fractional_scale_manager: SimpleGlobal<WpFractionalScaleManagerV1>,

    viewporter: SimpleGlobal<WpViewporter>,
}

impl WinitEnv {
    pub fn new() -> Self {
        // Output tracking for available_monitors, etc.
        let outputs = OutputHandler::new();

        // Keyboard/Pointer/Touch input.
        let seats = SeatHandler::new();

        // Essential globals.
        let shm = ShmHandler::new();
        let compositor = SimpleGlobal::new();
        let subcompositor = SimpleGlobal::new();

        // Gracefully handle shell picking, since SCTK automatically supports multiple
        // backends.
        let shell = ShellHandler::new();

        // Server side decorations.
        let decoration_manager = SimpleGlobal::new();

        // Device events for pointer.
        let relative_pointer_manager = SimpleGlobal::new();

        // Pointer grab functionality.
        let pointer_constraints = SimpleGlobal::new();

        // IME handling.
        let text_input_manager = SimpleGlobal::new();

        // Surface activation.
        let xdg_activation = SimpleGlobal::new();

        // Fractional surface scaling.
        let fractional_scale_manager = SimpleGlobal::new();

        // Surface resizing (used for fractional scaling).
        let viewporter = SimpleGlobal::new();

        Self {
            seats,
            outputs,
            shm,
            compositor,
            subcompositor,
            shell,
            decoration_manager,
            relative_pointer_manager,
            pointer_constraints,
            text_input_manager,
            xdg_activation,
            fractional_scale_manager,
            viewporter,
        }
    }
}

impl ShellHandling for WinitEnv {
    fn get_shell(&self) -> Option<Shell> {
        self.shell.get_shell()
    }
}

impl SeatHandling for WinitEnv {
    fn listen<F: FnMut(Attached<WlSeat>, &SeatData, DispatchData<'_>) + 'static>(
        &mut self,
        f: F,
    ) -> SeatListener {
        self.seats.listen(f)
    }
}

impl OutputHandling for WinitEnv {
    fn listen<F: FnMut(WlOutput, &OutputInfo, DispatchData<'_>) + 'static>(
        &mut self,
        f: F,
    ) -> OutputStatusListener {
        self.outputs.listen(f)
    }
}
