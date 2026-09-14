//! Java 26.2 BlockPos.containing floors before the saturating double-to-int cast.
pub fn containing(position: [f64; 3]) -> [i32; 3] {
    position.map(|value| value.floor() as i32)
}
