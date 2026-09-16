use pumpkin_data::{advancement::Advancement, entity::EntityStatus, sound::Sound};
use pumpkin_macros::pumpkin_block;
use std::sync::atomic::Ordering;

use crate::block::{
    BlockBehaviour, OnEntityCollisionArgs, OnLandedUponArgs, PathComputationType,
    UpdateEntityMovementAfterFallOnArgs, stop_vertical_movement_after_fall,
};

#[pumpkin_block("minecraft:honey_block")]
pub struct HoneyBlock;

impl BlockBehaviour for HoneyBlock {
    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        let entity = args.entity.get_entity();
        entity.play_sound(Sound::BlockHoneyBlockSlide);
        args.world
            .send_entity_status(entity, EntityStatus::HoneyJump, None);
        if let Some(living) = args.entity.get_living_entity()
            && living.apply_fall_damage_with_type(
                args.entity,
                args.fall_distance,
                0.2,
                pumpkin_data::damage::DamageType::FALL,
            )
        {
            args.world.play_sound_fine(
                Sound::BlockHoneyBlockFall,
                pumpkin_data::sound::SoundCategory::Neutral,
                &entity.pos.load(),
                0.5,
                0.75,
            );
        }
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        let entity = args.entity.get_entity();
        let pos = entity.pos.load();
        let block_pos = args.position.0;
        let velocity = entity.velocity.load();
        let old_y = velocity.y / f64::from(0.98_f32) + 0.08;
        let overlap = 0.4375 + f64::from(entity.entity_dimension.load().width / 2.0);
        if entity.on_ground.load(Ordering::Relaxed)
            || pos.y > f64::from(block_pos.y) + 0.9375 - 1.0e-7
            || old_y >= -0.08
            || ((f64::from(block_pos.x) + 0.5 - pos.x).abs() + 1.0e-7 <= overlap
                && (f64::from(block_pos.z) + 0.5 - pos.z).abs() + 1.0e-7 <= overlap)
        {
            return;
        }
        if args.world.get_world_age() % 20 == 0
            && let Some(player) = args.entity.get_player()
        {
            player.trigger_advancement_criterion(
                Advancement::ADVENTURE_HONEY_BLOCK_SLIDE,
                "honey_block_slide",
            );
        }
        let horizontal_scale = if old_y < -0.13 { -0.05 / old_y } else { 1.0 };
        let mut slid = velocity.multiply(horizontal_scale, 1.0, horizontal_scale);
        slid.y = (-0.05 - 0.08) * f64::from(0.98_f32);
        entity.velocity.store(slid);
        if let Some(living) = args.entity.get_living_entity() {
            living.fall_distance.store(0.0);
        } else if let Some(falling) = args
            .entity
            .cast_any()
            .downcast_ref::<crate::entity::falling::FallingEntity>()
        {
            falling.reset_fall_distance();
        }
        let name = entity.entity_type.resource_name;
        if args.entity.get_living_entity().is_some()
            || name.ends_with("minecart")
            || name.ends_with("boat")
            || name.ends_with("raft")
            || name == "tnt"
        {
            if args.world.rand_bounded_i32(5) == 0 {
                entity.play_sound(Sound::BlockHoneyBlockSlide);
            }
            if args.world.rand_bounded_i32(5) == 0 {
                args.world
                    .send_entity_status(entity, EntityStatus::HoneySlide, None);
            }
        }
    }

    fn update_entity_movement_after_fall_on(&self, args: UpdateEntityMovementAfterFallOnArgs<'_>) {
        stop_vertical_movement_after_fall(args.entity);
    }

    fn is_pathfindable(
        &self,
        _state: &pumpkin_data::BlockState,
        _computation_type: PathComputationType,
    ) -> bool {
        false
    }
}
