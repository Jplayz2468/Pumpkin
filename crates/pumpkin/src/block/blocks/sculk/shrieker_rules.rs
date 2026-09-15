//! Decisions shared by the live shrieker and direct Java contract probes.
pub const SHRIEK_TICKS: u32 = 90;

pub const fn can_respond(can_summon: bool, peaceful: bool, spawn_wardens: bool) -> bool {
    can_summon && !peaceful && spawn_wardens
}

/// Reset the block's response level for each new shriek, but not an ongoing one.
/// The warning attempt may fail due to a group cooldown or nearby Warden.
pub fn begin_shriek(
    has_player: bool,
    shrieking: bool,
    can_respond: bool,
    warning: &mut i32,
    try_warn: impl FnOnce() -> Option<i32>,
) -> bool {
    if !has_player || shrieking {
        return false;
    }
    *warning = 0;
    if can_respond {
        let Some(level) = try_warn() else {
            return false;
        };
        *warning = level;
    }
    true
}

/// MobEffectUtil's survival/adventure, strict radius, and refresh threshold.
/// Infinite (-1) and >=200 tick equal-or-stronger darkness are not refreshed.
pub fn should_apply_darkness(
    survival_or_adventure: bool,
    distance_squared: f64,
    existing: Option<(i32, i32)>,
) -> bool {
    survival_or_adventure
        && distance_squared < 40.0 * 40.0
        && existing.is_none_or(|(amplifier, duration)| {
            amplifier < 0 || (duration != -1 && duration <= 199)
        })
}
