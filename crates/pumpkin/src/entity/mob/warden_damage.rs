//! Warden reactions run after a damage attempt, even when no health is lost.
#[derive(Debug)]
pub struct Facts {
    pub no_ai: bool,
    pub digging_or_emerging: bool,
    pub eligible: bool,
    pub has_dig_cooldown: bool,
    pub living: bool,
    pub player: bool,
    pub old_target_player: bool,
    pub direct: bool,
    pub distance_squared: f64,
}
#[derive(Debug, Default)]
pub struct Reaction {
    pub reset_dig: bool,
    pub clear_attack: bool,
    pub set_attack: bool,
}
pub fn react(
    facts: &Facts,
    has_target: bool,
    mut increase_anger: impl FnMut(i32) -> i32,
) -> Reaction {
    if facts.no_ai || facts.digging_or_emerging {
        return Reaction::default();
    }
    let mut result = Reaction::default();
    if facts.eligible {
        result.reset_dig = facts.has_dig_cooldown;
        let anger = increase_anger(100);
        result.clear_attack = facts.player && !facts.old_target_player && anger >= 80;
    }
    // Java can set this memory even for an ineligible living attacker. The
    // fight activity validates it on its own subsequent behavior step.
    result.set_attack = (!has_target || result.clear_attack)
        && facts.living
        && (facts.direct || facts.distance_squared < 25.0);
    result
}
