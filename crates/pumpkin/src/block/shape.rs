//! Shared collision-shape operations used by vanilla block behavior.
use crate::world::World;
use pumpkin_data::{BlockDirection, BlockState};
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};
use std::sync::Arc;

/// Shared with world-generation multiface attachment checks.
pub(crate) fn collision_face_covers(
    state: &BlockState,
    pos: BlockPos,
    side: BlockDirection,
    region: [f64; 4],
) -> bool {
    state.collision_face_covers(pos, side, region)
}

/// Vanilla Block.pushEntitiesUp: move entities by the newly occupied collision
/// volume only. The box subtraction implements Shapes.join(..., ONLY_SECOND).
pub(crate) fn push_entities_up(
    world: &Arc<World>,
    pos: BlockPos,
    old: &BlockState,
    new: &BlockState,
) {
    let mut added: Vec<BoundingBox> = new.get_block_collision_shapes_at(&pos).collect();
    for cut in old.get_block_collision_shapes_at(&pos) {
        added = added
            .into_iter()
            .flat_map(|bounds| {
                if !bounds.intersects(&cut) {
                    return vec![bounds];
                }
                let lo = Vector3::new(
                    bounds.min.x.max(cut.min.x),
                    bounds.min.y.max(cut.min.y),
                    bounds.min.z.max(cut.min.z),
                );
                let hi = Vector3::new(
                    bounds.max.x.min(cut.max.x),
                    bounds.max.y.min(cut.max.y),
                    bounds.max.z.min(cut.max.z),
                );
                let pieces = [
                    BoundingBox::new(bounds.min, Vector3::new(lo.x, bounds.max.y, bounds.max.z)),
                    BoundingBox::new(Vector3::new(hi.x, bounds.min.y, bounds.min.z), bounds.max),
                    BoundingBox::new(
                        Vector3::new(lo.x, bounds.min.y, bounds.min.z),
                        Vector3::new(hi.x, lo.y, bounds.max.z),
                    ),
                    BoundingBox::new(
                        Vector3::new(lo.x, hi.y, bounds.min.z),
                        Vector3::new(hi.x, bounds.max.y, bounds.max.z),
                    ),
                    BoundingBox::new(
                        Vector3::new(lo.x, lo.y, bounds.min.z),
                        Vector3::new(hi.x, hi.y, lo.z),
                    ),
                    BoundingBox::new(
                        Vector3::new(lo.x, lo.y, hi.z),
                        Vector3::new(hi.x, hi.y, bounds.max.z),
                    ),
                ];
                pieces
                    .into_iter()
                    .filter(|b| b.min.x < b.max.x && b.min.y < b.max.y && b.min.z < b.max.z)
                    .collect()
            })
            .collect();
    }
    let offset = Vector3::new(f64::from(pos.0.x), f64::from(pos.0.y), f64::from(pos.0.z));
    for shape in &mut added {
        *shape = shape.shift(offset);
    }
    let Some(first) = added.first().copied() else {
        return;
    };
    let bounds = added.iter().skip(1).fold(first, |a, b| {
        BoundingBox::new(
            Vector3::new(
                a.min.x.min(b.min.x),
                a.min.y.min(b.min.y),
                a.min.z.min(b.min.z),
            ),
            Vector3::new(
                a.max.x.max(b.max.x),
                a.max.y.max(b.max.y),
                a.max.z.max(b.max.z),
            ),
        )
    });
    for entity in world.get_entities_at_box(&bounds) {
        let moved = entity
            .get_entity()
            .bounding_box
            .load()
            .shift(Vector3::new(0.0, 1.0, 0.0));
        let mut downward: f64 = -1.0;
        for shape in &added {
            if moved.max.x > shape.min.x + 1.0e-7
                && moved.min.x < shape.max.x - 1.0e-7
                && moved.max.z > shape.min.z + 1.0e-7
                && moved.min.z < shape.max.z - 1.0e-7
                && moved.min.y >= shape.max.y - 1.0e-7
            {
                downward = downward.max(shape.max.y - moved.min.y);
            }
        }
        crate::entity::teleport::move_relative(entity.as_ref(), Vector3::new(0.0, 1.0 + downward, 0.0));
    }
}
