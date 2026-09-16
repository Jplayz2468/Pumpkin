use std::sync::Arc;

use pumpkin_data::block_properties::BedPart;
use pumpkin_data::entity::{EntityPose, EntityType};
use pumpkin_data::tag::Taggable;
use pumpkin_data::translation;
use pumpkin_data::{Block, BlockState, BlockStateId};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::{boundingbox::BoundingBox, vector3::Vector3};
use pumpkin_world::world::BlockFlags;

use crate::block::OnLandedUponArgs;
use crate::block::UpdateEntityMovementAfterFallOnArgs;
use crate::block::bounce_entity_after_fall;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BrokenArgs, GetStateForNeighborUpdateArgs, NormalUseArgs, OnPlaceArgs,
    PathComputationType, PlayerPlacedArgs,
};
use crate::entity::{EntityBase, player::Player};
use crate::world::World;

type BedProperties = pumpkin_data::block_properties::WhiteBedLikeProperties;

const NO_SLEEP_IDS: &[u16] = &[
    EntityType::BLAZE.id,
    EntityType::BOGGED.id,
    EntityType::SKELETON.id,
    EntityType::STRAY.id,
    EntityType::WITHER_SKELETON.id,
    EntityType::BREEZE.id,
    EntityType::CREAKING.id,
    EntityType::CREEPER.id,
    EntityType::DROWNED.id,
    EntityType::ENDERMITE.id,
    EntityType::EVOKER.id,
    EntityType::GIANT.id,
    EntityType::GUARDIAN.id,
    EntityType::ELDER_GUARDIAN.id,
    EntityType::ILLUSIONER.id,
    EntityType::PIGLIN.id,
    EntityType::PIGLIN_BRUTE.id,
    EntityType::PILLAGER.id,
    EntityType::PARCHED.id,
    EntityType::RAVAGER.id,
    EntityType::SILVERFISH.id,
    EntityType::SPIDER.id,
    EntityType::CAVE_SPIDER.id,
    EntityType::VEX.id,
    EntityType::VINDICATOR.id,
    EntityType::WARDEN.id,
    EntityType::WITCH.id,
    EntityType::WITHER.id,
    EntityType::ZOGLIN.id,
    EntityType::ZOMBIE.id,
    EntityType::ZOMBIE_VILLAGER.id,
    EntityType::HUSK.id,
    EntityType::ENDERMAN.id,
    EntityType::ZOMBIFIED_PIGLIN.id,
];

#[pumpkin_block_from_tag("minecraft:beds")]
pub struct BedBlock;

impl BlockBehaviour for BedBlock {
    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        if let Some(living) = args.entity.get_living_entity() {
            living.handle_fall_damage(args.entity, args.fall_distance * 0.5, 1.0);
        }
    }

    fn update_entity_movement_after_fall_on(&self, args: UpdateEntityMovementAfterFallOnArgs<'_>) {
        bounce_entity_after_fall(args.entity, 0.75);
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut bed_props = BedProperties::default(args.block);

        bed_props.facing = args.player.get_entity().get_horizontal_facing();
        bed_props.part = BedPart::Foot;

        let head = args.position.offset(bed_props.facing.to_offset());
        if !args.world.get_block_state(&head).replaceable()
            || !args
                .world
                .worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains_block(head.0.x, head.0.z)
        {
            return BlockStateId::AIR;
        }
        bed_props.to_state_id(args.block)
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        let mut props = BedProperties::from_state_id(args.state_id);
        props.part = BedPart::Head;
        args.world.set_block_state(
            &args.position.offset(props.facing.to_offset()),
            props.to_state_id(args.block),
            BlockFlags::NOTIFY_ALL,
        );
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let mut props = BedProperties::from_state_id(args.state_id);
        let direction = if props.part == BedPart::Foot {
            props.facing
        } else {
            props.facing.opposite()
        };
        if args.direction.to_offset() == direction.to_offset() {
            if args.neighbor_state_id.to_block() != args.block {
                return BlockStateId::AIR;
            }
            let other = BedProperties::from_state_id(args.neighbor_state_id);
            if other.part == props.part {
                return BlockStateId::AIR;
            }
            props.occupied = other.occupied;
            return props.to_state_id(args.block);
        }
        args.state_id
    }

    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        let props = BedProperties::from_state_id(args.state.id);
        if args.player.gamemode.load() == GameMode::Creative && props.part == BedPart::Foot {
            let head = args.position.offset(props.facing.to_offset());
            let state = args.world.get_block_state_id(&head);
            if state.to_block() == args.block
                && BedProperties::from_state_id(state).part == BedPart::Head
            {
                args.world.set_block_state(
                    &head,
                    BlockStateId::AIR,
                    BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_DROPS,
                );
                let packet = pumpkin_protocol::java::client::play::CWorldEvent::new(
                    pumpkin_data::world::WorldEvent::ParticlesDestroyBlock as i32,
                    head,
                    i32::from(state.as_u16()),
                    false,
                );
                args.world.broadcast_to_chunk_except(
                    head.chunk_position(),
                    &[args.player.gameprofile.id],
                    &packet,
                );
            }
        }
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        Self::use_bed(args.world, args.player, args.block, args.position)
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}

