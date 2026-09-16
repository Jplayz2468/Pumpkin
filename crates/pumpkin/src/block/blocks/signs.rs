use std::sync::{Arc, Mutex};

use crate::block::entities::sign::{SignEntityRef, Text};
use crate::command::CommandSender;
use crate::command::context::command_source::CommandSource;
use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::BlockId;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::EnumVariants;
use pumpkin_data::block_properties::Facing;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{FacingExt, HorizontalFacingExt};
use pumpkin_inventory::screen_handler::InventoryPlayer;
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::text::TextComponent;
use pumpkin_util::text::click::ClickEvent;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockAccessor;
use uuid::Uuid;

use crate::block::BlockBehaviour;
use crate::block::CanPlaceAtArgs;
use crate::block::GetStateForNeighborUpdateArgs;
use crate::block::NormalUseArgs;
use crate::block::OnPlaceArgs;
use crate::block::PlayerPlacedArgs;
use crate::block::UseWithItemArgs;
use crate::block::registry::BlockActionResult;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::item::items::dye::DyeItem;
use crate::item::items::glowing_ink_sac::GlowingInkSacItem;
use crate::item::items::honeycomb::HoneyCombItem;
use crate::item::items::ink_sac::InkSacItem;
use crate::world::World;
use pumpkin_protocol::java::client::play::COpenSignEditor;

#[pumpkin_block_from_tag("minecraft:all_signs")]
pub struct SignBlock;

/// Helper struct for sign placement configuration
struct SignPlacement {
    facing: Option<String>,
    rotation: Option<u8>,
    attached: bool,
    waterlogged: bool,
}

impl SignBlock {
    fn placement_state(args: &OnPlaceArgs<'_>) -> BlockStateId {
        let hanging = args.block.name.contains("hanging");
        let mut directions = args.player.get_entity().get_entity_facing_order();
        if args.replacing == crate::block::BlockIsReplacing::None {
            let face = args.direction.to_facing();
            if let Some(i) = directions.iter().position(|dir| *dir == face) {
                directions.copy_within(0..i, 1);
                directions[0] = face;
            }
        }
        let water = args.world.get_fluid(args.position).id == Fluid::WATER.id;
        let wall = Block::from_id(get_sign_variant(args.block, hanging));
        let wall_state = directions.iter().find_map(|dir| {
            let horizontal = dir.to_horizontal_facing()?;
            if hanging && horizontal.to_block_direction().to_axis() == args.direction.to_axis() {
                return None;
            }
            let facing = horizontal.opposite().to_block_direction();
            let state = Self::apply_placement_properties(
                wall,
                &SignPlacement {
                    facing: Some(facing.to_cardinal_direction().to_value().to_string()),
                    rotation: None,
                    attached: false,
                    waterlogged: water,
                },
            );
            let valid = if hanging {
                wall_hanging_can_place(args.world, args.position, facing)
            } else {
                args.world
                    .get_block_state(&args.position.offset(horizontal.to_offset()))
                    .is_solid()
            };
            valid.then_some(state)
        });
        let attached_direction = if hanging { Facing::Up } else { Facing::Down };
        for dir in directions {
            if dir == attached_direction.opposite() {
                continue;
            }
            if dir != attached_direction {
                if let Some(state) = wall_state {
                    return state;
                }
                continue;
            }
            let mut attached = false;
            let mut rotation = args.player.get_entity().get_flipped_rotation_16();
            if hanging {
                let (above_block, above) = args.world.get_block_and_state(&args.position.up());
                if !above.is_center_solid(BlockDirection::Down) {
                    continue;
                }
                let sneaking = args.player.get_entity().is_sneaking();
                attached = !above.is_side_solid(BlockDirection::Down) || sneaking;
                let horizontal = args
                    .player
                    .get_entity()
                    .get_horizontal_facing()
                    .to_block_direction();
                if above_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_ALL_HANGING_SIGNS)
                    && !sneaking
                {
                    if let Some(support) = get_wall_support_direction(above_block, above.id) {
                        if support.to_axis() == horizontal.to_axis() {
                            attached = false;
                        }
                    } else if let Some(props) = above_block.properties(above.id) {
                        if let Some((_, value)) =
                            props.to_props().iter().find(|(key, _)| *key == "rotation")
                        {
                            let segment = value.parse::<u8>().unwrap_or(0);
                            if segment % 4 == 0 {
                                let above_axis = if segment % 8 == 0 {
                                    pumpkin_data::block_properties::Axis::Z
                                } else {
                                    pumpkin_data::block_properties::Axis::X
                                };
                                if above_axis == horizontal.to_axis() {
                                    attached = false;
                                }
                            }
                        }
                    }
                }
                if !attached {
                    rotation = match horizontal.opposite() {
                        BlockDirection::South => 0,
                        BlockDirection::West => 4,
                        BlockDirection::North => 8,
                        _ => 12,
                    };
                }
            } else if !args.world.get_block_state(&args.position.down()).is_solid() {
                continue;
            }
            return Self::apply_placement_properties(
                args.block,
                &SignPlacement {
                    facing: None,
                    rotation: Some(rotation),
                    attached,
                    waterlogged: water,
                },
            );
        }
        BlockStateId::AIR
    }

    /// Applies placement properties to a block.
    fn apply_placement_properties(block: &Block, placement: &SignPlacement) -> BlockStateId {
        let mut props = block
            .properties(block.default_state.id)
            .map(|p| p.to_props())
            .unwrap_or_default();

        if let Some(facing) = &placement.facing
            && let Some(prop) = props.iter_mut().find(|(k, _)| *k == "facing")
        {
            prop.1 = facing;
        }

        if let Some(rotation) = placement.rotation
            && let Some(prop) = props.iter_mut().find(|(k, _)| *k == "rotation")
        {
            prop.1 = match rotation {
                1 => "1",
                2 => "2",
                3 => "3",
                4 => "4",
                5 => "5",
                6 => "6",
                7 => "7",
                8 => "8",
                9 => "9",
                10 => "10",
                11 => "11",
                12 => "12",
                13 => "13",
                14 => "14",
                15 => "15",
                _ => "0",
            };
        }

        if let Some(prop) = props.iter_mut().find(|(k, _)| *k == "attached") {
            prop.1 = if placement.attached { "true" } else { "false" };
        }

        if let Some(prop) = props.iter_mut().find(|(k, _)| *k == "waterlogged") {
            prop.1 = if placement.waterlogged {
                "true"
            } else {
                "false"
            };
        }

        block.from_properties(&props).to_state_id(block)
    }
}

