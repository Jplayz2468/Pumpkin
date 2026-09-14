use crate::entity::ai::goal::{Controls, Goal, PrioritizedGoal};
use crate::entity::mob::Mob;
use std::any::TypeId;

/// `GoalSelector` manages a set of goals and decides which ones can run.
///
/// Important: `GoalSelector` is intentionally not `Send`/`Sync`.
/// Once the outer mutex is locked, no other thread can access it,
/// so we don't need extra thread-safe wrappers here.
/// don't touch this if you dont know how this works!
pub struct GoalSelector {
    /// Indieces into self.goals
    /// `usize::max` means no goal
    goals_by_control: [usize; 4],
    goals: Vec<PrioritizedGoal>,
    disabled_controls: Controls,
}

impl GoalSelector {
    pub fn add_goal<G: Goal + 'static>(&mut self, priority: u8, goal: Box<G>) {
        self.goals
            .push(PrioritizedGoal::new(TypeId::of::<G>(), priority, goal));
    }

    pub fn remove_goal<G: Goal + 'static>(&mut self, mob: &dyn Mob) {
        let mut stopped = self.remove_goal_by_type_id(TypeId::of::<G>());
        for goal in &mut stopped {
            goal.stop(mob);
        }
    }

    pub fn remove_goals<G: Goal + 'static>(&mut self) -> Vec<PrioritizedGoal> {
        self.remove_goal_by_type_id(TypeId::of::<G>())
    }

    pub fn remove_goal_by_type_id(&mut self, type_id: TypeId) -> Vec<PrioritizedGoal> {
        let mut stopped = Vec::new();
        let mut i = 0;
        while i < self.goals.len() {
            if self.goals[i].type_id == type_id {
                // Java keeps available goals in insertion order. Removing a
                // goal must not change which equal-priority goal wins next.
                let goal = self.goals.remove(i);
                for slot in &mut self.goals_by_control {
                    if *slot == usize::MAX {
                        continue;
                    }
                    if *slot == i {
                        *slot = usize::MAX;
                    } else if *slot > i {
                        *slot -= 1;
                    }
                }
                if goal.running {
                    stopped.push(goal);
                }
            } else {
                i += 1;
            }
        }
        stopped
    }

    pub fn clear(&mut self) -> Vec<PrioritizedGoal> {
        let mut running = Vec::new();
        for goal in self.goals.drain(..) {
            if goal.running {
                running.push(goal);
            }
        }
        self.goals_by_control = [usize::MAX; 4];
        running
    }

    fn uses_any(prioritized_goal: &PrioritizedGoal, controls: Controls) -> bool {
        let goal_controls = prioritized_goal.controls();
        for control in Controls::ITER {
            if controls.get(control) && goal_controls.get(control) {
                return true;
            }
        }

        false
    }

    fn can_replace_all(&self, goal: &PrioritizedGoal) -> bool {
        let controls = goal.controls();
        for control in Controls::ITER {
            if controls.get(control) {
                let goal_idx = self.goals_by_control[control.idx()];

                if goal_idx != usize::MAX && !self.goals[goal_idx].can_be_replaced_by(goal) {
                    return false;
                }
            }
        }
        true
    }

    pub fn tick(&mut self, mob: &dyn Mob) {
        for prioritized_goal in &mut self.goals {
            if prioritized_goal.running
                && (Self::uses_any(prioritized_goal, self.disabled_controls)
                    || !prioritized_goal.should_continue(mob))
            {
                prioritized_goal.stop(mob);
            }
        }

        self.goals_by_control.iter_mut().for_each(|goal| {
            if *goal != usize::MAX && !self.goals[*goal].running {
                *goal = usize::MAX;
            }
        });

        for i in 0..self.goals.len() {
            if !self.goals[i].running
                && !Self::uses_any(&self.goals[i], self.disabled_controls)
                && self.can_replace_all(&self.goals[i])
                && self.goals[i].can_start(mob)
            {
                let controls = self.goals[i].controls();
                for control in Controls::ITER {
                    if controls.get(control) {
                        if let Some(goal) = self.get_goal_by_control(control) {
                            goal.stop(mob);
                        }
                        self.goals_by_control[control.idx()] = i;
                    }
                }
                self.goals[i].start(mob);
            }
        }

        self.tick_goals(mob, true);
    }

    pub fn tick_goals(&mut self, mob: &dyn Mob, tick_all: bool) {
        for prioritized_goal in &mut self.goals {
            if prioritized_goal.running && (tick_all || prioritized_goal.should_run_every_tick()) {
                prioritized_goal.tick(mob);
            }
        }
    }

    pub const fn disable_control(&mut self, control: Controls) {
        self.disabled_controls.set(control, true);
    }

    pub const fn enable_control(&mut self, control: Controls) {
        self.disabled_controls.set(control, false);
    }

    pub const fn set_control_enabled(&mut self, control: Controls, enabled: bool) {
        self.disabled_controls.set(control, !enabled);
    }

    fn get_goal_by_control(&mut self, control: Controls) -> Option<&mut PrioritizedGoal> {
        let i = self.goals_by_control[control.idx()];
        self.goals.get_mut(i)
    }
}

