//! Ports of vanilla's `net/minecraft/world/entity/ai/behavior` classes.
//!
//! These are the behaviours every brain mob shares. A species brain is then mostly a
//! list of these plus a handful of its own, which is how vanilla composes them.

pub mod animal_make_love;
pub mod animal_panic;
pub mod baby_follow_adult;
pub mod charge_attack;
pub mod count_down_cooldown_ticks;
pub mod croak;
pub mod do_nothing;
pub mod erase_memory_if;
pub mod follow_temptation;
pub mod gate;
pub mod long_jump;
pub mod long_jump_util;
pub mod look_at_target_sink;
pub mod melee_attack;
pub mod move_to_target_sink;
pub mod play_dead;
pub mod random_look_around;
pub mod random_stroll;
pub mod set_entity_look_target_sometimes;
pub mod set_walk_target_from_attack_target;
pub mod set_walk_target_from_look_target;
pub mod shoot_tongue;
pub mod start_attacking;
pub mod stop_attacking_if_target_invalid;
pub mod swim;
pub mod trigger_if;
pub mod try_find_land;
pub mod try_find_land_near_water;
pub mod try_find_water;
pub mod try_lay_spawn;