impl BlockBehaviour for SignBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        Self::placement_state(&args)
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(sign) = SignEntityRef::from_block_entity(&*block_entity)
        {
            open_text_edit(
                args.player,
                sign.currently_editing_player(),
                args.position,
                true,
            );
            return;
        }
        args.player
            .try_send_client_packet(&COpenSignEditor::new(*args.position, true));
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        if args.block.name.contains("wall_hanging") {
            get_wall_support_direction(args.block, args.state.id).is_some_and(|support| {
                wall_hanging_can_place(args.block_accessor, args.position, support.opposite())
            })
        } else {
            sign_can_survive(
                args.block_accessor,
                args.block,
                args.state.id,
                args.position,
            )
        }
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let support = if args.block.name.contains("wall_hanging") {
            None
        } else if args.block.name.contains("hanging") {
            Some(BlockDirection::Up)
        } else if args.block.name.contains("wall") {
            get_wall_support_direction(args.block, args.state_id)
        } else {
            Some(BlockDirection::Down)
        };
        if support == Some(args.direction)
            && !sign_can_survive(args.world, args.block, args.state_id, args.position)
        {
            return BlockStateId::AIR;
        }
        if args.state_id.is_waterlogged() {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u32,
                TickPriority::Normal,
            );
        }
        args.state_id
    }

    /// Handles normal use (right-click) on the sign block.
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(sign_entity) = SignEntityRef::from_block_entity(&*block_entity) else {
            return BlockActionResult::Pass;
        };

        let is_front_text =
            is_facing_front_text(args.world, args.position, args.block, args.player);
        let text = sign_entity.get_text(is_front_text);

        let executed_click_command =
            execute_click_commands_if_present(args.world, args.player, args.position, text);

        if sign_entity.is_waxed() {
            let is_hanging = args.block.name.contains("hanging");
            let sound = if is_hanging {
                pumpkin_data::sound::Sound::BlockHangingSignWaxedInteractFail
            } else {
                pumpkin_data::sound::Sound::BlockSignWaxedInteractFail
            };
            args.world.play_block_sound(
                sound,
                pumpkin_data::sound::SoundCategory::Blocks,
                *args.position,
            );
            BlockActionResult::SuccessServer
        } else if executed_click_command {
            BlockActionResult::SuccessServer
        } else if !other_player_is_editing_sign(
            args.player,
            sign_entity.currently_editing_player(),
            args.world,
            args.position,
        ) && args.player.may_build()
            && has_editable_text(text, args.player)
        {
            open_text_edit(
                args.player,
                sign_entity.currently_editing_player(),
                args.position,
                is_front_text,
            );
            BlockActionResult::SuccessServer
        } else {
            BlockActionResult::Pass
        }
    }

    /// Handles use with an item on the sign block.
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return BlockActionResult::Pass;
        };
        let Some(sign_entity) = SignEntityRef::from_block_entity(&*block_entity) else {
            return BlockActionResult::Pass;
        };

        let Some(pumpkin_item) = args
            .server
            .item_registry
            .get_pumpkin_item(args.item_stack.item.id)
        else {
            return BlockActionResult::PassToDefaultBlockAction;
        };

        let is_applicator = pumpkin_item.as_any().is::<HoneyCombItem>()
            || pumpkin_item.as_any().is::<GlowingInkSacItem>()
            || pumpkin_item.as_any().is::<InkSacItem>()
            || pumpkin_item.as_any().is::<DyeItem>();

        let has_applicator_to_use = is_applicator && args.player.may_build();

        if has_applicator_to_use
            && !sign_entity.is_waxed()
            && !other_player_is_editing_sign(
                args.player,
                sign_entity.currently_editing_player(),
                args.world,
                args.position,
            )
        {
            let is_front_text =
                is_facing_front_text(args.world, args.position, args.block, args.player);
            let text = sign_entity.get_text(is_front_text);

            // SignBlock.java:97-100 — `HoneycombItem` overrides `canApplyToSign` to always
            // return `true` (waxing a blank sign is allowed), but every other
            // `SignApplicator` (ink sac, glow ink sac, dye) relies on the interface's default
            // `canApplyToSign`, which is `SignText.hasMessage(player)`: applying dye/ink to a
            // sign with no text on the clicked face is refused so the click falls through to
            // opening the text editor instead of silently consuming the item.
            let result = if let Some(honeycomb_item) =
                pumpkin_item.as_any().downcast_ref::<HoneyCombItem>()
            {
                honeycomb_item.apply_to_sign(&args, &block_entity, &sign_entity)
            } else if !sign_has_message(text, args.player) {
                BlockActionResult::PassToDefaultBlockAction
            } else if let Some(g_ink_sac_item) =
                pumpkin_item.as_any().downcast_ref::<GlowingInkSacItem>()
            {
                g_ink_sac_item.apply_to_sign(&args, &block_entity, text)
            } else if let Some(ink_sac_item) = pumpkin_item.as_any().downcast_ref::<InkSacItem>() {
                ink_sac_item.apply_to_sign(&args, &block_entity, text)
            } else if let Some(dye) = pumpkin_item.as_any().downcast_ref::<DyeItem>() {
                let color_name = args
                    .item_stack
                    .item
                    .registry_key
                    .strip_suffix("_dye")
                    .unwrap_or(args.item_stack.item.registry_key);
                dye.apply_to_sign(&args, &block_entity, text, color_name)
            } else {
                BlockActionResult::PassToDefaultBlockAction
            };

            if result == BlockActionResult::Success {
                execute_click_commands_if_present(args.world, args.player, args.position, text);
                if pumpkin_item.as_any().is::<GlowingInkSacItem>() {
                    args.player.trigger_advancement(
                        crate::entity::player::advancement::trigger::AdvancementTrigger::GlowedSign,
                    );
                }
                // SignBlock.java:109-111 — `awardStat`/`gameEvent(BLOCK_CHANGE)` are fired by
                // the block generically on every successful applicator use, not by the
                // individual `SignApplicator` (ink sac/glow ink sac/dye/honeycomb all share
                // this, matching the item-use stat and world event on the block).
                args.player.increment_stat(
                    pumpkin_data::statistic::StatisticCategory::Used,
                    i32::from(args.item_stack.item.id),
                    1,
                );
                args.world.emit_game_event_with_context(
                    pumpkin_data::game_event::GameEvent::BlockChange.name(),
                    args.position.to_centered_f64(),
                    Some(args.player.get_entity().entity_id),
                    Some(args.world.get_block_state_id(args.position)),
                );
                if !args.player.has_infinite_materials() {
                    args.item_stack.decrement(1);
                }
                return BlockActionResult::Success;
            }
        }

        BlockActionResult::PassToDefaultBlockAction
    }
}

