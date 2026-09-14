//! Warden MeleeAttack behavior and the unarmed Mob attack-box contract.
pub struct Facts {
    pub no_ai: bool,
    pub active: bool,
    pub attack: bool,
    pub sensor: bool,
    pub visible: bool,
    pub usable_non_melee: bool,
    pub in_range: bool,
}
pub fn attack(cooldown: &mut Option<i64>, facts: &Facts) -> bool {
    if facts.no_ai
        || !facts.active
        || !facts.attack
        || cooldown.is_some()
        || !facts.sensor
        || facts.usable_non_melee
        || !facts.in_range
        || !facts.visible
    {
        return false;
    }
    *cooldown = Some(18);
    true
}
pub fn default_reach() -> f64 {
    f64::from(2.04_f32).sqrt() - f64::from(0.6_f32)
}
pub fn in_range(mut actor: [f64; 6], vehicle: Option<[f64; 6]>, target: [f64; 6]) -> bool {
    if let Some(vehicle) = vehicle {
        actor[0] = actor[0].min(vehicle[0]);
        actor[2] = actor[2].min(vehicle[2]);
        actor[3] = actor[3].max(vehicle[3]);
        actor[5] = actor[5].max(vehicle[5]);
    }
    let r = default_reach();
    actor[0] -= r;
    actor[2] -= r;
    actor[3] += r;
    actor[5] += r;
    actor[0] < target[3]
        && actor[3] > target[0]
        && actor[1] < target[4]
        && actor[4] > target[1]
        && actor[2] < target[5]
        && actor[5] > target[2]
}
