use super::*;
use x11rb::protocol::randr::{self, ConnectionExt as _};

impl XConnection {
    pub fn set_crtc_config(&self, crtc_id: u32, mode_id: randr::Mode) -> Result<(), PlatformError> {
        let root = self.default_screen().root;
        let version = self.connection.randr_query_version(0, 0)?.reply()?;
        let timestamp = if version.major_version == 1 && version.minor_version >= 3 {
            self.connection
                .randr_get_screen_resources_current(root)?
                .reply()?
                .timestamp
        } else {
            self.connection
                .randr_get_screen_resources(root)?
                .reply()?
                .timestamp
        };

        let crtc = self
            .connection
            .randr_get_crtc_info(crtc_id, timestamp)?
            .reply()?;

        self.connection
            .randr_set_crtc_config(
                crtc_id,
                timestamp,
                x11rb::CURRENT_TIME,
                crtc.x,
                crtc.y,
                mode_id,
                crtc.rotation,
                &crtc.outputs,
            )?
            .reply()?;

        Ok(())
    }

    pub fn get_crtc_mode(&self, crtc_id: u32) -> Result<randr::Mode, PlatformError> {
        let root = self.default_screen().root;
        let version = self.connection.randr_query_version(0, 0)?.reply()?;

        // Get the timestamp to use.
        let timestamp = if version.major_version == 1 && version.minor_version >= 3 {
            self.connection
                .randr_get_screen_resources_current(root)?
                .reply()?
                .timestamp
        } else {
            self.connection
                .randr_get_screen_resources(root)?
                .reply()?
                .timestamp
        };

        // Fetch the CRTC version.
        let crtc = self
            .connection
            .randr_get_crtc_info(crtc_id, timestamp)?
            .reply()?;
        Ok(crtc.mode)
    }
}
