use std::sync::Arc;

use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::BlockStateId;
use pumpkin_data::HorizontalFacingExt;
use pumpkin_data::block_properties::AttachFace;
use pumpkin_data::entity::EntityType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

type ButtonLikeProperties = pumpkin_data::block_properties::LeverLikeProperties;

use crate::block::CanPlaceAtArgs;
use crate::block::EmitsRedstonePowerArgs;
use crate::block::GetRedstonePowerArgs;
use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::OnEntityCollisionArgs;
use crate::block::OnPlaceArgs;
use crate::block::OnProjectileHitArgs;
use crate::block::OnScheduledTickArgs;
use crate::block::OnStateReplacedArgs;
use crate::block::blocks::abstract_wall_mounting::WallMountedBlock;
use crate::block::blocks::redstone::lever::LeverLikePropertiesExt;
use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, NormalUseArgs};
use crate::world::World;

fn get_sound(block: &Block, on: bool) -> Sound {
    if block == &Block::STONE_BUTTON || block == &Block::POLISHED_BLACKSTONE_BUTTON {
        if on {
            Sound::BlockStoneButtonClickOn
        } else {
            Sound::BlockStoneButtonClickOff
        }
    } else if on {
        Sound::BlockWoodenButtonClickOn
    } else {
        Sound::BlockWoodenButtonClickOff
    }
}

pub fn get_ticks_to_stay_pressed(block: &Block) -> u32 {
    if block == &Block::STONE_BUTTON || block == &Block::POLISHED_BLACKSTONE_BUTTON {
        20
    } else {
        30
    }
}

pub fn can_button_be_activated_by_arrows(block: &Block) -> bool {
    block != &Block::STONE_BUTTON && block != &Block::POLISHED_BLACKSTONE_BUTTON
}

fn press_button(
    world: &Arc<World>,
    block_pos: &BlockPos,
    block: &Block,
    mut button_props: ButtonLikeProperties,
) {
    button_props.powered = true;
    world.set_block_state(
        block_pos,
        button_props.to_state_id(block),
        BlockFlags::NOTIFY_ALL,
    );
    let delay = get_ticks_to_stay_pressed(block);
    world.schedule_block_tick(block, *block_pos, delay, TickPriority::Normal);
    ButtonBlock::update_neighbors(world, block_pos, button_props);
    world.play_block_sound(get_sound(block, true), SoundCategory::Blocks, *block_pos);
    world.emit_game_event(
        GameEvent::BlockActivate.name(),
        block_pos.to_centered_f64(),
    );
}

/// Presses the button, unless it is already pressed. Returns whether it was
/// pressed, so callers can tell the two cases apart the way vanilla's
/// `ButtonBlock::useWithoutItem` does.
fn click_button(world: &Arc<World>, block_pos: &BlockPos) -> bool {
    let (block, state) = world.get_block_and_state_id(block_pos);

    let button_props = ButtonLikeProperties::from_state_id(state);
    if button_props.powered {
        return false;
    }

    press_button(world, block_pos, block, button_props);
    true
}

#[pumpkin_block_from_tag("minecraft:buttons")]
pub struct ButtonBlock;

impl BlockBehaviour for ButtonBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if click_button(args.world, args.position) {
            BlockActionResult::Success
        } else {
            BlockActionResult::Consume
        }
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        if can_button_be_activated_by_arrows(args.block) {
            let entity_type = &args.projectile.get_entity().entity_type;
            if *entity_type == &EntityType::ARROW
                || *entity_type == &EntityType::SPECTRAL_ARROW
                || *entity_type == &EntityType::TRIDENT
            {
                let state = args.world.get_block_state(args.position);
                let props = ButtonLikeProperties::from_state_id(state.id);
                if !props.powered {
                    press_button(args.world, args.position, args.block, props);
                }
            }
        }
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if can_button_be_activated_by_arrows(args.block) {
            let entity_type = &args.entity.get_entity().entity_type;
            if *entity_type == &EntityType::ARROW
                || *entity_type == &EntityType::SPECTRAL_ARROW
                || *entity_type == &EntityType::TRIDENT
            {
                let props = ButtonLikeProperties::from_state_id(args.state.id);
                if !props.powered {
                    press_button(args.world, args.position, args.block, props);
                }
            }
        }
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = ButtonLikeProperties::from_state_id(state.id);
        if props.powered {
            let should_be_pressed = if can_button_be_activated_by_arrows(args.block) {
                let aabb = BoundingBox::from_block(args.position);
                args.world
                    .get_entities_at_box(&aabb)
                    .iter()
                    .any(|e| {
                        let entity_type = &e.get_entity().entity_type;
                        *entity_type == &EntityType::ARROW
                            || *entity_type == &EntityType::SPECTRAL_ARROW
                            || *entity_type == &EntityType::TRIDENT
                    })
            } else {
                false
            };

            if should_be_pressed {
                let delay = get_ticks_to_stay_pressed(args.block);
                args.world
                    .schedule_block_tick(args.block, *args.position, delay, TickPriority::Normal);
            } else {
                props.powered = false;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(args.block),
                    BlockFlags::NOTIFY_ALL,
                );
                Self::update_neighbors(args.world, args.position, props);
                args.world.play_block_sound(
                    get_sound(args.block, false),
                    SoundCategory::Blocks,
                    *args.position,
                );
                args.world.emit_game_event(
                    GameEvent::BlockDeactivate.name(),
                    args.position.to_centered_f64(),
                );
            }
        }
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let button_props = ButtonLikeProperties::from_state_id(args.state.id);
        if button_props.powered { 15 } else { 0 }
    }

    fn get_strong_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        let button_props = ButtonLikeProperties::from_state_id(args.state.id);
        if button_props.powered && button_props.get_direction() == args.direction {
            15
        } else {
            0
        }
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if !args.moved {
            let button_props = ButtonLikeProperties::from_state_id(args.old_state_id);
            if button_props.powered {
                Self::update_neighbors(args.world, args.position, button_props);
            }
        }
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = ButtonLikeProperties::from_state_id(args.block.default_state.id);
        (props.face, props.facing) =
            WallMountedBlock::get_placement_face(self, args.player, args.direction);

        props.to_state_id(args.block)
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        // Use the provided direction, or fallback to the current state's direction if missing
        let direction = args
            .direction
            .unwrap_or_else(|| self.get_direction(args.state.id, args.block));

        WallMountedBlock::can_place_at(self, args.block_accessor, args.position, direction)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        WallMountedBlock::get_state_for_neighbor_update(self, args)
    }
}

