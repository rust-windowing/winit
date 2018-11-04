pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value > max {
        max
    } else if value < min {
        min
    } else {
        value
    }
}

pub fn normalize_asymmetric(value: f64, min: f64, max: f64) -> f64 {
    let range = max - min;
    let translated = value - min;
    let scaled = translated / range;
    clamp(scaled, 0.0, 1.0)
}

pub fn normalize_symmetric(value: f64, min: f64, max: f64) -> f64 {
    (2.0 * normalize_asymmetric(value, min, max)) - 1.0
}