fn wall_hanging_can_place(
    world: &dyn BlockAccessor,
    pos: &BlockPos,
    facing: BlockDirection,
) -> bool {
    let axis = facing.to_axis();
    let sides = if axis == pumpkin_data::block_properties::Axis::Z {
        [BlockDirection::West, BlockDirection::East]
    } else {
        [BlockDirection::North, BlockDirection::South]
    };
    sides.iter().any(|side| {
        let (block, state) = world.get_block_and_state(&pos.offset(side.to_offset()));
        if block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_WALL_HANGING_SIGNS) {
            get_wall_support_direction(block, state.id).is_some_and(|other| other.to_axis() == axis)
        } else {
            state.is_side_solid(side.opposite())
        }
    })
}

fn sign_can_survive(
    world: &dyn BlockAccessor,
    block: &Block,
    state: BlockStateId,
    pos: &BlockPos,
) -> bool {
    if block.name.contains("wall_hanging") {
        true
    } else if block.name.contains("hanging") {
        world
            .get_block_state(&pos.up())
            .is_center_solid(BlockDirection::Down)
    } else if block.name.contains("wall") {
        get_wall_support_direction(block, state).is_some_and(|dir| {
            world
                .get_block_state(&pos.offset(dir.to_offset()))
                .is_solid()
        })
    } else {
        world.get_block_state(&pos.down()).is_solid()
    }
}

