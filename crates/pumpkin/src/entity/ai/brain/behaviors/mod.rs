//! Ports of vanilla's `net/minecraft/world/entity/ai/behavior` classes.
//!
//! These are the behaviours every brain mob shares. A species brain is then mostly a
//! list of these plus a handful of its own, which is how vanilla composes them.

pub mod animal_make_love;
pub mod animal_panic;
pub mod charge_attack;
pub mod count_down_cooldown_ticks;
pub mod follow_temptation;
pub mod gate;
pub mod look_at_target_sink;
pub mod move_to_target_sink;
pub mod random_stroll;
pub mod set_walk_target_from_look_target;
pub mod start_attacking;
