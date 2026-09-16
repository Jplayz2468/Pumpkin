use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::memory::MemoryValue;
use crate::entity::ai::brain::registry::MemoryModuleType;
use crate::entity::ai::brain::sensor::{Sensor, SensorContext};

/// How far vanilla's player sensor looks, from `PlayerSensor`'s follow-range use.
const SEARCH_RADIUS: f64 = 16.0;

/// Port of `NearestPlayersSensor`.
///
/// Writes the nearest player into `nearest_visible_player` and also publishes it as the
/// mob's `look_target`, which is what gives an idle brain mob something to look at.
///
/// Known gap: vanilla separates `nearest_players`, `nearest_visible_player` and
/// `nearest_visible_attackable_player` behind `TargetingConditions` (line of sight,
/// invisibility, spectator and team checks). This writes the closest player without those
/// checks, so a mob can notice a player it should not be able to see.
pub struct NearestPlayersSensor;

impl Sensor<MobActor> for NearestPlayersSensor {
    fn do_tick(&mut self, ctx: &mut SensorContext<'_, MobActor>) {
        let nearest = ctx
            .actor
            .world
            .get_closest_player(ctx.actor.position, SEARCH_RADIUS);

        match nearest {
            Some(player) => {
                let entity = &player.living_entity.entity;
                ctx.memories.set(
                    MemoryModuleType::NearestVisiblePlayer,
                    MemoryValue::EntityId(entity.entity_id),
                );
                ctx.memories
                    .set(MemoryModuleType::LookTarget, MemoryValue::Vec3(entity.pos.load()));
            }
            None => {
                ctx.memories.erase(MemoryModuleType::NearestVisiblePlayer);
            }
        }
    }

    fn requires(&self) -> &[MemoryModuleType] {
        &[
            MemoryModuleType::NearestVisiblePlayer,
            MemoryModuleType::LookTarget,
        ]
    }

    fn debug_name(&self) -> &'static str {
        "nearest_players"
    }
}
