use super::{Controls, Goal, to_goal_ticks};

use crate::entity::ai::goal::track_target::TrackTargetGoal;
use crate::entity::ai::target_predicate::TargetPredicate;
use crate::entity::living::LivingEntity;
use crate::entity::mob::Mob;
use crate::entity::{EntityBase, mob::MobEntity};
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use rand::RngExt;
use std::sync::Arc;

const DEFAULT_RECIPROCAL_CHANCE: i32 = 10;

/// A target goal that, unlike [`super::active_target::ActiveTargetGoal`], is not restricted to a
/// single `EntityType`. It looks at every nearby living entity (players included) and attacks the
/// closest one that is not one of `excluded_types` and passes the attackability predicate.
///
/// This backs mobs whose vanilla Brain behaviour picks a target from
/// `NEAREST_VISIBLE_LIVING_ENTITIES` with a simple type-exclusion filter instead of hunting one
/// specific `EntityType` (e.g. `Zoglin.findNearestValidAttackTarget`, which excludes only
/// `zoglin` and `creeper`).
pub struct ActiveTargetAnyGoal {
    track_target_goal: TrackTargetGoal,
    target: Option<Arc<dyn EntityBase>>,
    reciprocal_chance: i32,
    excluded_types: &'static [&'static EntityType],
    target_predicate: TargetPredicate,
}

impl ActiveTargetAnyGoal {
    pub fn new<F>(
        mob: &MobEntity,
        excluded_types: &'static [&'static EntityType],
        reciprocal_chance: i32,
        check_visibility: bool,
        check_can_navigate: bool,
        predicate: Option<F>,
    ) -> Self
    where
        F: Fn(&LivingEntity, &crate::world::World) -> bool + Send + Sync + 'static,
    {
        let track_target_goal = TrackTargetGoal::new(check_visibility, check_can_navigate);
        let mut target_predicate = TargetPredicate::create_attackable();
        target_predicate.base_max_distance = mob
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);

        if let Some(predicate) = predicate {
            target_predicate.set_predicate(predicate);
        }

        Self {
            track_target_goal,
            target: None,
            reciprocal_chance: to_goal_ticks(reciprocal_chance),
            excluded_types,
            target_predicate,
        }
    }

    fn find_closest_target(&mut self, mob: &MobEntity) {
        let follow_range = mob
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        self.target_predicate.base_max_distance = follow_range;

        let world = mob.living_entity.entity.world.load();

        // Vanilla searches using getEyeY(), so we offset the position by the eye height
        let mut search_pos = mob.living_entity.entity.pos.load();
        search_pos.y += mob.living_entity.entity.entity_dimension.load().eye_height as f64;

        let mut best: Option<(Arc<dyn EntityBase>, f64)> = None;

        let mut consider = |candidate: Arc<dyn EntityBase>| {
            let candidate_type = candidate.get_entity().entity_type;
            if self
                .excluded_types
                .iter()
                .any(|excluded| *excluded == candidate_type)
            {
                return;
            }
            let Some(living) = candidate.get_living_entity() else {
                return;
            };
            if !self
                .target_predicate
                .test(&world, Some(&mob.living_entity), living)
            {
                return;
            }
            let dist_sq = candidate
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&search_pos);
            if best.as_ref().is_none_or(|(_, best_dist)| dist_sq < *best_dist) {
                best = Some((candidate, dist_sq));
            }
        };

        for player in world.get_nearby_players(search_pos, follow_range) {
            consider(player as Arc<dyn EntityBase>);
        }
        for (_, entity) in world.get_nearby_entities(search_pos, follow_range) {
            consider(entity);
        }

        self.target = best.map(|(entity, _)| entity);
    }
}

impl Goal for ActiveTargetAnyGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.reciprocal_chance > 0
            && mob.get_random().random_range(0..self.reciprocal_chance) != 0
        {
            return false;
        }
        self.find_closest_target(mob.get_mob_entity());
        self.target.is_some()
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        self.track_target_goal.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.set_mob_target(self.target.clone());
        self.track_target_goal.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.track_target_goal.stop(mob);
    }

    fn controls(&self) -> Controls {
        self.track_target_goal.controls()
    }
}
