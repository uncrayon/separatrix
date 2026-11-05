/// Euclidean length from correctly rounded operations only, so geometry that
/// decides which points are kept gives the same answer on every platform
/// (`f64::hypot` comes from the system libm and can differ in the last bit).
/// Inputs are pixel-scale distances, far from overflow.
pub(crate) fn len(dx: f64, dy: f64) -> f64 {
    (dx * dx + dy * dy).sqrt()
}
