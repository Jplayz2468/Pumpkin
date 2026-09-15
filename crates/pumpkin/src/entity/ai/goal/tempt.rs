use std::sync::Arc;

use super::{Controls, Goal, to_goal_ticks};
use crate::entity::EntityBase;
use crate::entity::{ai::pathfinder::NavigatorGoal, mob::Mob, player::Player};
use pumpkin_data::attributes::Attributes;
use pumpkin_data::item::Item;

/// `TemptGoal.stop`'s cooldown (TemptGoal.java:107) is `reducedTickDelay(100)`, not a raw
/// 100 — `to_goal_ticks` is Pumpkin's port of `reducedTickDelay` (`ceil(n / 2)`).
const CALM_DOWN_TICKS: i32 = 100;
const STOP_DISTANCE: f64 = 2.5;

pub struct TemptGoal {
    goal_control: Controls,
    speed: f64,
    tempt_items: &'static [&'static Item],
    target_player: Option<Arc<Player>>,
    cooldown: i32,
}

impl TemptGoal {
    #[must_use]
    pub fn new(speed: f64, tempt_items: &'static [&'static Item]) -> Self {
        Self {
            goal_control: Controls::MOVE | Controls::LOOK,
            speed,
            tempt_items,
            target_player: None,
            cooldown: 0,
        }
    }

    fn is_tempt_item(&self, stack: &pumpkin_data::item_stack::ItemStack) -> bool {
        stack.item_count > 0 && self.tempt_items.iter().any(|i| i.id == stack.item.id)
    }

    fn is_holding_tempt_item(&self, player: &Player) -> bool {
        let main = player.inventory().held_item();
        if self.is_tempt_item(&main) {
            return true;
        }
        let off = player.inventory().off_hand_item();
        self.is_tempt_item(&off)
    }

    /// `TemptGoal.canUse` (TemptGoal.java:57-59) uses `getNearestPlayer`, i.e. the
    /// *closest* matching player, not merely the first one found in iteration order.
    fn find_tempting_player(&self, mob: &dyn Mob) -> Option<Arc<Player>> {
        let mob_entity = mob.get_mob_entity();
        let pos = mob_entity.living_entity.entity.pos.load();
        let world = mob_entity.living_entity.entity.world.load();
        let range = mob_entity
            .living_entity
            .get_attribute_value(&Attributes::TEMPT_RANGE);

        world
            .get_nearby_players(pos, range)
            .into_iter()
            .filter(|player| self.is_holding_tempt_item(player))
            .min_by(|a, b| {
                a.get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&pos)
                    .total_cmp(&b.get_entity().pos.load().squared_distance_to_vec(&pos))
            })
    }

    fn is_player_still_tempting(&self, player: &Player, mob: &dyn Mob) -> bool {
        let mob_entity = mob.get_mob_entity();
        let mob_pos = mob_entity.living_entity.entity.pos.load();
        let player_pos = player.get_entity().pos.load();
        let range = mob_entity
            .living_entity
            .get_attribute_value(&Attributes::TEMPT_RANGE);
        if mob_pos.squared_distance_to_vec(&player_pos) > range * range {
            return false;
        }
        self.is_holding_tempt_item(player)
    }
}

impl Goal for TemptGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.cooldown > 0 {
            self.cooldown -= 1;
            return false;
        }
        self.target_player = self.find_tempting_player(mob);
        self.target_player.is_some()
    }

    fn should_continue(&self, mob: &dyn Mob) -> bool {
        self.target_player
            .as_ref()
            .is_some_and(|player| self.is_player_still_tempting(player, mob))
    }

    fn tick(&mut self, mob: &dyn Mob) {
        if let Some(player) = &self.target_player {
            let mob_entity = mob.get_mob_entity();
            let player_pos = player.get_entity().pos.load();

            mob_entity
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at(
                    mob,
                    player_pos.x,
                    player.get_entity().get_eye_y(),
                    player_pos.z,
                );

            let mob_pos = mob_entity.living_entity.entity.pos.load();
            if mob_pos.squared_distance_to_vec(&player_pos) > STOP_DISTANCE * STOP_DISTANCE {
                let mut navigator = mob_entity
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                navigator.set_progress(NavigatorGoal::new(mob_pos, player_pos, self.speed));
            }
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        self.target_player = None;
        self.cooldown = to_goal_ticks(CALM_DOWN_TICKS);
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
