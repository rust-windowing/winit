pub struct Canvas;

impl Canvas {
    pub fn new() -> Self {
        let element = document()
            .create_element("canvas")
            .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?;

        let canvas: CanvasElement = element
            .try_into()
            .map_err(|_| os_error!(OsError("Failed to create canvas element".to_owned())))?;

        document()
            .body()
            .ok_or_else(|| os_error!(OsError("Failed to find body node".to_owned())))?
            .append_child(&canvas);

        Canvas(canvas)
    }
}
