use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockState};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

use crate::block::{BlockBehaviour, OnEntityCollisionArgs, OnLandedUponArgs, PathComputationType};
use crate::entity::EntityBase;

#[pumpkin_block("minecraft:powder_snow")]
pub struct PowderSnowBlock;

const FALLING_COLLISION_SHAPE: BoundingBox =
    BoundingBox::new_array([0.0, 0.0, 0.0], [1.0, 0.9, 1.0]);
const WALK_ON_EPSILON: f64 = 1.0e-7;

pub(crate) fn can_entity_walk_on_powder_snow(entity: &dyn EntityBase) -> bool {
    let base = entity.get_entity();
    if base
        .entity_type
        .has_tag(&tag::EntityType::MINECRAFT_POWDER_SNOW_WALKABLE_MOBS)
    {
        return true;
    }

    let Some(living) = entity.get_living_entity() else {
        return false;
    };

    living.entity_equipment.try_lock().is_ok_and(|equipment| {
        equipment
            .equipment
            .get(&EquipmentSlot::FEET)
            .is_some_and(|boots| boots.item == &Item::LEATHER_BOOTS)
    })
}

fn is_entity_above_block(entity: &crate::entity::Entity, position: &BlockPos) -> bool {
    let bb = entity.bounding_box.load();
    let block_top = f64::from(position.0.y) + 1.0;
    bb.min.y >= block_top - WALK_ON_EPSILON
}

fn is_entity_descending(entity: &crate::entity::Entity) -> bool {
    entity.velocity.load().y < 0.0
}

pub(crate) fn collision_shape_for_entity(
    entity: &dyn EntityBase,
    position: &BlockPos,
) -> Option<BoundingBox> {
    let fall_distance = entity
        .get_living_entity()
        .map_or(0.0, |living| living.fall_distance.load());

    if fall_distance > 2.5f32 {
        return Some(FALLING_COLLISION_SHAPE);
    }

    let base = entity.get_entity();
    if base.entity_type == &EntityType::FALLING_BLOCK {
        return Some(BoundingBox::full_block());
    }

    if can_entity_walk_on_powder_snow(entity)
        && is_entity_above_block(base, position)
        && !is_entity_descending(base)
    {
        return Some(BoundingBox::full_block());
    }

    None
}

pub(crate) fn inside_collision_shape_for_entity(
    entity: &dyn EntityBase,
    position: &BlockPos,
) -> BoundingBox {
    collision_shape_for_entity(entity, position).unwrap_or_else(BoundingBox::full_block)
}

impl BlockBehaviour for PowderSnowBlock {
    fn on_landed_upon(&self, args: OnLandedUponArgs<'_>) {
        if let Some(living) = args.entity.get_living_entity()
            && args.fall_distance >= 4.0
        {
            let sound = if args.fall_distance < 7.0 {
                Sound::EntityGenericSmallFall
            } else {
                Sound::EntityGenericBigFall
            };

            living.entity.play_sound(sound);
        }
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        use crate::entity::inside_effects::Effect;
        let entity = args.entity.get_entity();
        if args.entity.get_living_entity().is_none()
            || args.world.get_block(&entity.block_pos.load()) == &Block::POWDER_SNOW
        {
            entity.slow_movement(
                args.state,
                Vector3::new(f64::from(0.9_f32), 1.5, f64::from(0.9_f32)),
            );
        }
        let world = args.world.clone();
        let pos = *args.position;
        args.effects.before(Effect::Extinguish, move |entity| {
            let may_interact = if let Some(player) = entity.get_player() {
                !world.is_in_spawn_protection(player, &pos)
            } else {
                !crate::entity::projectile::is_projectile(entity.get_entity().entity_type)
                    || crate::entity::projectile::may_interact(entity, &world, &pos)
            };
            if entity.get_entity().is_on_fire()
                && may_interact
                && (entity.get_player().is_some()
                    || world.level_info.load().game_rules.mob_griefing)
            {
                world.break_block(&pos, None, BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_DROPS);
            }
        });
        args.effects.apply(Effect::Freeze);
        args.effects.apply(Effect::Extinguish);
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        true
    }
}
