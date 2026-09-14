//! Warden darkness scheduling and MobEffectUtil player eligibility.
pub fn due(tick_count: i32, entity_id: i32, no_ai: bool) -> bool {
    !no_ai && tick_count.wrapping_add(entity_id) % 120 == 0
}
pub fn eligible(
    survival_or_adventure: bool,
    allied: bool,
    distance_squared: f64,
    existing: Option<(i32, i32)>,
) -> bool {
    survival_or_adventure
        && !allied
        && distance_squared < 20.0 * 20.0
        && existing.is_none_or(|(amplifier, duration)| {
            amplifier < 0 || (duration != -1 && duration <= 199)
        })
}
