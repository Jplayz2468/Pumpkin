use std::sync::Arc;

use crate::block::BlockBehaviour;
use crate::block::OnEntityCollisionArgs;
use pumpkin_data::dimension::Dimension;
use pumpkin_macros::pumpkin_block;

#[pumpkin_block("minecraft:end_portal")]
pub struct EndPortalBlock;

impl BlockBehaviour for EndPortalBlock {
    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        if !portal_eligible(args.entity) {
            return;
        }

        let target_world =
            if args.world.dimension.minecraft_name == Dimension::THE_END.minecraft_name {
                args.server.get_world_from_dimension(&Dimension::OVERWORLD)
            } else {
                args.server.get_world_from_dimension(&Dimension::THE_END)
            };
        if Arc::ptr_eq(&target_world, args.world) {
            return;
        }
        args.entity
            .get_entity()
            .try_use_portal(target_world, *args.position);
    }

    fn get_inside_collision_shape(
        &self,
        _args: crate::block::GetInsideCollisionShapeArgs<'_>,
    ) -> pumpkin_util::math::boundingbox::BoundingBox {
        pumpkin_util::math::boundingbox::BoundingBox::new_array(
            [0.0, 6.0 / 16.0, 0.0],
            [1.0, 12.0 / 16.0, 1.0],
        )
    }
}

/// Entity.canUsePortal(false), including LivingEntity's health predicate.
pub(super) fn portal_eligible(entity: &dyn crate::entity::EntityBase) -> bool {
    entity.get_entity().is_alive()
        && !entity.get_entity().has_vehicle()
        && entity
            .get_living_entity()
            .is_none_or(|living| living.health.load() > 0.0)
}
