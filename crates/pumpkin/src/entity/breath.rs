use crate::entity::EntityBase;
use crate::entity::player::Player;
use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::tag::Taggable;
use pumpkin_protocol::codec::var_int::VarInt;
use std::sync::atomic::{AtomicI32, Ordering};

pub const MAX_AIR: i32 = 300;
pub const AIR_RECOVERY_RATE: i32 = 4;
pub const AIR_DEPLETION_RATE: i32 = 1;
pub const DROWNING_INTERVAL: i32 = 20;
pub const DROWNING_DAMAGE: f32 = 2.0;

pub struct BreathManager {
    pub air_supply: AtomicI32,
    pub drowning_tick: AtomicI32,
}

impl Default for BreathManager {
    fn default() -> Self {
        Self {
            air_supply: AtomicI32::new(MAX_AIR),
            drowning_tick: AtomicI32::new(0),
        }
    }
}

impl BreathManager {
    pub fn tick(&self, player: &Player) {
        if player.living_entity.health.load() <= 0.0 {
            return;
        }
        let entity = player.get_entity();
        let world = player.world();
        let in_water = entity.is_submerged_in_water()
            && world.get_block(&pumpkin_util::math::position::BlockPos::floored_v(
                player.eye_position(),
            )) != &pumpkin_data::Block::BUBBLE_COLUMN;
        let water_breathing = player
            .living_entity
            .has_effect(&StatusEffect::WATER_BREATHING);
        let conduit = player
            .living_entity
            .has_effect(&StatusEffect::CONDUIT_POWER);
        let nautilus = player
            .living_entity
            .has_effect(&StatusEffect::BREATH_OF_THE_NAUTILUS);
        let invulnerable = player
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .invulnerable;
        let underwater_breathing = entity
            .entity_type
            .is_tagged_with("minecraft:can_breathe_under_water")
            .unwrap_or(false);
        let can_drown = in_water
            && !underwater_breathing
            && !water_breathing
            && !conduit
            && !nautilus
            && !invulnerable;
        let previous = self.air_supply.load(Ordering::Relaxed);
        let next = if can_drown {
            let oxygen_bonus = player
                .living_entity
                .get_attribute_value(&pumpkin_data::attributes::Attributes::OXYGEN_BONUS);
            // Entity-owned Java random initialization remains a shared engine dependency.
            if oxygen_bonus > 0.0 && rand::random::<f64>() >= 1.0 / (oxygen_bonus + 1.0) {
                previous
            } else {
                previous.wrapping_sub(AIR_DEPLETION_RATE)
            }
        } else if previous < MAX_AIR && (!in_water || !nautilus || water_breathing || conduit) {
            (previous + AIR_RECOVERY_RATE).min(MAX_AIR)
        } else {
            previous
        };
        if !self.change_air(player, next) {
            return;
        }
        if can_drown && self.air_supply.load(Ordering::Relaxed) <= -DROWNING_INTERVAL {
            self.change_air(player, 0);
            world.send_entity_status(
                entity,
                pumpkin_data::entity::EntityStatus::DrownParticles,
                None,
            );
            if world.level_info.load().game_rules.drowning_damage {
                player
                    .living_entity
                    .damage(player, DROWNING_DAMAGE, DamageType::DROWN);
            }
        }
        self.drowning_tick.store(
            self.air_supply
                .load(Ordering::Relaxed)
                .wrapping_neg()
                .clamp(0, DROWNING_INTERVAL - 1),
            Ordering::Relaxed,
        );
    }

    fn change_air(&self, player: &Player, mut amount: i32) -> bool {
        if amount == self.air_supply.load(Ordering::Relaxed) {
            return true;
        }
        if let Some(server) = player.world().server.upgrade() {
            let mut event =
                crate::plugin::api::events::entity::entity_air_change::EntityAirChangeEvent::new(
                    player.entity_id(),
                    amount,
                );
            server.plugin_manager.fire_blocking(&server, &mut event);
            if event.cancelled {
                return false;
            }
            amount = event.amount;
        }
        self.air_supply.store(amount, Ordering::Relaxed);
        self.send_air_supply(player);
        true
    }

    pub fn send_air_supply(&self, player: &Player) {
        let air = self.air_supply.load(Ordering::Relaxed);

        let mut bedrock_meta =
            pumpkin_protocol::bedrock::client::set_actor_data::SyncedActorDataList::new();
        bedrock_meta.set(
            pumpkin_protocol::bedrock::client::set_actor_data::entity_data_key::AIR_SUPPLY,
            pumpkin_protocol::bedrock::client::set_actor_data::MetadataValue::Short(
                air.clamp(0, MAX_AIR) as i16,
            ),
        );

        player.get_entity().set_synced_data(
            pumpkin_data::tracked_data::entity::DATA_AIR_SUPPLY_ID,
            VarInt(air),
        );
        player.get_entity().send_bedrock_actor_data(&bedrock_meta);
    }

    pub fn reset(&self, player: &Player) {
        self.air_supply.store(MAX_AIR, Ordering::Relaxed);
        self.send_air_supply(player);
        self.drowning_tick.store(0, Ordering::Relaxed);
    }
}
