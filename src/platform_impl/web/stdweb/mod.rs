#[cfg(feature = "stdweb")]
impl WindowExtStdweb for RootWindow {
    fn canvas(&self) -> CanvasElement {
        self.window.canvas.clone()
    }
}
