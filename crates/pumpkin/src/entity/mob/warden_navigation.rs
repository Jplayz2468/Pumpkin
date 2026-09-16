//! World adapters for the separately Java-verified Warden movement behaviors.
use super::{LookTarget, WardenEntity};
use crate::entity::ai::pathfinder::{
    Navigator, path::Path, pathfinding_context::PathfindingContext,
};
use crate::entity::{
    EntityBase,
    mob::{warden_brain::Phase, warden_investigate, warden_melee, warden_move, warden_random_pos},
};
use pumpkin_util::{
    math::{position::BlockPos, vector3::Vector3},
    random::RandomImpl,
};
use std::{
    collections::HashMap,
    sync::{Arc, atomic::Ordering},
};

#[derive(Clone)]
pub(super) enum WalkPosition {
    Block([i32; 3]),
    Entity(Arc<dyn EntityBase>),
}
#[derive(Clone)]
pub(super) struct WalkMemory {
    pub target: WalkPosition,
    pub speed: f32,
    pub distance: i32,
}
impl WalkMemory {
    fn snapshot(&self) -> warden_move::WalkTarget {
        let (position, spectator) = match &self.target {
            WalkPosition::Block(p) => (*p, false),
            WalkPosition::Entity(e) => {
                let p = e.get_entity().block_pos.load().0;
                (
                    [p.x, p.y, p.z],
                    e.get_player().is_some_and(|p| p.is_spectator()),
                )
            }
        };
        warden_move::WalkTarget {
            position,
            spectator,
            speed: self.speed,
            distance: self.distance,
        }
    }
}
#[derive(Default)]
pub(super) struct MovementState {
    sink: warden_move::MoveSink,
    memories: warden_move::Memories,
    walk: Option<WalkMemory>,
    paths: HashMap<u64, Path>,
}
struct Adapter<'a> {
    warden: &'a WardenEntity,
    nav: &'a mut Navigator,
    paths: &'a mut HashMap<u64, Path>,
}
impl Adapter<'_> {
    fn create(&mut self, target: [i32; 3]) -> Option<(u64, bool)> {
        let living = &self.warden.mob_entity.living_entity;
        let size = living.entity.entity_dimension.load();
        self.nav
            .set_mob_dimensions(size.width as f32, size.height as f32);
        let target = BlockPos::new(target[0], target[1], target[2]);
        let path = self.nav.create_path(
            living,
            Vector3::new(
                f64::from(target.0.x),
                f64::from(target.0.y),
                f64::from(target.0.z),
            ),
            0,
        )?;
        let result = (path.identity(), path.can_reach());
        self.paths.insert(result.0, path);
        Some(result)
    }
}
impl warden_move::Navigation for Adapter<'_> {
    fn create_path(&mut self, target: [i32; 3]) -> Option<(u64, bool)> {
        self.create(target)
    }
    fn fallback_path(&mut self, target: [i32; 3]) -> Option<u64> {
        let position = self.warden.random_destination(
            self.nav,
            Some([
                f64::from(target[0]) + 0.5,
                f64::from(target[1]),
                f64::from(target[2]) + 0.5,
            ]),
            false,
        )?;
        self.create(position.map(|v| v.floor() as i32)).map(|p| p.0)
    }
    fn move_to(&mut self, path: Option<u64>, speed: f32) {
        let path = path.and_then(|id| self.paths.get(&id)).cloned();
        self.nav.move_to_path(
            path,
            f64::from(speed),
            &self.warden.mob_entity.living_entity,
        );
    }
    fn path(&self) -> Option<u64> {
        self.nav.get_path().map(Path::identity)
    }
    fn done(&self) -> bool {
        self.nav.is_done()
    }
    fn stuck(&self) -> bool {
        self.nav.is_stuck()
    }
    fn stop(&mut self) {
        self.nav.stop();
    }
    fn next_world_int(&mut self, bound: i32) -> i32 {
        self.warden.next_world_int(bound)
    }
}
impl WardenEntity {
    pub(super) fn has_walk_target(&self) -> bool {
        self.movement
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .walk
            .is_some()
    }
    pub(super) fn clear_walk_target(&self) {
        self.movement
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .walk = None;
    }
    fn set_walk_target(&self, walk: WalkMemory) {
        self.movement
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .walk = Some(walk);
    }
    fn random_destination(
        &self,
        nav: &Navigator,
        target: Option<[f64; 3]>,
        land: bool,
    ) -> Option<[f64; 3]> {
        let entity = self.get_entity();
        let world = entity.world.load_full();
        let origin = entity.pos.load();
        let facts = warden_random_pos::Search {
            origin: [origin.x, origin.y, origin.z],
            horizontal: 10,
            vertical: 7,
            target,
            spread: f64::from(std::f32::consts::FRAC_PI_2),
            home: None,
            min_y: world.get_bottom_y(),
            max_y: world.get_top_y() + 1,
        };
        let mut context = PathfindingContext::new(entity.block_pos.load().0, world.clone());
        let mut random = self
            .random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        warden_random_pos::choose_transformed(&mut *random, facts, |p| {
            let pos = BlockPos::new(p[0], p[1], p[2]);
            if !world.get_block_state(&pos.down()).is_solid_render() {
                return None;
            }
            let p = if land {
                warden_random_pos::land_candidate(
                    p,
                    world.get_top_y(),
                    |p| {
                        world
                            .get_block_state(&BlockPos::new(p[0], p[1], p[2]))
                            .is_solid()
                    },
                    |p| {
                        use pumpkin_data::tag::Taggable;
                        pumpkin_data::fluid::Fluid::from_state_id(
                            world.get_block_state_id(&BlockPos::new(p[0], p[1], p[2])),
                        )
                        .is_some_and(|f| f.has_tag(&pumpkin_data::tag::Fluid::MINECRAFT_WATER))
                    },
                    |p| {
                        nav.get_pathfinding_malus(
                            context.get_land_node_type(Vector3::new(p[0], p[1], p[2])),
                        )
                    },
                )?
            } else {
                if nav.get_pathfinding_malus(context.get_land_node_type(pos.0)) != 0.0 {
                    return None;
                }
                p
            };
            // Warden overrides Monster's light-based target preference with zero.
            Some((p, 0.0))
        })
    }
    pub(super) fn tick_move(&self, phase: Phase) {
        let entity = self.get_entity();
        let p = entity.block_pos.load().0;
        let time = entity.world.load().get_world_age();
        let mut state = self
            .movement
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut nav = self
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(path) = nav.get_path() {
            state.paths.insert(path.identity(), path.clone());
        }
        state.memories.walk = state.walk.as_ref().map(WalkMemory::snapshot);
        let MovementState {
            sink,
            memories,
            paths,
            ..
        } = &mut *state;
        let mut adapter = Adapter {
            warden: self,
            nav: &mut nav,
            paths,
        };
        match phase {
            Phase::Start => sink.start(time, [p.x, p.y, p.z], memories, &mut adapter),
            Phase::Run => sink.run(time, [p.x, p.y, p.z], memories, &mut adapter),
        }
        if state.memories.walk.is_none() {
            state.walk = None;
        }
        let keep = [
            state.sink.path,
            state.memories.path,
            nav.get_path().map(Path::identity),
        ];
        state.paths.retain(|id, _| keep.contains(&Some(*id)));
    }
    pub(super) fn investigate(&self) {
        let disturbance = self
            .sniff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .disturbance
            .map(|v| v.0);
        let attack = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attack_target
            .is_some();
        let p = self.get_entity().block_pos.load().0;
        if let Some(walk) = warden_investigate::investigate(
            [p.x, p.y, p.z],
            disturbance,
            attack,
            self.has_walk_target(),
            |bound| self.next_world_int(bound),
        ) {
            self.set_walk_target(WalkMemory {
                target: WalkPosition::Block(walk.position),
                speed: walk.speed,
                distance: walk.distance,
            });
        }
    }
    pub(super) fn pursue(&self) {
        let id = self
            .roar
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .attack_target;
        let Some(target) = id.and_then(|id| self.get_entity().world.load().get_entity_by_id(id))
        else {
            return;
        };
        let b = self.get_entity().bounding_box.load();
        let bounds = [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z];
        let vehicle = self.get_entity().get_vehicle().map(|v| {
            let b = v.get_entity().bounding_box.load();
            [b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z]
        });
        if self.look_entity_visible(target.as_ref())
            && warden_melee::in_range(bounds, vehicle, self.sensor_target(target.as_ref()).bounds)
        {
            self.clear_walk_target();
        } else {
            self.set_look_target(LookTarget::Entity(target.clone()), i64::MAX);
            self.set_walk_target(WalkMemory {
                target: WalkPosition::Entity(target),
                speed: 1.2,
                distance: 0,
            });
        }
    }
    pub(super) fn tick_idle(&self, phase: Phase) {
        let time = self.get_entity().world.load().get_world_age();
        let mut idle = self
            .idle
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if phase == Phase::Run {
            idle.run(time);
            return;
        }
        let sniffing = self
            .sniff
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .sniffing
            .is_some();
        let mut shuffle = self
            .idle_random
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if idle.start(
            time,
            sniffing,
            self.has_walk_target(),
            self.get_entity().is_in_water(),
            || shuffle.next_f32(),
            |bound| self.next_world_int(bound),
        ) {
            let nav = self
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let position = self.random_destination(&nav, None, true);
            drop(nav);
            if let Some(position) = position {
                self.set_walk_target(WalkMemory {
                    target: WalkPosition::Block(position.map(|v| v.floor() as i32)),
                    speed: 0.5,
                    distance: 0,
                });
            } else {
                self.clear_walk_target();
            }
        }
    }
    pub(super) fn tick_swim(&self, phase: Phase) {
        let entity = self.get_entity();
        let time = entity.world.load().get_world_age();
        let eligible = crate::entity::mob::warden_swim::should_swim(
            entity.is_in_water(),
            entity.water_height.load(),
            self.mob_entity.living_entity.get_swim_height(),
            entity.is_in_lava(),
        );
        let mut swim = self
            .swim
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match phase {
            Phase::Start => swim.start(time, eligible, |bound| self.next_world_int(bound)),
            Phase::Run => {
                let mut random = self
                    .random
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if swim.run(time, eligible, || random.next_f32()) {
                    self.mob_entity
                        .living_entity
                        .jumping
                        .store(true, Ordering::Relaxed);
                }
            }
        }
    }
}