/// Opens the sign text edit interface for the player and registers them as allowed editor.
fn open_text_edit(
    player: &Player,
    currently_editing_player: &Arc<Mutex<Option<Uuid>>>,
    position: &BlockPos,
    is_front_text: bool,
) {
    *currently_editing_player
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(player.gameprofile.id);
    player.try_send_client_packet(&COpenSignEditor::new(*position, is_front_text));
}

/// Editor ownership is released by the sign entity tick.
fn other_player_is_editing_sign(
    player: &Player,
    currently_editing_player: &Arc<Mutex<Option<Uuid>>>,
    _world: &World,
    _position: &BlockPos,
) -> bool {
    currently_editing_player
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some_and(|editor| editor != player.gameprofile.id)
}

/// `SignText.hasMessage`: whether the given sign text face has at least one non-empty
/// message line, using the filtered lines when the player has text filtering enabled.
/// This gates every `SignApplicator` other than the honeycomb (which overrides it to
/// always allow waxing), mirroring `SignApplicator::canApplyToSign`'s default impl.
fn sign_has_message(text: &Text, player: &Player) -> bool {
    let messages = if player.is_text_filtering_enabled() {
        &text.filtered_messages
    } else {
        &text.messages
    };
    let messages = messages
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    messages.iter().any(|msg| {
        serde_json::from_str::<TextComponent>(msg).map_or_else(
            |_| !msg.is_empty(),
            |component| !component.get_text().is_empty(),
        )
    })
}

/// Checks whether all messages on the given sign text face are plain text or empty.
fn has_editable_text(text: &Text, player: &Player) -> bool {
    let messages = if player.is_text_filtering_enabled() {
        &text.filtered_messages
    } else {
        &text.messages
    };
    let messages = messages
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    messages.iter().all(|msg| is_plain_or_empty_text(msg))
}

fn is_plain_or_empty_text(text: &str) -> bool {
    if text.is_empty() {
        return true;
    }
    if !text.starts_with('{') {
        return true;
    }
    match serde_json::from_str::<TextComponent>(text) {
        Ok(component) => matches!(
            *component.0.content,
            pumpkin_util::text::TextContent::Text { .. }
        ),
        Err(_) => true,
    }
}

/// Executes any `run_command` click events defined in the sign's text messages.
fn execute_click_commands_if_present(
    world: &Arc<World>,
    player: &Arc<Player>,
    position: &BlockPos,
    text: &Text,
) -> bool {
    let Some(server) = world.server.upgrade() else {
        return false;
    };

    let messages = if player.is_text_filtering_enabled() {
        &text.filtered_messages
    } else {
        &text.messages
    };
    let messages = messages
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();

    let mut has_run_command = false;
    for msg in messages.iter() {
        if msg.is_empty() || !msg.starts_with('{') {
            continue;
        }
        if let Ok(component) = serde_json::from_str::<TextComponent>(msg)
            && let Some(ClickEvent::RunCommand { command }) = &component.0.style.click_event
        {
            let source = CommandSource::new(
                CommandSender::Dummy,
                world.clone(),
                Some(player.clone()),
                position.to_centered_f64(),
                Vector2::new(0.0, 0.0),
                player.gameprofile.name.clone(),
                player.get_display_name(),
                server.clone(),
            );
            let command_str = command.strip_prefix('/').unwrap_or(command);
            let dispatcher = server.command_dispatcher.load();
            dispatcher.handle_command(&source, command_str);
            has_run_command = true;
        }
    }
    has_run_command
}

