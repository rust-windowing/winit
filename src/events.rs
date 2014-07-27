
#[deriving(Clone,Show)]
pub enum Event {
    /// The position of the window has changed.
    PositionChanged(uint, uint),

    /// The size of the window has changed.
    SizeChanged(uint, uint),

    /// The window has been closed.
    Closed,

    /// The cursor has moved on the window.
    /// 
    /// The parameter are the (x,y) coords in pixels relative to the top-left corner of the window.
    CursorPositionChanged(uint, uint),

    /// The window gained or lost focus.
    /// 
    /// The parameter is true if the window has gained focus, and false if it has lost focus.
    Focused(bool),

    /// The window has been turned into an icon or restored.
    /// 
    /// The parameter is true if the window has been iconified, and false if it has been restored.
    Iconified(bool),

    /// The system asked that the content of this window must be redrawn.
    NeedRefresh,

    /// The size of the framebuffer of the window has changed.
    FramebufferSizeChanged(uint, uint),
}
