//! Java gliding is shared entity flag 7, independent of the flying ability.
use super::*;
use pumpkin_data::data_component::DataComponent;

/// Entity.checkFallDistanceAccumulation in Mojang Java 26.2.
fn glide_fall_distance(distance: f64, vertical_speed: f64) -> f64 {
    if vertical_speed > -0.5 && distance > 1.0 {
        1.0
    } else {
        distance
    }
}

/// LivingEntity.handleFallFlyingCollisions in Mojang Java 26.2.
pub(super) fn wall_damage(before: f64, after: f64) -> f32 {
    ((before - after) * 10.0 - 3.0) as f32
}

fn start_gliding_requested(gliding: bool, can_glide: bool, in_water: bool) -> bool {
    !gliding && can_glide && !in_water
}

fn can_glide_using(stack: &ItemStack, slot: &EquipmentSlot) -> bool {
    !stack.is_empty()
        && stack.has_data_component(DataComponent::Glider)
        && stack
            .get_data_component::<EquippableImpl>()
            .is_some_and(|c| c.slot == slot)
        && !(stack.is_damageable()
            && !stack.is_unbreakable()
            && stack.get_damage() >= stack.get_max_damage().unwrap_or(0) - 1)
}

impl Player {
    pub fn accepts_movement(&self) -> bool {
        self.has_client_loaded()
            && self.living_entity.health.load() > 0.0
            && !self.living_entity.dead.load(Ordering::Relaxed)
            && !self.is_movement_locked.load(Ordering::Relaxed)
            && self
                .awaiting_teleport
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_none()
            && !self.get_entity().has_vehicle()
    }

    pub fn can_glide(&self) -> bool {
        let entity = self.get_entity();
        self.living_entity.health.load() > 0.0
            && !self.living_entity.dead.load(Ordering::Relaxed)
            && !self.is_flying()
            && !entity.on_ground.load(Ordering::Relaxed)
            && !entity.has_vehicle()
            && !self.living_entity.has_effect(&StatusEffect::LEVITATION)
            && self
                .inventory
                .equipment_slots
                .iter()
                .any(|(index, slot)| can_glide_using(&self.inventory.get_slot(*index), slot))
    }

    pub fn can_start_gliding(&self) -> bool {
        start_gliding_requested(
            self.get_entity().is_fall_flying(),
            self.can_glide(),
            self.get_entity().touching_water.load(Ordering::Relaxed),
        )
    }

    pub fn update_gliding(&self) {
        let entity = self.get_entity();
        if entity.is_fall_flying() && !self.can_glide() {
            entity.set_fall_flying(false);
            self.update_player_pose();
        }
    }

    pub fn reset_flight_after_respawn(&self) {
        let mut abilities = self
            .abilities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        abilities.reset_flight_after_respawn(self.gamemode.load());
        drop(abilities);
        self.get_entity().movement.store(Vector3::default());
        self.last_client_movement.store(Vector3::default());
        self.send_abilities_update();
    }

