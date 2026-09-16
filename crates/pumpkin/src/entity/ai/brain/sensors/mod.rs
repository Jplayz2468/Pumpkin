//! Ports of vanilla's `net/minecraft/world/entity/ai/sensing` classes.
//!
//! A sensor refreshes memories on its own scan rate; behaviours then read those memories.
//! Keeping the split means a behaviour never searches the world itself.

pub mod hurt_by;
pub mod is_in_water;
pub mod nearest_players;
pub mod temptations;
