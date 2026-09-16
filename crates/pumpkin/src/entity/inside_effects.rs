//! Vanilla InsideBlockEffectApplier.StepBasedCollector: deduplicate primary effects
//! within a step, preserve callbacks, and execute steps in their encounter order.
use super::EntityBase;
use pumpkin_data::{
    damage::DamageType,
    entity::EntityType,
    sound::{Sound, SoundCategory},
};
use std::{cell::RefCell, sync::atomic::Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Effect {
    Freeze,
    ClearFreeze,
    FireIgnite,
    LavaIgnite,
    Extinguish,
}
const ORDER: [Effect; 5] = [
    Effect::Freeze,
    Effect::ClearFreeze,
    Effect::FireIgnite,
    Effect::LavaIgnite,
    Effect::Extinguish,
];

#[derive(Debug, PartialEq)]
enum Queued<A> {
    Effect(Effect),
    Callback(A),
}
struct Collector<A> {
    step: i32,
    effects: [bool; 5],
    before: [Vec<A>; 5],
    after: [Vec<A>; 5],
    final_effects: Vec<Queued<A>>,
}
impl<A> Default for Collector<A> {
    fn default() -> Self {
        Self {
            step: -1,
            effects: [false; 5],
            before: std::array::from_fn(|_| Vec::new()),
            after: std::array::from_fn(|_| Vec::new()),
            final_effects: Vec::new(),
        }
    }
}
impl<A> Collector<A> {
    fn advance(&mut self, step: i32) {
        if self.step != step {
            self.step = step;
            self.flush();
        }
    }
    fn flush(&mut self) {
        for effect in ORDER {
            let index = effect as usize;
            self.final_effects
                .extend(self.before[index].drain(..).map(Queued::Callback));
            if std::mem::take(&mut self.effects[index]) {
                self.final_effects.push(Queued::Effect(effect));
            }
            self.final_effects
                .extend(self.after[index].drain(..).map(Queued::Callback));
        }
    }
    fn finish(&mut self) -> Vec<Queued<A>> {
        self.flush();
        self.step = -1;
        std::mem::take(&mut self.final_effects)
    }
}

type Callback = Box<dyn FnOnce(&dyn EntityBase)>;
#[derive(Default)]
pub struct InsideEffects {
    collector: RefCell<Collector<Callback>>,
}
impl InsideEffects {
    pub fn advance_step(&self, step: i32) {
        self.collector.borrow_mut().advance(step);
    }
    pub fn apply(&self, effect: Effect) {
        self.collector.borrow_mut().effects[effect as usize] = true;
    }
    pub fn before(&self, effect: Effect, callback: impl FnOnce(&dyn EntityBase) + 'static) {
        self.collector.borrow_mut().before[effect as usize].push(Box::new(callback));
    }
    pub fn after(&self, effect: Effect, callback: impl FnOnce(&dyn EntityBase) + 'static) {
        self.collector.borrow_mut().after[effect as usize].push(Box::new(callback));
    }
    pub fn apply_and_clear(&self, entity: &dyn EntityBase) {
        let actions = self.collector.borrow_mut().finish();
        for action in actions {
            if entity.get_entity().is_removed()
                || entity
                    .get_living_entity()
                    .is_some_and(|living| living.health.load() <= 0.0)
            {
                break;
            }
            match action {
                Queued::Effect(effect) => apply_primary(entity, effect),
                Queued::Callback(callback) => callback(entity),
            }
        }
    }
    pub fn lava(&self) {
        self.apply(Effect::ClearFreeze);
        self.apply(Effect::LavaIgnite);
        self.after(Effect::LavaIgnite, lava_hurt);
    }
}

fn apply_primary(caller: &dyn EntityBase, effect: Effect) {
    let entity = caller.get_entity();
    let fire_immune = entity.entity_type.fire_immune || entity.fire_immune.load(Ordering::Relaxed);
    match effect {
        Effect::Freeze => {
            entity.is_in_powder_snow.store(true, Ordering::Relaxed);
            if entity.can_freeze(caller) {
                entity.set_frozen_ticks(
                    (entity.get_frozen_ticks() + 1).min(super::Entity::MAX_FROZEN_TICKS),
                );
            }
        }
        Effect::ClearFreeze => entity.set_frozen_ticks(0),
        Effect::FireIgnite => {
            if !fire_immune {
                let ticks = entity.fire_ticks.load(Ordering::Relaxed);
                if ticks < 0 {
                    entity.fire_ticks.store(ticks + 1, Ordering::Relaxed);
                } else if caller.get_player().is_some() {
                    entity.fire_ticks.store(
                        ticks + 1 + entity.world.load().rand_bounded_i32(2),
                        Ordering::Relaxed,
                    );
                }
                if entity.fire_ticks.load(Ordering::Relaxed) >= 0 {
                    caller.set_on_fire_for(8.0);
                }
            }
        }
        Effect::LavaIgnite => {
            if !fire_immune {
                caller.set_on_fire_for(15.0);
            }
        }
        Effect::Extinguish => entity.extinguish(),
    }
}

fn lava_hurt(caller: &dyn EntityBase) {
    let entity = caller.get_entity();
    if !entity.entity_type.fire_immune
        && !entity.fire_immune.load(Ordering::Relaxed)
        && caller.damage(caller, 4.0, DamageType::LAVA)
        && entity.entity_type != &EntityType::ITEM
        && !entity.silent.load(Ordering::Relaxed)
    {
        let world = entity.world.load();
        world.play_sound_fine(
            Sound::EntityGenericBurn,
            SoundCategory::Neutral,
            &entity.pos.load(),
            0.4,
            2.0 + world.rand_f32() * 0.4,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn collector_matches_unmodified_java_26_2() {
        #[derive(serde::Deserialize)]
        struct Case {
            operations: Vec<[usize; 4]>,
            trace: Vec<String>,
        }
        fn drain(collector: &mut Collector<String>, trace: &mut Vec<String>) {
            trace.extend(collector.finish().into_iter().map(|entry| match entry {
                Queued::Effect(effect) => format!("e{}", effect as usize),
                Queued::Callback(token) => token,
            }));
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("inside_effects_cases.json")).unwrap();
        for (index, case) in cases.into_iter().enumerate() {
            let mut collector = Collector::default();
            let mut trace = Vec::new();
            for [operation, step, effect, token] in case.operations {
                match operation {
                    0 => collector.advance(step as i32),
                    1 => collector.effects[effect] = true,
                    2 => collector.before[effect].push(format!("c{token}")),
                    3 => collector.after[effect].push(format!("c{token}")),
                    4 => drain(&mut collector, &mut trace),
                    _ => panic!("unknown oracle operation"),
                }
            }
            drain(&mut collector, &mut trace);
            assert_eq!(trace, case.trace, "Java collector case {index}");
        }
    }

    #[test]
    fn one_primary_per_step_but_all_callbacks_preserve_source_order() {
        let mut collector = Collector::default();
        collector.advance(0);
        collector.effects[Effect::Extinguish as usize] = true;
        collector.effects[Effect::Freeze as usize] = true;
        collector.effects[Effect::Freeze as usize] = true;
        collector.after[Effect::Freeze as usize].extend(["a", "b"]);
        collector.before[Effect::Extinguish as usize].push("c");
        collector.advance(0);
        collector.advance(1);
        collector.effects[Effect::Freeze as usize] = true;
        assert_eq!(
            collector.finish(),
            vec![
                Queued::Effect(Effect::Freeze),
                Queued::Callback("a"),
                Queued::Callback("b"),
                Queued::Callback("c"),
                Queued::Effect(Effect::Extinguish),
                Queued::Effect(Effect::Freeze)
            ]
        );
        assert!(collector.finish().is_empty());
    }
}
