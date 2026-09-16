use pumpkin_data::BlockDirection;
use pumpkin_data::block_properties::{Axis, TargetProperties};
use pumpkin_data::entity::EntityType;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

use crate::block::{
    BlockBehaviour, EmitsRedstonePowerArgs, GetRedstonePowerArgs, OnProjectileHitArgs,
    OnScheduledTickArgs, PlacedArgs,
};

fn get_redstone_strength(hit_pos: &Vector3<f64>, face: BlockDirection) -> u8 {
    let dist_x = ((hit_pos.x - hit_pos.x.floor()) - 0.5).abs();
    let dist_y = ((hit_pos.y - hit_pos.y.floor()) - 0.5).abs();
    let dist_z = ((hit_pos.z - hit_pos.z.floor()) - 0.5).abs();

    let distance = match face.to_axis() {
        Axis::Y => dist_x.max(dist_z),
        Axis::Z => dist_x.max(dist_y),
        Axis::X => dist_y.max(dist_z),
    };

    let norm = ((0.5 - distance) / 0.5).clamp(0.0, 1.0);
    let power = (15.0 * norm).ceil() as u8;
    power.clamp(1, 15)
}

#[pumpkin_block("minecraft:target")]
pub struct TargetBlock;

impl BlockBehaviour for TargetBlock {
    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let props = TargetProperties::from_state_id(args.state.id);
        props.r#power
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        let power = get_redstone_strength(args.hit_pos, args.hit_face);
        if !args
            .world
            .is_block_tick_scheduled(args.position, args.block)
        {
            let mut props = TargetProperties::from_state_id(args.state.id);
            props.r#power = power;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
            let entity_type = args.projectile.get_entity().entity_type;
            let delay = if entity_type == &EntityType::ARROW
                || entity_type == &EntityType::SPECTRAL_ARROW
                || entity_type == &EntityType::TRIDENT
            {
                20
            } else {
                8
            };
            args.world
                .schedule_block_tick(args.block, *args.position, delay, TickPriority::Normal);
        }
        if let Some(player) = args.projectile.get_projectile_owner_player() {
            player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::TargetHit as i32,
                1,
            );
            let owner = player.living_entity.entity.pos.load();
            let projectile = args.projectile.get_entity().pos.load();
            let dx = projectile.x - owner.x;
            let dz = projectile.z - owner.z;
            if power == 15 && dx * dx + dz * dz >= 900.0 {
                player.trigger_advancement(
                    crate::entity::player::advancement::trigger::AdvancementTrigger::Bullseye,
                );
            }
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state_id = args.world.get_block_state_id(args.position);
        let mut props = TargetProperties::from_state_id(state_id);
        if props.r#power != 0 {
            props.r#power = 0;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        let mut props = TargetProperties::from_state_id(args.state_id);
        if props.r#power > 0
            && !args
                .world
                .is_block_tick_scheduled(args.position, args.block)
        {
            props.r#power = 0;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS | BlockFlags::SKIP_SHAPE_UPDATES,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_bullseye_gives_15() {
        // Bullseye exactly at center (0.5, 0.5, 0.5)
        let hit = Vector3::new(10.5, 64.5, 20.0);
        assert_eq!(get_redstone_strength(&hit, BlockDirection::North), 15);
    }

    #[test]
    fn test_target_edge_gives_at_least_1() {
        // Edge hit at boundary
        let hit = Vector3::new(10.99, 64.99, 20.0);
        let strength = get_redstone_strength(&hit, BlockDirection::North);
        assert!(strength >= 1 && strength <= 15);
    }
}
