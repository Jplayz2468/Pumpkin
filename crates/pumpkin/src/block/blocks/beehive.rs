use crate::block::entities::beehive::BeehiveBlockEntity;
use crate::block::{BlockBehaviour, BlockMetadata, GetComparatorOutputArgs};
use crate::entity::EntityBase;
use crate::entity::mob::Mob;
use crate::entity::passive::bee::BeeEntity;
use crate::world::World;
use rand::RngExt;
use pumpkin_data::block_properties::{BeeNestLikeProperties, CampfireLikeProperties};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockId, BlockState};
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use std::sync::Arc;

pub struct BeehiveBlock;

impl BlockMetadata for BeehiveBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BEEHIVE, BlockId::BEE_NEST].into()
    }
}

impl BlockBehaviour for BeehiveBlock {
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        {
            let state_id = args.world.get_block_state_id(args.position);
            let props = BeeNestLikeProperties::from_state_id(state_id);
            Some(props.honey_level)
        }
    }
}

/// Mirrors `CampfireBlock.isSmokeyPos` (CampfireBlock.java:266-280). Walks up to 5 blocks
/// below `pos` looking for a lit campfire; if a block fully blocks the column first, only the
/// single block below *that* is checked before giving up. Vanilla blocks smoke with a precise
/// collision-shape intersection test against a thin vertical "post" shape — this approximates
/// that with `BlockState::is_full_cube`, which covers the common solid-floor case but is not a
/// pixel-perfect port.
pub fn is_smokey_pos(world: &World, pos: &BlockPos) -> bool {
    for i in 1..=5 {
        let check_pos = pos.down_height(i);
        let (block, state) = world.get_block_and_state(&check_pos);
        if is_lit_campfire(block, state) {
            return true;
        }
        if state.is_full_cube() {
            let (below_block, below_state) = world.get_block_and_state(&check_pos.down());
            return is_lit_campfire(below_block, below_state);
        }
    }
    false
}

fn is_lit_campfire(block: &'static Block, state: &'static BlockState) -> bool {
    block.has_tag(&tag::Block::MINECRAFT_CAMPFIRES)
        && CampfireLikeProperties::from_state_id(state.id).lit
}

/// Mirrors `BeehiveBlock.angerNearbyBees` (BeehiveBlock.java:118-132): every `Bee` in a
/// 17x13x17 box centered on the hive (`AABB(pos).inflate(8.0, 6.0, 8.0)`) that has no current
/// target is set to attack a random player also found in that box.
pub fn anger_nearby_bees(world: &World, pos: &BlockPos) {
    let center = pos.to_centered_f64();
    let radius = Vector3::new(8.5, 6.5, 8.5);
    let aabb = BoundingBox::new(center - radius, center + radius);

    let bees: Vec<Arc<dyn EntityBase>> = world
        .get_entities_at_box(&aabb)
        .into_iter()
        .filter(|e| e.cast_any().downcast_ref::<BeeEntity>().is_some())
        .collect();
    if bees.is_empty() {
        return;
    }

    let players = world.get_players_at_box(&aabb);
    if players.is_empty() {
        return;
    }

    for bee_entity in &bees {
        let Some(bee) = bee_entity.cast_any().downcast_ref::<BeeEntity>() else {
            continue;
        };
        if bee.mob_entity.get_target().is_some() {
            continue;
        }
        let target = players[rand::rng().random_range(0..players.len())].clone();
        bee.set_mob_target(Some(target as Arc<dyn EntityBase>));
    }
}

/// Approximates `BeehiveBlockEntity.isEmpty()` (BeehiveBlockEntity.java:117-119) as "does the
/// hive's stored `bees` NBT list have any entries". Pumpkin keeps that list as opaque NBT
/// (`BeehiveBlockEntity::bees` in `block/entities/beehive.rs`) rather than decoding it into live
/// occupants, so this cannot distinguish stale/expired entries the way vanilla's occupant list
/// does; it only gates whether nearby bees should be angered when a hive is sheared.
pub fn hive_contains_bees(world: &World, pos: &BlockPos) -> bool {
    world
        .get_block_entity(pos)
        .and_then(|block_entity| {
            let hive = block_entity
                .as_any()
                .downcast_ref::<BeehiveBlockEntity>()?;
            let guard = hive.bees.lock().ok()?;
            Some(guard.as_ref().is_some_and(|list| !list.is_empty()))
        })
        .unwrap_or(false)
}
