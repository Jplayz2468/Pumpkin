use super::{Controls, Goal};
use crate::entity::mob::Mob;
use pumpkin_data::Block;
use pumpkin_data::tag::{self, Taggable};
use std::sync::atomic::Ordering;

/// Port of `ClimbOnTopOfPowderSnowGoal` (ClimbOnTopOfPowderSnowGoal.java): while a mob
/// tagged `#minecraft:powder_snow_walkable_mobs` is standing in (or just left) powder
/// snow and the block above it is either powder snow or has no collision shape, jump
/// every tick (ClimbOnTopOfPowderSnowGoal.java:22-42). Used by both Silverfish
/// (Silverfish.java:44) and Endermite (Endermite.java:42).
#[derive(Default)]
pub struct ClimbOnTopOfPowderSnowGoal;

impl Goal for ClimbOnTopOfPowderSnowGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        // ClimbOnTopOfPowderSnowGoal.java:24: `this.mob.wasInPowderSnow ||
        // this.mob.isInPowderSnow`.
        let in_powder_snow =
            entity.is_in_powder_snow() || entity.was_in_powder_snow.load(Ordering::Relaxed);
        if !in_powder_snow || !entity.entity_type.has_tag(&tag::EntityType::MINECRAFT_POWDER_SNOW_WALKABLE_MOBS)
        {
            return false;
        }

        let above = entity.block_pos.load().up();
        let world = entity.world.load();
        let block = world.get_block(&above);
        if block.id == Block::POWDER_SNOW.id {
            return true;
        }
        let state = world.get_block_state(&above);
        state.get_block_collision_shapes_at(&above).next().is_none()
    }

    fn tick(&mut self, mob: &dyn Mob) {
        // ClimbOnTopOfPowderSnowGoal.java:41: `this.mob.getJumpControl().jump();`
        mob.get_mob_entity()
            .living_entity
            .jumping
            .store(true, Ordering::SeqCst);
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::JUMP
    }
}
