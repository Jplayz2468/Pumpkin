use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::entity::EntityBase;
use crate::entity::decoration::leash_knot::LeashKnotEntity;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::Block;
use pumpkin_data::BlockDirection;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

pub struct LeadItem;

impl ItemMetadata for LeadItem {
    fn ids() -> Box<[u16]> {
        [Item::LEAD.id].into()
    }
}

/// Vanilla's leash-snap distance (`Leashable.leashSnapDistance()`, Leashable.java:181-183 /
/// `Leashable.canHaveALeashAttachedTo`, Leashable.java:55-65): a candidate found by the wide
/// area scan is only actually attached to the knot if it is within this distance of it.
const LEASH_SNAP_DISTANCE_SQ: f64 = 12.0 * 12.0;

impl ItemBehaviour for LeadItem {
    fn use_on_block(
        &self,
        _item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        if !block.has_tag(&pumpkin_data::tag::Block::MINECRAFT_FENCES) {
            return BlockActionResult::Pass;
        }

        let world = player.world();
        let center = Vector3::new(
            f64::from(location.0.x) + 0.5,
            f64::from(location.0.y) + 0.5,
            f64::from(location.0.z) + 0.5,
        );
        // LeashFenceKnotEntity's own position (LeashFenceKnotEntity.java:31,48) is offset by
        // OFFSET_Y (0.375), not centered on the block like `center` above. Used both as the
        // reference point for the snap-distance check and for the placement sound.
        let knot_center = Vector3::new(
            f64::from(location.0.x) + 0.5,
            f64::from(location.0.y) + 0.375,
            f64::from(location.0.z) + 0.5,
        );

        let search_dim = EntityDimensions {
            width: 32.0,
            height: 32.0,
            eye_height: 16.0,
        };

        let search_box = BoundingBox::new_from_pos(center.x, center.y, center.z, &search_dim);

        let player_id = player.entity_id();
        let entities = world.get_entities_at_box(&search_box);

        // LeadItem.java:38: entitiesToLeash = Leashable.leashableInArea(level, center, l ->
        // l.getLeashHolder() == player) — only entities currently leashed to the interacting
        // player are candidates; the snap-distance gate below is separate (canHaveALeashAttachedTo).
        let candidates: Vec<_> = entities
            .into_iter()
            .filter(|entity_base| {
                let ent = entity_base.get_entity();
                ent.leashed_to
                    .try_lock()
                    .ok()
                    .and_then(|guard| {
                        guard
                            .as_ref()
                            .map(|holder| holder.get_entity().entity_id == player_id)
                    })
                    .unwrap_or(false)
            })
            .collect();

        if candidates.is_empty() {
            // LeadItem.java:39-41
            return BlockActionResult::Pass;
        }

        let mut any_leashed = false;
        let mut knot: Option<Arc<LeashKnotEntity>> = None;

        for entity_base in candidates {
            let ent = entity_base.get_entity();

            // Leashable.canHaveALeashAttachedTo / leashDistanceTo (Leashable.java:55-65):
            // reject candidates farther than the snap distance from the knot, even though the
            // area scan above found them within the much wider 32-block box.
            let ent_bb = ent.bounding_box.load();
            let ent_center = ent_bb.min.add(&ent_bb.max).multiply(0.5, 0.5, 0.5);
            if ent_center.squared_distance_to_vec(&knot_center) > LEASH_SNAP_DISTANCE_SQ {
                continue;
            }

            if knot.is_none() {
                knot = Some(LeashKnotEntity::get_or_create(&world, location));
            }
            if let Some(k) = &knot {
                ent.leash_to(k.clone() as Arc<dyn EntityBase>);
                any_leashed = true;
            }
        }

        if any_leashed {
            // LeadItem.java:37-65 (bindPlayerMobs): binding already-leashed mobs to a fence
            // knot never shrinks the item stack — there is no itemStack.shrink call anywhere
            // in this method. The lead was already spent when it was first attached to the mob.
            world.play_sound(Sound::ItemLeadTied, SoundCategory::Neutral, &knot_center);
            world.emit_game_event(GameEvent::BlockAttach.name(), location.to_centered_f64());
            BlockActionResult::Success
        } else {
            BlockActionResult::Pass
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
