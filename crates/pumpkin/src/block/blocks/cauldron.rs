use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BlockMetadata, GetComparatorOutputArgs, PathComputationType, UseWithItemArgs,
};
use crate::item::items::bucket::exchange_filled_result;
use crate::plugin::block::cauldron_level_change::CauldronChangeReason;
use pumpkin_data::block_properties::WaterCauldronLikeProperties;
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{BannerPatternsImpl, DyedColorImpl, PotionContentsImpl};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::statistic::{CustomStatistic, StatisticCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_data::{Block, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos};
use pumpkin_world::world::BlockFlags;

pub struct CauldronBlock;

impl BlockMetadata for CauldronBlock {
    fn ids() -> Box<[BlockId]> {
        [
            BlockId::CAULDRON,
            BlockId::WATER_CAULDRON,
            BlockId::LAVA_CAULDRON,
            BlockId::POWDER_SNOW_CAULDRON,
        ]
        .into()
    }
}

/// Filled cauldrons use a union of the basin walls and their current contents.
/// The engine must test this union along the movement, not only at its endpoint.
pub(crate) fn inside_collision_shapes(
    block: &Block,
    state: &BlockState,
    pos: &BlockPos,
) -> Option<Vec<BoundingBox>> {
    if block != &Block::WATER_CAULDRON
        && block != &Block::LAVA_CAULDRON
        && block != &Block::POWDER_SNOW_CAULDRON
    {
        return None;
    }
    let level = fill_level(block, state);
    let mut shapes: Vec<_> = state.get_block_collision_shapes_at(pos).collect();
    shapes.push(BoundingBox::new_array(
        [0.125, 0.25, 0.125],
        [0.875, f64::from(6 + 3 * level) / 16.0, 0.875],
    ));
    Some(shapes)
}

fn fire_cauldron_change(
    world: &std::sync::Arc<crate::world::World>,
    pos: pumpkin_util::math::position::BlockPos,
    old_level: i32,
    new_level: i32,
    reason: crate::plugin::block::cauldron_level_change::CauldronChangeReason,
    entity: Option<std::sync::Arc<dyn crate::entity::EntityBase>>,
) -> bool {
    let mut event = crate::plugin::block::cauldron_level_change::CauldronLevelChangeEvent {
        block_pos: pos,
        world: world.clone(),
        old_level,
        new_level,
        reason,
        entity,
        cancelled: false,
    };
    if let Some(server) = world.server.upgrade() {
        server.plugin_manager.fire_blocking(&server, &mut event);
    }
    !event.cancelled
}

fn fill_state(block: &Block, level: u8) -> BlockStateId {
    if level == 0 {
        return Block::CAULDRON.default_state.id;
    }
    if block == &Block::LAVA_CAULDRON {
        return block.default_state.id;
    }
    let mut props = WaterCauldronLikeProperties::default(block);
    props.level = level;
    props.to_state_id(block)
}

fn fill_level(block: &Block, state: &BlockState) -> u8 {
    match block.id {
        BlockId::WATER_CAULDRON | BlockId::POWDER_SNOW_CAULDRON => {
            WaterCauldronLikeProperties::from_state_id(state.id).level
        }
        BlockId::LAVA_CAULDRON => 3,
        _ => 0,
    }
}

fn change_level(
    world: &Arc<crate::world::World>,
    pos: &BlockPos,
    state: BlockStateId,
    lower: bool,
) {
    world.set_block_state(pos, state, BlockFlags::NOTIFY_ALL);
    if lower {
        world.emit_game_event_from_entity("block_change", pos.to_centered_f64(), None, Some(state));
    }
}

impl BlockBehaviour for CauldronBlock {
    fn on_scheduled_tick(&self, args: crate::block::OnScheduledTickArgs<'_>) {
        let Some(fluid) = super::dripstone::cauldron_drip(args.world, *args.position) else {
            return;
        };
        let state = args.world.get_block_state(args.position);
        if !super::dripstone::accepts_drip(state, fluid) {
            return;
        }
        let old_level = if args.block == &Block::WATER_CAULDRON {
            WaterCauldronLikeProperties::from_state_id(state.id).level
        } else {
            0
        };
        if old_level == 3 {
            return;
        }
        if !fire_cauldron_change(
            args.world,
            *args.position,
            i32::from(old_level),
            if fluid == &pumpkin_data::fluid::Fluid::LAVA {
                3
            } else {
                i32::from(old_level + 1)
            },
            crate::plugin::block::cauldron_level_change::CauldronChangeReason::NaturalFill,
            None,
        ) {
            return;
        }
        let (next, event) = if fluid == &pumpkin_data::fluid::Fluid::LAVA {
            (
                Block::LAVA_CAULDRON.default_state.id,
                pumpkin_data::world::WorldEvent::SoundDripLavaIntoCauldron,
            )
        } else {
            let mut props = WaterCauldronLikeProperties::default(&Block::WATER_CAULDRON);
            props.level = old_level + 1;
            (
                props.to_state_id(&Block::WATER_CAULDRON),
                pumpkin_data::world::WorldEvent::SoundDripWaterIntoCauldron,
            )
        };
        args.world
            .set_block_state(args.position, next, BlockFlags::NOTIFY_ALL);
        args.world.emit_game_event_from_entity(
            "block_change",
            args.position.to_centered_f64(),
            None,
            Some(next),
        );
        args.world.sync_world_event(event, *args.position, 0);
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let item = args.item_stack.item;
        let state = args.world.get_block_state(args.position);
        let level = fill_level(args.block, state);
        let water = args.block == &Block::WATER_CAULDRON;
        let pass = BlockActionResult::PassToDefaultBlockAction;
        let allow = |new_level: u8, reason| {
            fire_cauldron_change(
                args.world,
                *args.position,
                i32::from(level),
                i32::from(new_level),
                reason,
                Some(Arc::clone(args.player) as Arc<dyn crate::entity::EntityBase>),
            )
        };

        // Tag dispatch precedes item dispatch, including custom tag membership.
        if water
            && item
                .is_tagged_with("minecraft:cauldron_can_remove_dye")
                .unwrap_or(false)
        {
            if args
                .item_stack
                .get_data_component::<DyedColorImpl>()
                .is_none()
            {
                return pass;
            }
            if !allow(level - 1, CauldronChangeReason::Unknown) {
                return BlockActionResult::Pass;
            }
            args.item_stack
                .remove_data_component(DataComponent::DyedColor);
            args.player.increment_stat(
                StatisticCategory::Custom,
                CustomStatistic::CleanArmor as i32,
                1,
            );
            change_level(
                args.world,
                args.position,
                fill_state(&Block::WATER_CAULDRON, level - 1),
                true,
            );
            return BlockActionResult::Success;
        }

        // Built-in banner and dyed shulker registrations are item identities, not tags.
        let banner = item.registry_key.ends_with("_banner");
        let dyed_shulker = item.registry_key.ends_with("_shulker_box");
        if water && (banner || dyed_shulker) {
            let mut cleaned = args.item_stack.copy_with_count(1);
            let stat = if banner {
                let Some(mut patterns) = args
                    .item_stack
                    .get_data_component::<BannerPatternsImpl>()
                    .cloned()
                else {
                    return pass;
                };
                if patterns.layers.pop().is_none() {
                    return pass;
                }
                cleaned.set_data_component(patterns);
                CustomStatistic::CleanBanner
            } else {
                cleaned.item = &Item::SHULKER_BOX;
                CustomStatistic::CleanShulkerBox
            };
            if !allow(level - 1, CauldronChangeReason::Unknown) {
                return BlockActionResult::Pass;
            }
            exchange_filled_result(args.player, args.item_stack, cleaned, false);
            args.player
                .increment_stat(StatisticCategory::Custom, stat as i32, 1);
            change_level(
                args.world,
                args.position,
                fill_state(&Block::WATER_CAULDRON, level - 1),
                true,
            );
            return BlockActionResult::Success;
        }

        let (result, next_block, next_level, reason, stat, sound, event, lower) = if item
            == &Item::WATER_BUCKET
            || item == &Item::LAVA_BUCKET
            || item == &Item::POWDER_SNOW_BUCKET
        {
            if item != &Item::WATER_BUCKET {
                let (fluid, _) = args.world.get_fluid_and_fluid_state(&args.position.up());
                if fluid.is_tagged_with("minecraft:water").unwrap_or(false) {
                    return BlockActionResult::Consume;
                }
            }
            let (block, sound) = if item == &Item::WATER_BUCKET {
                (&Block::WATER_CAULDRON, Sound::ItemBucketEmpty)
            } else if item == &Item::LAVA_BUCKET {
                (&Block::LAVA_CAULDRON, Sound::ItemBucketEmptyLava)
            } else {
                (
                    &Block::POWDER_SNOW_CAULDRON,
                    Sound::ItemBucketEmptyPowderSnow,
                )
            };
            (
                ItemStack::new(1, &Item::BUCKET),
                block,
                3,
                CauldronChangeReason::BucketEmpty,
                CustomStatistic::FillCauldron,
                sound,
                "fluid_place",
                false,
            )
        } else if item == &Item::BUCKET && level == 3 {
            let (filled, sound) = match args.block.id {
                BlockId::WATER_CAULDRON => (&Item::WATER_BUCKET, Sound::ItemBucketFill),
                BlockId::LAVA_CAULDRON => (&Item::LAVA_BUCKET, Sound::ItemBucketFillLava),
                BlockId::POWDER_SNOW_CAULDRON => {
                    (&Item::POWDER_SNOW_BUCKET, Sound::ItemBucketFillPowderSnow)
                }
                _ => return pass,
            };
            (
                ItemStack::new(1, filled),
                &Block::CAULDRON,
                0,
                CauldronChangeReason::BucketFill,
                CustomStatistic::UseCauldron,
                sound,
                "fluid_pickup",
                false,
            )
        } else if item == &Item::GLASS_BOTTLE && water {
            let mut potion = ItemStack::new(1, &Item::POTION);
            potion.set_data_component(PotionContentsImpl {
                potion_id: Some(pumpkin_data::potion::Potion::WATER.id as i32),
                custom_color: None,
                custom_effects: Vec::new(),
                custom_name: None,
            });
            (
                potion,
                &Block::WATER_CAULDRON,
                level - 1,
                CauldronChangeReason::BottleFill,
                CustomStatistic::UseCauldron,
                Sound::ItemBottleFill,
                "fluid_pickup",
                true,
            )
        } else if item == &Item::POTION && (water || args.block == &Block::CAULDRON) && level < 3 {
            if !args
                .item_stack
                .get_data_component::<PotionContentsImpl>()
                .is_some_and(|potion| {
                    potion.potion_id == Some(pumpkin_data::potion::Potion::WATER.id as i32)
                        && potion.custom_effects.is_empty()
                })
            {
                return pass;
            }
            (
                ItemStack::new(1, &Item::GLASS_BOTTLE),
                &Block::WATER_CAULDRON,
                level + 1,
                CauldronChangeReason::BottleEmpty,
                CustomStatistic::UseCauldron,
                Sound::ItemBottleEmpty,
                "fluid_place",
                false,
            )
        } else {
            return pass;
        };
        if !allow(next_level, reason) {
            return BlockActionResult::Pass;
        }
        // WATER's potion handler reads the consumed original stack for ITEM_USED.
        let used = if water
            && item == &Item::POTION
            && args.item_stack.item_count == 1
            && args.player.gamemode.load() != pumpkin_util::GameMode::Creative
        {
            &Item::AIR
        } else {
            item
        };
        exchange_filled_result(args.player, args.item_stack, result, true);
        args.player
            .increment_stat(StatisticCategory::Custom, stat as i32, 1);
        args.player
            .increment_stat(StatisticCategory::Used, i32::from(used.id), 1);
        change_level(
            args.world,
            args.position,
            fill_state(next_block, next_level),
            lower,
        );
        args.world.play_sound(
            sound,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
        );
        args.world
            .emit_game_event_from_entity(event, args.position.to_centered_f64(), None, None);
        BlockActionResult::Success
    }

    fn on_entity_collision(&self, args: crate::block::OnEntityCollisionArgs<'_>) {
        let level = fill_level(args.block, args.state);
        if level == 0 {
            return;
        }
        use crate::entity::inside_effects::Effect;
        if args.block == &Block::LAVA_CAULDRON {
            args.effects.lava();
        } else {
            let world = args.world.clone();
            let pos = *args.position;
            args.effects.before(Effect::Extinguish, move |entity| {
                let base = entity.get_entity();
                let may_interact = if let Some(player) = entity.get_player() {
                    !world.is_in_spawn_protection(player, &pos)
                } else {
                    !crate::entity::projectile::is_projectile(base.entity_type)
                        || crate::entity::projectile::may_interact(entity, &world, &pos)
                };
                if base.is_on_fire()
                    && may_interact
                    && fire_cauldron_change(
                        &world,
                        pos,
                        i32::from(level),
                        i32::from(level - 1),
                        CauldronChangeReason::Extinguish,
                        world.get_entity_by_id(base.entity_id),
                    )
                {
                    change_level(
                        &world,
                        &pos,
                        fill_state(&Block::WATER_CAULDRON, level - 1),
                        true,
                    );
                }
            });
            args.effects.apply(Effect::Extinguish);
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        match args.block.id {
            BlockId::WATER_CAULDRON | BlockId::POWDER_SNOW_CAULDRON => {
                let state_id = args.world.get_block_state_id(args.position);
                let props = WaterCauldronLikeProperties::from_state_id(state_id);
                Some(props.level)
            }
            BlockId::LAVA_CAULDRON => Some(3),
            _ => Some(0),
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
