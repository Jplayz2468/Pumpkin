use crate::entity::ai::brain::MobActor;
use crate::entity::ai::brain::behavior::{Behavior, BehaviorContext};
use crate::entity::ai::brain::behaviors::start_attacking::attack_target;
use crate::entity::ai::brain::memory::{MemoryStatus, MemoryValue};
use crate::entity::ai::brain::registry::MemoryModuleType;
use pumpkin_data::sound::Sound;
use pumpkin_util::math::vector3::Vector3;

/// Port of `ChargeAttack`.
///
/// Dashes at the `attack_target` in a straight line, hitting whatever it runs into, then
/// sets a cooldown. Used by the nautilus for its dash.
///
/// Known gaps: vanilla checks line of sight before charging, and computes knockback from
/// the charge speed scaled by movement speed and any speed or slowness effect. This uses
/// the shared melee attack, so the hit lands but the knockback is the ordinary one.
pub struct ChargeAttack {
    time_between_attacks: i32,
    speed: f32,
    max_charge_distance: f64,
    max_target_detection_distance: f64,
    charge_sound: Sound,
    charge_velocity: Vector3<f64>,
    start_position: Vector3<f64>,
    conditions: [(MemoryModuleType, MemoryStatus); 2],
}

impl ChargeAttack {
    #[must_use]
    pub fn new(
        time_between_attacks: i32,
        speed: f32,
        max_charge_distance: f64,
        max_target_detection_distance: f64,
        charge_sound: Sound,
    ) -> Self {
        Self {
            time_between_attacks,
            speed,
            max_charge_distance,
            max_target_detection_distance,
            charge_sound,
            charge_velocity: Vector3::new(0.0, 0.0, 0.0),
            start_position: Vector3::new(0.0, 0.0, 0.0),
            conditions: [
                (
                    MemoryModuleType::ChargeCooldownTicks,
                    MemoryStatus::ValueAbsent,
                ),
                (MemoryModuleType::AttackTarget, MemoryStatus::ValuePresent),
            ],
        }
    }

    fn target_position(ctx: &BehaviorContext<'_, MobActor>) -> Option<Vector3<f64>> {
        let id = attack_target(ctx.memories)?;
        ctx.actor
            .world
            .get_entity_by_id(id)
            .map(|target| target.get_entity().pos.load())
    }
}

impl Behavior<MobActor> for ChargeAttack {
    fn entry_conditions(&self) -> &[(MemoryModuleType, MemoryStatus)] {
        &self.conditions
    }

    fn check_extra_start_conditions(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        attack_target(ctx.memories).is_some()
    }

    fn can_still_use(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) -> bool {
        let Some(target) = Self::target_position(ctx) else {
            return false;
        };
        let travelled = ctx.actor.position.squared_distance_to(
            self.start_position.x,
            self.start_position.y,
            self.start_position.z,
        );
        if travelled >= self.max_charge_distance * self.max_charge_distance {
            return false;
        }
        let to_target = ctx
            .actor
            .position
            .squared_distance_to(target.x, target.y, target.z);
        if to_target >= self.max_target_detection_distance * self.max_target_detection_distance {
            return false;
        }
        !ctx.memories.has(MemoryModuleType::ChargeCooldownTicks)
    }

    fn start(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        self.start_position = ctx.actor.position;
        let Some(target) = Self::target_position(ctx) else {
            return;
        };
        let direction = (target - ctx.actor.position).normalize();
        self.charge_velocity = direction * f64::from(self.speed);

        if let Some(entity) = ctx.actor.entity() {
            let entity = entity.get_entity();
            entity.world.load().play_sound(
                self.charge_sound,
                pumpkin_data::sound::SoundCategory::Neutral,
                &entity.pos.load(),
            );
        }
    }

    fn tick(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        let Some(target_pos) = Self::target_position(ctx) else {
            return;
        };
        let Some(entity) = ctx.actor.entity() else {
            return;
        };
        entity.get_entity().look_at(target_pos);
        entity.get_entity().set_velocity(self.charge_velocity);

        // Vanilla scans its own bounding box for something to hit; arriving at the target
        // is the same moment for a straight-line charge.
        let Some(target_id) = attack_target(ctx.memories) else {
            return;
        };
        let Some(target) = ctx.actor.world.get_entity_by_id(target_id) else {
            return;
        };
        let reached = ctx
            .actor
            .position
            .squared_distance_to(target_pos.x, target_pos.y, target_pos.z)
            <= 1.0;
        if reached
            && let Some(mob) = entity.get_mob()
        {
            mob.get_mob_entity().try_attack(&*entity, &*target);
            ctx.memories.set(
                MemoryModuleType::ChargeCooldownTicks,
                MemoryValue::Int(self.time_between_attacks),
            );
            ctx.memories.erase(MemoryModuleType::AttackTarget);
        }
    }

    fn stop(&mut self, ctx: &mut BehaviorContext<'_, MobActor>) {
        ctx.memories.set(
            MemoryModuleType::ChargeCooldownTicks,
            MemoryValue::Int(self.time_between_attacks),
        );
        ctx.memories.erase(MemoryModuleType::AttackTarget);
    }

    fn debug_name(&self) -> &'static str {
        "charge_attack"
    }
}
