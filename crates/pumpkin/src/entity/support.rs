//! CollisionGetter.findSupportingBlock: distance first, then greatest (Y, Z, X).
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

pub(super) fn footprint(bounds: BoundingBox) -> BoundingBox {
    BoundingBox::new(
        Vector3::new(bounds.min.x, bounds.min.y - 1.0e-6, bounds.min.z),
        Vector3::new(bounds.max.x, bounds.min.y, bounds.max.z),
    )
}

pub(crate) fn nearest(
    position: Vector3<f64>,
    candidates: impl IntoIterator<Item = BlockPos>,
) -> Option<BlockPos> {
    let mut best: Option<BlockPos> = None;
    let mut distance = f64::MAX;
    for pos in candidates {
        let dx = f64::from(pos.0.x) + 0.5 - position.x;
        let dy = f64::from(pos.0.y) + 0.5 - position.y;
        let dz = f64::from(pos.0.z) + 0.5 - position.z;
        let candidate_distance = dx * dx + dy * dy + dz * dz;
        let key = |p: BlockPos| (p.0.y, p.0.z, p.0.x);
        if candidate_distance < distance
            || (candidate_distance == distance && best.is_none_or(|old| key(old) < key(pos)))
        {
            best = Some(pos);
            distance = candidate_distance;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn support_ties_use_java_order_independent_of_iteration() {
        let candidates = [
            BlockPos::new(-1, -1, -1),
            BlockPos::new(0, -1, -1),
            BlockPos::new(-1, -1, 0),
            BlockPos::new(0, -1, 0),
        ];
        assert_eq!(nearest(Vector3::default(), candidates), Some(candidates[3]));
        assert_eq!(
            nearest(Vector3::default(), candidates.into_iter().rev()),
            Some(candidates[3])
        );
        assert_eq!(
            nearest(Vector3::new(-0.1, 0.0, -0.1), candidates),
            Some(candidates[0])
        );
        assert_eq!(
            nearest(
                Vector3::default(),
                [BlockPos::new(0, -1, 0), BlockPos::new(0, 0, 0)]
            ),
            Some(BlockPos::new(0, 0, 0))
        );
        assert_eq!(nearest(Vector3::default(), []), None);
    }
    #[test]
    fn footprint_intersects_fence_top_but_not_blocks_below_feet() {
        let feet = footprint(BoundingBox::new_array([0.2, 1.5, 0.2], [0.8, 3.3, 0.8]));
        assert!(feet.intersects(&BoundingBox::new_array(
            [0.375, 0.0, 0.375],
            [0.625, 1.5, 0.625]
        )));
        assert!(!feet.intersects(&BoundingBox::new_array([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])));
    }
}