impl WallMountedBlock for ButtonBlock {
    fn get_direction(&self, state_id: BlockStateId, _block: &Block) -> BlockDirection {
        let props = ButtonLikeProperties::from_state_id(state_id);
        match props.face {
            AttachFace::Floor => BlockDirection::Up,
            AttachFace::Ceiling => BlockDirection::Down,
            AttachFace::Wall => props.facing.to_block_direction(),
        }
    }
}

impl ButtonBlock {
    fn update_neighbors(world: &Arc<World>, block_pos: &BlockPos, props: ButtonLikeProperties) {
        let direction = props.get_direction().opposite();
        world.update_neighbors(block_pos, None);
        world.update_neighbors(&block_pos.offset(direction.to_offset()), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::Block;

    #[test]
    fn button_delays_match_vanilla() {
        // Vanilla: Stone & Polished Blackstone buttons stay pressed for 20 ticks
        assert_eq!(get_ticks_to_stay_pressed(&Block::STONE_BUTTON), 20);
        assert_eq!(
            get_ticks_to_stay_pressed(&Block::POLISHED_BLACKSTONE_BUTTON),
            20
        );

        // Vanilla: Wooden buttons stay pressed for 30 ticks
        assert_eq!(get_ticks_to_stay_pressed(&Block::OAK_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::SPRUCE_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::BIRCH_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::JUNGLE_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::ACACIA_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::DARK_OAK_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::MANGROVE_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::CHERRY_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::BAMBOO_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::CRIMSON_BUTTON), 30);
        assert_eq!(get_ticks_to_stay_pressed(&Block::WARPED_BUTTON), 30);
    }

    #[test]
    fn arrow_activation_capability() {
        // Stone & polished blackstone cannot be activated by arrows
        assert!(!can_button_be_activated_by_arrows(&Block::STONE_BUTTON));
        assert!(!can_button_be_activated_by_arrows(
            &Block::POLISHED_BLACKSTONE_BUTTON
        ));

        // Wooden buttons can be activated by arrows
        assert!(can_button_be_activated_by_arrows(&Block::OAK_BUTTON));
        assert!(can_button_be_activated_by_arrows(&Block::SPRUCE_BUTTON));
        assert!(can_button_be_activated_by_arrows(&Block::DARK_OAK_BUTTON));
    }

    #[test]
    fn button_power_logic() {
        let block = &Block::OAK_BUTTON;
        let mut props = ButtonLikeProperties::default(block);
        props.face = AttachFace::Floor;
        props.powered = false;

        let weak_power = |p: ButtonLikeProperties| if p.powered { 15 } else { 0 };
        let strong_power = |p: ButtonLikeProperties, dir: BlockDirection| {
            if p.powered && p.get_direction() == dir {
                15
            } else {
                0
            }
        };

        assert_eq!(weak_power(props), 0);
        assert_eq!(strong_power(props, BlockDirection::Up), 0);

        props.powered = true;
        assert_eq!(weak_power(props), 15);
        // Strong power is 15 strictly into the attached block (Up for Floor)
        assert_eq!(strong_power(props, BlockDirection::Up), 15);
        assert_eq!(strong_power(props, BlockDirection::Down), 0);
        assert_eq!(strong_power(props, BlockDirection::North), 0);
    }
}

