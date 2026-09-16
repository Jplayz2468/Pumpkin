//! Shared LivingEntity climbable-block predicates and ladder movement limits.
use pumpkin_data::block_properties::{
    BlockProperties, LadderLikeProperties, OakTrapdoorLikeProperties,
};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockStateId};
use pumpkin_util::math::vector3::Vector3;

pub fn can_climb(
    state: BlockStateId,
    below: impl FnOnce() -> BlockStateId,
    spectator: bool,
    fall_flying: bool,
) -> bool {
    let block = state.to_block();
    if spectator || (fall_flying && block.has_tag(&tag::Block::MINECRAFT_CAN_GLIDE_THROUGH)) {
        return false;
    }
    if block.has_tag(&tag::Block::MINECRAFT_CLIMBABLE) {
        return true;
    }
    if OakTrapdoorLikeProperties::handles_block_id(block.id) {
        let trapdoor = OakTrapdoorLikeProperties::from_state_id(state);
        if trapdoor.open {
            let below = below();
            return below.to_block() == &Block::LADDER
                && LadderLikeProperties::from_state_id(below).facing == trapdoor.facing;
        }
    }
    false
}

pub fn limit_velocity(mut velocity: Vector3<f64>, hold_ladder: bool) -> Vector3<f64> {
    let max = f64::from(0.15_f32);
    velocity.x = velocity.x.clamp(-max, max);
    velocity.z = velocity.z.clamp(-max, max);
    velocity.y = velocity.y.max(-max);
    if velocity.y < 0.0 && hold_ladder {
        velocity.y = 0.0;
    }
    velocity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn climbable_tags_gliding_and_aligned_open_trapdoors() {
        let air = || BlockStateId::AIR;
        for block in [
            &Block::LADDER,
            &Block::VINE,
            &Block::SCAFFOLDING,
            &Block::CAVE_VINES,
        ] {
            assert!(can_climb(block.default_state.id, air, false, false));
            assert!(!can_climb(block.default_state.id, air, true, false));
            assert_eq!(
                can_climb(block.default_state.id, air, false, true),
                !block.has_tag(&tag::Block::MINECRAFT_CAN_GLIDE_THROUGH)
            );
        }
        for facing in ["north", "south", "east", "west"] {
            let ladder = Block::LADDER
                .from_properties(&[("facing", facing)])
                .to_state_id(&Block::LADDER);
            for block in [
                &Block::OAK_TRAPDOOR,
                &Block::IRON_TRAPDOOR,
                &Block::COPPER_TRAPDOOR,
            ] {
                for open in ["true", "false"] {
                    for trap_facing in ["north", "south", "east", "west"] {
                        let state = block
                            .from_properties(&[("facing", trap_facing), ("open", open)])
                            .to_state_id(block);
                        assert_eq!(
                            can_climb(state, || ladder, false, false),
                            open == "true" && trap_facing == facing
                        );
                        assert!(!can_climb(state, air, false, false));
                    }
                }
            }
        }
    }

    #[test]
    fn java_float_speed_limit_and_player_ladder_hold() {
        let falling = Vector3::new(0.7, -0.8, -0.6);
        let max = f64::from(0.15_f32);
        assert_eq!(
            limit_velocity(falling, false),
            Vector3::new(max, -max, -max)
        );
        assert_eq!(limit_velocity(falling, true), Vector3::new(max, 0.0, -max));
        assert_eq!(limit_velocity(Vector3::new(0.0, 0.2, 0.0), true).y, 0.2);
    }
}
