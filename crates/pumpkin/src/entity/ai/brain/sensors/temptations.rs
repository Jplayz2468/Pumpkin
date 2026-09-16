use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::memory::MemoryValue;
use crate::entity::ai::brain::registry::MemoryModuleType;
use crate::entity::ai::brain::sensor::{Sensor, SensorContext};
use pumpkin_data::tag::{Taggable, Tag};

/// `TemptingSensor.TEMPTATION_RANGE`.
const TEMPTATION_RANGE: f64 = 10.0;

/// Port of `TemptingSensor`.
///
/// Publishes the nearest player holding one of this mob's tempt items, which is what
/// `FollowTemptation` walks toward.
///
/// Known gap: vanilla tests both hands through `TargetingConditions` with line of sight
/// and spectator checks. This tests the held item of the closest player only.
pub struct TemptingSensor {
    tempt_items: &'static Tag,
}

impl TemptingSensor {
    #[must_use]
    pub const fn new(tempt_items: &'static Tag) -> Self {
        Self { tempt_items }
    }
}

impl Sensor<MobActor> for TemptingSensor {
    fn do_tick(&mut self, ctx: &mut SensorContext<'_, MobActor>) {
        let tempting = ctx
            .actor
            .world
            .get_closest_player(ctx.actor.position, TEMPTATION_RANGE)
            .filter(|player| player.inventory.held_item().item.has_tag(self.tempt_items));

        match tempting {
            Some(player) => ctx.memories.set(
                MemoryModuleType::TemptingPlayer,
                MemoryValue::EntityId(player.living_entity.entity.entity_id),
            ),
            None => ctx.memories.erase(MemoryModuleType::TemptingPlayer),
        }
    }

    fn requires(&self) -> &[MemoryModuleType] {
        &[MemoryModuleType::TemptingPlayer]
    }

    fn debug_name(&self) -> &'static str {
        "temptations"
    }
}
