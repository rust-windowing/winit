#[derive(Debug, Clone)]
pub enum Mapping {
    Standard { buttons: [bool; 16], axes: [f64; 6] },
    NoMapping { buttons: Vec<bool>, axes: Vec<f64> },
}

impl Mapping {
    pub(crate) fn buttons<'a>(&'a self) -> impl Iterator<Item = bool> + 'a {
        match self {
            Mapping::Standard { buttons, .. } => buttons.iter(),
            Mapping::NoMapping { buttons, .. } => buttons.iter(),
        }
        .cloned()
    }

    pub(crate) fn axes<'a>(&'a self) -> impl Iterator<Item = f64> + 'a {
        match self {
            Mapping::Standard { axes, .. } => axes.iter(),
            Mapping::NoMapping { axes, .. } => axes.iter(),
        }
        .cloned()
    }
}