    /// Common landing/gliding path for every Java movement packet variant.
    /// Player movement is client-authoritative in this fork: use the previous
    /// accepted movement for the pre-impact speed, never the truncated landing Y.
    pub fn handle_flight_movement(
        &self,
        delta: Vector3<f64>,
        previous: Vector3<f64>,
        ground: bool,
        horizontal_collision: bool,
    ) {
        if self.living_entity.health.load() <= 0.0
            || self.living_entity.dead.load(Ordering::Relaxed)
        {
            return;
        }
        let entity = self.get_entity();
        let gliding = entity.is_fall_flying();
        if gliding {
            self.living_entity.fall_distance.store(glide_fall_distance(
                self.living_entity.fall_distance.load(),
                previous.y,
            ));
            // Require a real obstruction as well as the collision report. A speed
            // reduction in open air (turning/braking) must never cause wall damage.
            if horizontal_collision && previous.horizontal_length() > 0.0 {
                let world = self.world();
                let bounds = entity.bounding_box.load();
                let blocked = |direction| {
                    !world
                        .get_block_collisions(
                            bounds.offset(BoundingBox::new(direction, direction)),
                            self,
                        )
                        .0
                        .is_empty()
                };
                let hit_x = previous.x != 0.0
                    && blocked(Vector3::new(previous.x.signum() * 1.0E-5, 0.0, 0.0));
                let hit_z = previous.z != 0.0
                    && blocked(Vector3::new(0.0, 0.0, previous.z.signum() * 1.0E-5));
                if hit_x || hit_z {
                    // Collision zeroes the blocked velocity component. The clipped
                    // displacement to the wall is not the post-collision velocity.
                    let remaining = Vector3::new(
                        if hit_x { 0.0 } else { delta.x },
                        0.0,
                        if hit_z { 0.0 } else { delta.z },
                    );
                    let damage =
                        wall_damage(previous.horizontal_length(), remaining.horizontal_length());
                    if damage > 0.0 {
                        self.living_entity
                            .damage(self, damage, DamageType::FLY_INTO_WALL);
                    }
                }
            }
        }
        entity.set_on_ground_with_movement(self, ground, Some(delta));
        if !self.is_flying() {
            self.living_entity.fall(
                self,
                delta.y,
                ground,
                self.gamemode.load() == GameMode::Creative,
            );
        }
        self.living_entity.check_climbing(self);
        let landed_in_liquid = entity.velocity.load().y < f64::from(1.0e-5_f32)
            && (entity.is_in_water() || entity.is_in_lava());
        if ground || landed_in_liquid || self.living_entity.climbing.load(Ordering::Relaxed)
            || self.gamemode.load() == GameMode::Spectator || entity.is_fall_flying()
            || Self::is_auto_spin_attack() {
            self.living_entity.try_reset_impulse_context();
        }
        self.update_gliding();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::living::calculate_fall_damage;
    use serde_json::Value;

    fn oracle() -> Value {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/flight-java-26.2.json"
        ))
        .unwrap()
    }

    #[test]
    fn java_glide_start_and_stop_transitions() {
        for c in oracle()["starts"].as_array().unwrap() {
            assert_eq!(
                start_gliding_requested(
                    c["gliding"].as_bool().unwrap(),
                    c["can"].as_bool().unwrap(),
                    c["water"].as_bool().unwrap()
                ),
                c["expected"].as_bool().unwrap(),
                "{c}"
            );
        }
    }

    #[test]
    fn java_glide_accumulation_thresholds() {
        for c in oracle()["caps"].as_array().unwrap() {
            assert_eq!(
                glide_fall_distance(
                    // This older fixture supplies float inputs promoted to Java double.
                    f64::from(c["distance"].as_f64().unwrap() as f32),
                    c["y"].as_f64().unwrap()
                ),
                c["expected"].as_f64().unwrap(),
                "{c}"
            );
        }
    }

    #[test]
    fn java_landing_damage_and_attributes() {
        for c in oracle()["falls"].as_array().unwrap() {
            assert_eq!(
                calculate_fall_damage(
                    c["distance"].as_f64().unwrap(),
                    c["safe"].as_f64().unwrap(),
                    1.0,
                    c["multiplier"].as_f64().unwrap()
                ),
                c["expected"].as_f64().unwrap() as f32,
                "{c}"
            );
        }
    }

    #[test]
    fn java_wall_collision_damage() {
        for c in oracle()["collisions"].as_array().unwrap() {
            let damage = if c["collision"].as_bool().unwrap() {
                wall_damage(c["before"].as_f64().unwrap(), c["after"].as_f64().unwrap()).max(0.0)
            } else {
                0.0
            };
            assert_eq!(damage, c["expected"].as_f64().unwrap() as f32, "{c}");
        }
    }

    #[test]
    fn java_glider_equipment_slots_and_durability() {
        for c in oracle()["equipment"].as_array().unwrap() {
            let slot = match c["slot"].as_str().unwrap() {
                "chest" => EquipmentSlot::CHEST,
                "head" => EquipmentSlot::HEAD,
                "offhand" => EquipmentSlot::OFF_HAND,
                s => panic!("{s}"),
            };
            let mut stack = ItemStack::new(1, &pumpkin_data::item::Item::ELYTRA);
            stack.set_damage(c["damage"].as_i64().unwrap() as i32);
            assert_eq!(
                can_glide_using(&stack, &slot),
                c["expected"].as_bool().unwrap(),
                "{c}"
            );
        }
        assert!(!can_glide_using(&ItemStack::EMPTY, &EquipmentSlot::CHEST));
        assert!(!can_glide_using(
            &ItemStack::new(1, &pumpkin_data::item::Item::DIAMOND_CHESTPLATE),
            &EquipmentSlot::CHEST
        ));
    }

    #[test]
    fn gentle_glide_caps_prior_fall_but_steep_dive_still_hurts() {
        let gentle = glide_fall_distance(40.0, -0.1);
        assert!(calculate_fall_damage(f64::from(gentle), 3.0, 1.0, 1.0) <= 0.0);
        let dive = glide_fall_distance(40.0, -0.5);
        assert_eq!(calculate_fall_damage(f64::from(dive), 3.0, 1.0, 1.0), 37.0);
        // A short final landing displacement must not replace the incoming speed.
        assert_eq!(glide_fall_distance(40.0, -2.0), 40.0);
    }

    #[test]
    fn death_respawn_resets_ability_flight_for_each_gamemode() {
        for (mode, flying, allowed) in [
            (GameMode::Survival, false, false),
            (GameMode::Adventure, false, false),
            (GameMode::Creative, false, true),
            (GameMode::Spectator, true, true),
        ] {
            let mut abilities = Abilities {
                flying: true,
                allow_flying: true,
                ..Abilities::default()
            };
            abilities.reset_flight_after_respawn(mode);
            assert_eq!(
                (abilities.flying, abilities.allow_flying),
                (flying, allowed)
            );
        }
        let mut abilities = Abilities {
            flying: true,
            ..Abilities::default()
        };
        abilities.reset_flight_after_respawn(GameMode::Creative);
        assert!(!abilities.flying);
        abilities.set_for_gamemode(GameMode::Survival);
        assert!(!abilities.flying && !abilities.allow_flying);
    }
}
