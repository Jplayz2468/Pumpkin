//! Warden.canTargetEntity eligibility; geometry uses Java 26.2's border epsilon.
pub struct TargetFacts {
    pub living: bool,
    pub same_world: bool,
    pub creative_or_spectator: bool,
    pub allied: bool,
    pub armor_stand_or_warden: bool,
    pub invulnerable: bool,
    pub dead_or_dying: bool,
}

pub fn eligible(facts: &TargetFacts, bounds: [f64; 4], border: [f64; 4]) -> bool {
    let epsilon = f64::from(1.0e-5_f32);
    facts.living
        && facts.same_world
        && !facts.creative_or_spectator
        && !facts.allied
        && !facts.armor_stand_or_warden
        && !facts.invulnerable
        && !facts.dead_or_dying
        && bounds[0] >= border[0]
        && bounds[1] >= border[1]
        && bounds[0] < border[2]
        && bounds[1] < border[3]
        && bounds[2] - epsilon >= border[0]
        && bounds[3] - epsilon >= border[1]
        && bounds[2] - epsilon < border[2]
        && bounds[3] - epsilon < border[3]
}