/// Returns the direction of the block supporting the wall sign.
fn get_wall_support_direction(block: &Block, state_id: BlockStateId) -> Option<BlockDirection> {
    block.properties(state_id).and_then(|props| {
        let prop_map = props.to_props();
        prop_map
            .into_iter()
            .find(|(k, _)| k == &"facing")
            .map(|(_, v)| match v {
                "north" => BlockDirection::South,
                "south" => BlockDirection::North,
                "east" => BlockDirection::West,
                _ => BlockDirection::East, // "west" and default case
            })
    })
}

/// Helper to convert a regular sign to its wall variant.
/// Returns the block ID of the wall variant, or the base block's ID if not found.
fn get_sign_variant(base: &Block, is_hanging: bool) -> BlockId {
    let base_name = base.name;
    let wood_type = base_name
        .strip_suffix("_hanging_sign")
        .or_else(|| base_name.strip_suffix("_sign"))
        .unwrap_or("oak");

    let target_name = if is_hanging {
        // This is the variant that provides the "horizontal wooden post"
        format!("{wood_type}_wall_hanging_sign")
    } else {
        format!("{wood_type}_wall_sign")
    };

    pumpkin_data::Block::from_name(&target_name).map_or(base.id, |b| b.id)
}

fn is_facing_front_text(
    world: &World,
    location: &BlockPos,
    block: &Block,
    player: &Player,
) -> bool {
    let state_id = world.get_block_state_id(location);
    // Read properties dynamically: some sign types use a `rotation` property (0..15),
    // others (wall signs) use a `facing` property (north/south/west/east),
    // hanging signs may have `rotation` + `attached`.
    let mut rotation: f32 = 0.0;
    if let Some(props) = block.properties(state_id) {
        let prop_map = props.to_props();
        if let Some((_, val)) = prop_map.iter().find(|(k, _)| k == &"rotation") {
            let r = val.parse().unwrap_or(0);
            rotation = get_yaw_from_rotation_16(r);
        } else if let Some((_, val)) = prop_map.iter().find(|(k, _)| k == &"facing") {
            rotation = match &val[..] {
                "north" => 180.0,
                "west" => 90.0,
                "east" => -90.0,
                _ => 0.0,
            };
        }
    }
    let mut bounding_box = Vector3::new(0.5, 0.5, 0.5);
    if block.name.contains("wall_sign") && !block.name.contains("hanging") {
        if let Some(support) = get_wall_support_direction(block, state_id) {
            bounding_box = bounding_box.add(&(support.to_offset().to_f64() * (7.0 / 16.0)));
        }
    }

    let d = player.eye_position().x - (f64::from(location.0.x) + bounding_box.x);
    let d1 = player.eye_position().z - (f64::from(location.0.z) + bounding_box.z);

    let f = (d1.atan2(d).to_degrees() as f32) - 90.0;

    let diff = (f - rotation + 180.0).rem_euclid(360.0) - 180.0;
    diff.abs() <= 90.0
}

fn get_yaw_from_rotation_16(rotation: u8) -> f32 {
    f32::from(rotation) * 22.5
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(_block: &Block, waterlogged: bool) -> SignPlacement {
        SignPlacement {
            facing: None,
            rotation: None,
            attached: false,
            waterlogged,
        }
    }

    /// `apply_placement_properties` matches property names as strings, so a
    /// renamed or missing `waterlogged` would silently stop waterlogging signs.
    #[test]
    fn placement_carries_waterlogging_into_the_state() {
        for block in [
            &Block::OAK_SIGN,
            &Block::OAK_WALL_SIGN,
            &Block::OAK_HANGING_SIGN,
            &Block::OAK_WALL_HANGING_SIGN,
        ] {
            for waterlogged in [false, true] {
                let state_id =
                    SignBlock::apply_placement_properties(block, &placement(block, waterlogged));
                assert_eq!(
                    state_id.is_waterlogged(),
                    waterlogged,
                    "{} placed with waterlogged={waterlogged}",
                    block.name
                );
            }
        }
    }
}