impl Default for GoalSelector {
    fn default() -> Self {
        Self {
            goals_by_control: [usize::MAX; 4],
            goals: Vec::default(),
            disabled_controls: Controls::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::mob::MobEntity;
    use std::sync::{Arc, Mutex};
    struct InputMob;
    impl Mob for InputMob {
        fn get_mob_entity(&self) -> &MobEntity {
            panic!("Selector must not inspect the scripted mob");
        }
    }
    struct Probe<const ID: usize> {
        state: Arc<Mutex<[bool; 3]>>,
        trace: Arc<Mutex<Vec<String>>>,
        controls: Controls,
        every: bool,
    }
    impl<const ID: usize> Probe<ID> {
        fn log(&self, event: &str) {
            self.trace.lock().unwrap().push(format!("{ID}:{event}"));
        }
    }
    impl<const ID: usize> Goal for Probe<ID> {
        fn can_start(&mut self, _: &dyn Mob) -> bool {
            self.log("can");
            self.state.lock().unwrap()[0]
        }
        fn should_continue(&self, _: &dyn Mob) -> bool {
            self.log("continue");
            self.state.lock().unwrap()[1]
        }
        fn can_stop(&self) -> bool {
            self.state.lock().unwrap()[2]
        }
        fn start(&mut self, _: &dyn Mob) {
            self.log("start");
        }
        fn stop(&mut self, _: &dyn Mob) {
            self.log("stop");
        }
        fn tick(&mut self, _: &dyn Mob) {
            self.log("tick");
        }
        fn should_run_every_tick(&self) -> bool {
            self.every
        }
        fn controls(&self) -> Controls {
            self.controls
        }
    }
    fn type_id(id: usize) -> TypeId {
        match id {
            0 => TypeId::of::<Probe<0>>(),
            1 => TypeId::of::<Probe<1>>(),
            2 => TypeId::of::<Probe<2>>(),
            3 => TypeId::of::<Probe<3>>(),
            4 => TypeId::of::<Probe<4>>(),
            5 => TypeId::of::<Probe<5>>(),
            _ => unreachable!(),
        }
    }
    #[test]
    fn java_goal_selector_lifecycle_and_insertion_order() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/goal-selector-java-26.2.json"
        ))
        .unwrap();
        for (case_index, c) in cases.as_array().unwrap().iter().enumerate() {
            let trace = Arc::new(Mutex::new(Vec::new()));
            let mut selector = GoalSelector::default();
            let states: Vec<_> = (0..6).map(|_| Arc::new(Mutex::new([true; 3]))).collect();
            for (id, g) in c["goals"].as_array().unwrap().iter().enumerate() {
                // Distinct type identities allow the production removal API to
                // remove the exact scripted goal, as the Java driver does.
                let probe = Probe::<0> {
                    state: states[id].clone(),
                    trace: trace.clone(),
                    controls: Controls(g["flags"].as_u64().unwrap() as u8),
                    every: g["every"].as_bool().unwrap(),
                };
                selector.goals.push(PrioritizedGoal::new(
                    type_id(id),
                    g["priority"].as_u64().unwrap() as u8,
                    match id {
                        0 => Box::new(probe),
                        1 => Box::new(Probe::<1> {
                            state: probe.state,
                            trace: probe.trace,
                            controls: probe.controls,
                            every: probe.every,
                        }),
                        2 => Box::new(Probe::<2> {
                            state: probe.state,
                            trace: probe.trace,
                            controls: probe.controls,
                            every: probe.every,
                        }),
                        3 => Box::new(Probe::<3> {
                            state: probe.state,
                            trace: probe.trace,
                            controls: probe.controls,
                            every: probe.every,
                        }),
                        4 => Box::new(Probe::<4> {
                            state: probe.state,
                            trace: probe.trace,
                            controls: probe.controls,
                            every: probe.every,
                        }),
                        5 => Box::new(Probe::<5> {
                            state: probe.state,
                            trace: probe.trace,
                            controls: probe.controls,
                            every: probe.every,
                        }),
                        _ => unreachable!(),
                    },
                ));
            }
            for (tick, step) in c["steps"].as_array().unwrap().iter().enumerate() {
                trace.lock().unwrap().clear();
                for (id, input) in step["states"].as_array().unwrap().iter().enumerate() {
                    *states[id].lock().unwrap() = serde_json::from_value(input.clone()).unwrap();
                }
                let removed = step["remove"].as_i64().unwrap();
                if removed >= 0 {
                    for goal in &mut selector.remove_goal_by_type_id(type_id(removed as usize)) {
                        goal.stop(&InputMob);
                    }
                }
                selector.disabled_controls = Controls(step["disabled"].as_u64().unwrap() as u8);
                if step["full"].as_bool().unwrap() {
                    selector.tick(&InputMob);
                } else {
                    selector.tick_goals(&InputMob, false);
                }
                let expected: Vec<String> = serde_json::from_value(step["trace"].clone()).unwrap();
                assert_eq!(
                    *trace.lock().unwrap(),
                    expected,
                    "case {case_index}, step {tick}"
                );
            }
        }
    }
}