impl BedBlock {
    #[expect(clippy::too_many_lines)]
    fn use_bed(
        world: &Arc<World>,
        player: &Arc<Player>,
        block: &Block,
        position: &BlockPos,
    ) -> BlockActionResult {
        let state_id = world.get_block_state_id(position);
        let bed_props = BedProperties::from_state_id(state_id);

        let (bed_head_pos, bed_foot_pos) = if bed_props.part == BedPart::Head {
            (
                *position,
                position.offset(bed_props.facing.opposite().to_offset()),
            )
        } else {
            (position.offset(bed_props.facing.to_offset()), *position)
        };

        if world.get_block(&bed_head_pos) != block {
            return BlockActionResult::Consume;
        }
        let bed_props = BedProperties::from_state_id(world.get_block_state_id(&bed_head_pos));
        let rule = world
            .environment_attributes()
            .get_value_bed_rule(&bed_head_pos);
        if rule.explodes {
            world.set_block_state(&bed_head_pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
            if world.get_block(&bed_foot_pos) == block {
                world.set_block_state(&bed_foot_pos, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
            }
            world.explode_bad_respawn_point(bed_head_pos.to_centered_f64(), None);
            return BlockActionResult::SuccessServer;
        }

        if bed_props.occupied {
            let bounds = BoundingBox::new(
                bed_head_pos.to_f64(),
                bed_head_pos.to_f64().add_raw(1.0, 1.0, 1.0),
            );
            if let Some(villager) = world
                .get_entities_at_box(&bounds)
                .into_iter()
                .find(|entity| {
                    entity.get_entity().entity_type.id == EntityType::VILLAGER.id
                        && entity.get_entity().pose.load() == EntityPose::Sleeping
                })
            {
                if let Some(home) = villager
                    .get_entity()
                    .synched_data
                    .get::<Option<BlockPos>>(pumpkin_data::tracked_data::villager::SLEEPING_POS_ID)
                    .flatten()
                {
                    let (home_block, state) = world.get_block_and_state(&home);
                    if home_block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_BEDS) {
                        Self::set_occupied(false, world, home_block, &home, state.id);
                        let entity = villager.get_entity();
                        let facing = BedProperties::from_state_id(state.id).facing;
                        let stand = Player::find_bed_spawn_position(
                            world,
                            &home,
                            facing,
                            entity.yaw.load(),
                            entity.entity_type,
                        )
                        .unwrap_or_else(|| home.to_f64().add_raw(0.5, 1.1, 0.5));
                        entity.set_pos(stand);
                        entity.yaw.store(Player::respawn_look_at_yaw(stand, &home));
                        entity.pitch.store(0.0);
                    }
                }
                villager.get_entity().set_pose(EntityPose::Standing);
                villager.get_entity().set_synced_data(
                    pumpkin_data::tracked_data::villager::SLEEPING_POS_ID,
                    None::<BlockPos>,
                );
            } else {
                player.send_system_message_raw(
                    &pumpkin_macros::translate_cross!(
                        translation::java::BLOCK_MINECRAFT_BED_OCCUPIED,
                        translation::bedrock::TILE_BED_OCCUPIED
                    ),
                    true,
                );
            }
            return BlockActionResult::SuccessServer;
        }
        if player.sleeping_since.load().is_some() || player.living_entity.health.load() <= 0.0 {
            return BlockActionResult::SuccessServer;
        }

        let is_dark = world.is_dark_outside();
        let can_sleep = rule.can_sleep(is_dark);
        let can_set_spawn = rule.can_set_spawn(is_dark);

        if !can_set_spawn && !can_sleep {
            player.send_system_message_raw(
                &pumpkin_macros::translate_cross!(
                    translation::java::BLOCK_MINECRAFT_BED_NO_SLEEP,
                    translation::bedrock::TILE_BED_NOSLEEP
                ),
                true,
            );
            return BlockActionResult::SuccessServer;
        }

        // Make sure player is close enough
        if !player.position().is_within_bounds(
            bed_head_pos.to_f64().add_raw(0.5, 0.0, 0.5),
            3.0,
            2.0,
            3.0,
        ) && !player.position().is_within_bounds(
            bed_foot_pos.to_f64().add_raw(0.5, 0.0, 0.5),
            3.0,
            2.0,
            3.0,
        ) {
            player.send_system_message_raw(
                &pumpkin_macros::translate_cross!(
                    translation::java::BLOCK_MINECRAFT_BED_TOO_FAR_AWAY,
                    translation::bedrock::TILE_BED_TOOFAR
                ),
                true,
            );
            return BlockActionResult::SuccessServer;
        }

        // Make sure the bed is not obstructed
        if [bed_head_pos.up(), bed_foot_pos.up()]
            .into_iter()
            .any(|pos| {
                let (block, state) = world.get_block_and_state(&pos);
                bed_is_obstructed(world, &pos, block, state)
            })
        {
            player.send_system_message_raw(
                &pumpkin_macros::translate_cross!(
                    translation::java::BLOCK_MINECRAFT_BED_OBSTRUCTED,
                    translation::bedrock::TILE_BED_OBSTRUCTED
                ),
                true,
            );
            return BlockActionResult::SuccessServer;
        }

        // Set respawn point
        if can_set_spawn
            && player.set_respawn_point(
                world.dimension.clone(),
                bed_head_pos,
                player.get_entity().yaw.load(),
                player.get_entity().pitch.load(),
                false,
            )
        {
            player.send_system_message(&pumpkin_macros::translate_cross!(
                translation::java::BLOCK_MINECRAFT_SET_SPAWN,
                translation::bedrock::TILE_BED_RESPAWNSET
            ));
        }

        // Make sure the time and weather allows sleep
        if !can_sleep {
            player.send_system_message_raw(
                &pumpkin_macros::translate_cross!(
                    translation::java::BLOCK_MINECRAFT_BED_NO_SLEEP,
                    translation::bedrock::TILE_BED_NOSLEEP
                ),
                true,
            );
            return BlockActionResult::SuccessServer;
        }

        let center = bed_head_pos.to_f64().add_raw(0.5, 0.0, 0.5);
        let bounds = BoundingBox::new(
            center - Vector3::new(8.0, 5.0, 8.0),
            center + Vector3::new(8.0, 5.0, 8.0),
        );
        if player.gamemode.load() != GameMode::Creative
            && world
                .get_entities_at_box(&bounds)
                .iter()
                .any(|entity| entity_prevents_sleep(entity.as_ref(), player))
        {
            player.send_system_message_raw(
                &pumpkin_macros::translate_cross!(
                    translation::java::BLOCK_MINECRAFT_BED_NOT_SAFE,
                    translation::bedrock::TILE_BED_NOTSAFE
                ),
                true,
            );
            return BlockActionResult::SuccessServer;
        }

        if let Some(server) = world.server.upgrade() {
            let mut event =
                crate::plugin::api::events::player::player_bed::PlayerBedEnterEvent::new(
                    player.clone(),
                    bed_head_pos,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return BlockActionResult::SuccessServer;
            }
        }

        player.sleep(bed_head_pos);
        player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Custom,
            pumpkin_data::statistic::CustomStatistic::SleepInBed as i32,
            1,
        );
        player.trigger_advancement(
            crate::entity::player::advancement::trigger::AdvancementTrigger::SleptInBed,
        );
        if world
            .level_info
            .load()
            .game_rules
            .players_sleeping_percentage
            > 100
        {
            player.send_system_message_raw(
                &pumpkin_util::text::TextComponent::translate(
                    translation::java::SLEEP_NOT_POSSIBLE,
                    [],
                ),
                true,
            );
        }
        Self::set_occupied(
            true,
            world,
            block,
            &bed_head_pos,
            world.get_block_state_id(&bed_head_pos),
        );

        BlockActionResult::SuccessServer
    }
}

