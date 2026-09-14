use super::{Controls, Goal, to_goal_ticks};
use crate::entity::{
    ai::pathfinder::{NavigatorGoal, pathfinding_context::PathfindingContext},
    mob::Mob,
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use rand::RngExt;
use std::sync::atomic::Ordering::Relaxed;

/// Java RandomPos samples ten candidates, keeps the first highest score, and
/// navigates to the bottom center of that block. Invalid candidates still count.
fn choose_position(mut candidate: impl FnMut() -> Option<(BlockPos, f64)>) -> Option<Vector3<f64>> {
    let mut best = None;
    let mut best_score = f64::NEG_INFINITY;
    for _ in 0..10 {
        if let Some((pos, score)) = candidate()
            && score > best_score
        {
            best_score = score;
            best = Some(Vector3::new(
                f64::from(pos.0.x) + 0.5,
                f64::from(pos.0.y),
                f64::from(pos.0.z) + 0.5,
            ));
        }
    }
    best
}

fn random_direction(
    horizontal: i32,
    vertical: i32,
    mut next_int: impl FnMut(i32) -> i32,
) -> Vector3<i32> {
    Vector3::new(
        next_int(2 * horizontal + 1) - horizontal,
        next_int(2 * vertical + 1) - vertical,
        next_int(2 * horizontal + 1) - horizontal,
    )
}

fn move_up_out_of_solid(
    mut pos: BlockPos,
    max_y: i32,
    mut solid: impl FnMut(BlockPos) -> bool,
) -> BlockPos {
    if solid(pos) {
        pos.0.y += 1;
        while pos.0.y <= max_y && solid(pos) {
            pos.0.y += 1;
        }
    }
    pos
}

fn can_stroll(no_action_time: i32, chance: i32, mut next_int: impl FnMut(i32) -> i32) -> bool {
    no_action_time < 100 && next_int(chance) == 0
}

pub struct WanderAroundGoal {
    speed: f64,
    target: Option<Vector3<f64>>,
    chance: i32,
    avoid_water: bool,
}

impl WanderAroundGoal {
    #[must_use]
    pub const fn new(speed: f64) -> Self {
        Self {
            speed,
            target: None,
            chance: to_goal_ticks(120),
            avoid_water: false,
        }
    }

    /// WaterAvoidingRandomStrollGoal used by ordinary land monsters in Java.
    #[must_use]
    pub const fn water_avoiding(speed: f64) -> Self {
        Self {
            avoid_water: true,
            ..Self::new(speed)
        }
    }

    fn find_ground_target(mob: &dyn Mob, horizontal: i32, land: bool) -> Option<Vector3<f64>> {
        let me = mob.get_mob_entity();
        let entity = &me.living_entity.entity;
        let pos = entity.pos.load();
        let world = entity.world.load_full();
        let mut rng = mob.get_random();
        let home = me.position_target.load();
        let has_home = me.has_position_target();
        let radius =
            f64::from(me.position_target_range.load(Relaxed)) + f64::from(horizontal) + 1.0;
        let home_center = Vector3::new(
            f64::from(home.0.x) + 0.5,
            f64::from(home.0.y) + 0.5,
            f64::from(home.0.z) + 0.5,
        );
        let restricted = has_home && (home_center - pos).length_squared() < radius * radius;
        let mut context = PathfindingContext::new(entity.block_pos.load().0, world.clone());
        let navigator = me
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        choose_position(|| {
            let offset = random_direction(horizontal, 7, |bound| rng.random_range(0..bound));
            let mut dx = f64::from(offset.x);
            let mut dz = f64::from(offset.z);
            if has_home && horizontal > 1 {
                let x_bias = rng.random::<f64>() * f64::from(horizontal) / 2.0;
                dx += if pos.x > f64::from(home.0.x) {
                    -x_bias
                } else {
                    x_bias
                };
                let z_bias = rng.random::<f64>() * f64::from(horizontal) / 2.0;
                dz += if pos.z > f64::from(home.0.z) {
                    -z_bias
                } else {
                    z_bias
                };
            }
            let mut target = (pos + Vector3::new(dx, f64::from(offset.y), dz)).to_block_pos();
            if target.0.y < world.min_y
                || target.0.y >= world.min_y + world.dimension.height
                || (restricted && !me.is_in_position_target_range_pos(&target))
                || !world
                    .get_block_state(&BlockPos(target.0 - Vector3::new(0, 1, 0)))
                    .is_solid_render()
            {
                return None;
            }
            if land {
                target =
                    move_up_out_of_solid(target, world.min_y + world.dimension.height - 1, |p| {
                        world.get_block_state(&p).is_solid()
                    });
                if context.is_water(&target) {
                    return None;
                }
            }
            if navigator.get_pathfinding_malus(context.get_land_node_type(target.0)) != 0.0 {
                return None;
            }
            // Monster#getWalkTargetValue is the negative light path cost.
            // Other species retain their existing neutral preference here.
            let score =
                if *entity.entity_type.category == pumpkin_data::entity::MobCategory::MONSTER {
                    let light = f32::from(world.get_max_local_raw_brightness(&target)) / 15.0;
                    let darkness = light / (4.0 - 3.0 * light);
                    let ambient = world.dimension.ambient_light;
                    0.5 - (darkness + ambient * (1.0 - darkness))
                } else {
                    0.0
                };
            Some((target, f64::from(score)))
        })
    }

    fn find_wander_target(&self, mob: &dyn Mob) -> Option<Vector3<f64>> {
        let me = mob.get_mob_entity();
        let ground = me
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .can_navigate_ground();
        if ground {
            if self.avoid_water && me.living_entity.entity.is_in_water() {
                return Self::find_ground_target(mob, 15, true)
                    .or_else(|| Self::find_ground_target(mob, 10, false));
            }
            let land = self.avoid_water && mob.get_random().random::<f32>() >= 0.001;
            return Self::find_ground_target(mob, 10, land);
        }
        // Flying and aquatic navigation need their own Java destination predicates.
        let pos = me.living_entity.entity.pos.load();
        let mut rng = mob.get_random();
        Some(
            pos + Vector3::new(
                rng.random_range(-10.0..=10.0),
                rng.random_range(-7.0..=7.0),
                rng.random_range(-10.0..=10.0),
            ),
        )
    }
}

impl Goal for WanderAroundGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if !can_stroll(
            mob.get_mob_entity().no_action_time.load(Relaxed),
            self.chance,
            |bound| mob.get_random().random_range(0..bound),
        ) {
            return false;
        }
        self.target = self.find_wander_target(mob);
        self.target.is_some()
    }
    fn should_continue(&self, mob: &dyn Mob) -> bool {
        !mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_idle()
    }
    fn start(&mut self, mob: &dyn Mob) {
        if let Some(target) = self.target {
            let me = mob.get_mob_entity();
            me.navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(
                    me.living_entity.entity.pos.load(),
                    target,
                    self.speed,
                ));
        }
    }
    fn stop(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.target = None;
    }
    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("../../../../tests/fixtures/ai-java-26.2.json")).unwrap()
    }
    fn pos(value: &serde_json::Value) -> BlockPos {
        BlockPos::new(
            value[0].as_i64().unwrap() as i32,
            value[1].as_i64().unwrap() as i32,
            value[2].as_i64().unwrap() as i32,
        )
    }
    #[test]
    fn java_oracle_wander_inactivity() {
        for c in fixture()["gates"].as_array().unwrap() {
            let mut draws = 0;
            let actual = can_stroll(
                c["idle"].as_i64().unwrap() as i32,
                to_goal_ticks(c["interval"].as_i64().unwrap() as i32),
                |_| {
                    draws += 1;
                    c["ticket"].as_i64().unwrap() as i32
                },
            );
            assert_eq!(actual, c["expected"].as_bool().unwrap());
            assert_eq!(draws, c["draws"].as_i64().unwrap());
        }
    }
    #[test]
    fn java_oracle_wander_directions() {
        for c in fixture()["directions"].as_array().unwrap() {
            let mut index = 0;
            let actual = random_direction(
                c["h"].as_i64().unwrap() as i32,
                c["v"].as_i64().unwrap() as i32,
                |bound| {
                    let result = c["tape"][index].as_i64().unwrap() as i32;
                    assert!((0..bound).contains(&result));
                    index += 1;
                    result
                },
            );
            assert_eq!(actual, pos(&c["expected"]).0);
            assert_eq!(index, c["draws"].as_u64().unwrap() as usize);
        }
    }
    #[test]
    fn java_oracle_wander_selection() {
        for c in fixture()["choices"].as_array().unwrap() {
            let mut index = 0;
            let actual = choose_position(|| {
                let candidate = &c["candidates"][index];
                index += 1;
                (!candidate["pos"].is_null())
                    .then(|| (pos(&candidate["pos"]), candidate["score"].as_f64().unwrap()))
            });
            let expected = (!c["expected"].is_null()).then(|| {
                Vector3::new(
                    c["expected"][0].as_f64().unwrap(),
                    c["expected"][1].as_f64().unwrap(),
                    c["expected"][2].as_f64().unwrap(),
                )
            });
            assert_eq!(actual, expected);
            assert_eq!(index, c["draws"].as_u64().unwrap() as usize);
        }
    }
    #[test]
    fn java_oracle_wander_solid_columns() {
        for c in fixture()["columns"].as_array().unwrap() {
            let actual = move_up_out_of_solid(
                BlockPos::new(-7, c["start"].as_i64().unwrap() as i32, 3),
                c["ceiling"].as_i64().unwrap() as i32,
                |p| p.0.y <= c["top"].as_i64().unwrap() as i32,
            );
            assert_eq!(actual, pos(&c["expected"]));
        }
    }
}