impl BedBlock {
    pub fn set_occupied(
        occupied: bool,
        world: &Arc<World>,
        block: &Block,
        block_pos: &BlockPos,
        state_id: BlockStateId,
    ) {
        let mut bed_props = BedProperties::from_state_id(state_id);
        bed_props.occupied = occupied;
        world.set_block_state(
            block_pos,
            bed_props.to_state_id(block),
            BlockFlags::NOTIFY_ALL,
        );
    }
}

fn entity_prevents_sleep(entity: &dyn EntityBase, player: &Player) -> bool {
    if !NO_SLEEP_IDS.contains(&entity.get_entity().entity_type.id) {
        return false;
    }
    if let Some(piglin) = (entity as &dyn std::any::Any)
        .downcast_ref::<crate::entity::mob::zombified_piglin::ZombifiedPiglinEntity>(
    ) {
        return piglin.is_angry()
            && piglin.mob_entity.get_target().is_some_and(|target| {
                target.get_entity().entity_id == player.get_entity().entity_id
            });
    }
    true
}

// Blocks.java overrides the default suffocation predicate for these built-in families.
fn bed_is_obstructed(world: &World, pos: &BlockPos, block: &Block, state: &BlockState) -> bool {
    if block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_SHULKER_BOXES) {
        return world
            .get_block_entity(pos)
            .and_then(|entity| {
                entity
                    .as_any()
                    .downcast_ref::<crate::block::entities::shulker_box::ShulkerBoxBlockEntity>()
                    .map(|shulker| shulker.is_closed())
            })
            .unwrap_or(true);
    }
    if matches!(block.name, "farmland" | "dirt_path" | "soul_sand" | "mud") {
        return true;
    }
    if block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_LEAVES)
        || matches!(
            block.name,
            "mangrove_roots" | "glass" | "tinted_glass" | "moving_piston"
        )
        || block.name.ends_with("stained_glass")
        || block.name.ends_with("copper_grate")
    {
        return false;
    }
    pumpkin_data::block_properties::blocks_movement(state, block.id) && state.is_full_cube()
}
