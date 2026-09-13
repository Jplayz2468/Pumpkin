//! Temporary, opt-in gameplay safeguards. Keep parity fixes separate from policy.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use pumpkin_config::local_safety::LocalSafetyConfig;
use pumpkin_data::Block;
use pumpkin_data::tag::Taggable;
use pumpkin_util::text::TextComponent;

use crate::entity::player::Player;
use crate::server::Server;

/// Per-connection warning state; no global player map or background timers.
#[derive(Default)]
pub struct SafetyNotices {
    last_redstone: Mutex<Option<Instant>>,
    last_shulker: Mutex<Option<Instant>>,
}

fn take_notice(last: &Mutex<Option<Instant>>, now: Instant, cooldown: Duration) -> bool {
    let mut last = last
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if last.is_some_and(|previous| now.saturating_duration_since(previous) < cooldown) {
        return false;
    }
    *last = Some(now);
    true
}

pub fn shulker_box_disabled(config: &LocalSafetyConfig, block: &Block) -> bool {
    config.disable_shulker_boxes && block.is_tagged_with("minecraft:shulker_boxes") == Some(true)
}

fn is_redstone_component(block: &Block) -> bool {
    let name = block.name;
    matches!(
        name,
        "redstone_wire"
            | "redstone_block"
            | "redstone_torch"
            | "redstone_wall_torch"
            | "redstone_lamp"
            | "repeater"
            | "comparator"
            | "observer"
            | "piston"
            | "sticky_piston"
            | "piston_head"
            | "moving_piston"
            | "lever"
            | "hopper"
            | "dropper"
            | "dispenser"
            | "crafter"
            | "daylight_detector"
            | "target"
            | "tripwire"
            | "tripwire_hook"
            | "trapped_chest"
            | "note_block"
            | "sculk_sensor"
            | "calibrated_sculk_sensor"
            | "sculk_shrieker"
            | "lightning_rod"
            | "rail"
            | "powered_rail"
            | "detector_rail"
            | "activator_rail"
            | "iron_door"
            | "iron_trapdoor"
    ) || name.ends_with("_button")
        || name.ends_with("_pressure_plate")
        || name.ends_with("copper_bulb")
}

/// Returns false before a restricted player action can mutate the world/items.
pub fn allow_block_action(player: &Player, server: &Server, block: &Block) -> bool {
    let config = &server.advanced_config.local_safety;
    if shulker_box_disabled(config, block) {
        if take_notice(
            &player.safety_notices.last_shulker,
            Instant::now(),
            Duration::from_secs(5),
        ) {
            player.send_system_message(&TextComponent::text(
                "Shulker boxes are temporarily disabled: stored items may be lost when a box is broken and placed again. Please use a chest or barrel.",
            ));
        }
        return false;
    }
    if config.warn_redstone
        && is_redstone_component(block)
        && take_notice(
            &player.safety_notices.last_redstone,
            Instant::now(),
            Duration::from_secs(config.redstone_warning_cooldown_seconds),
        )
    {
        player.send_system_message(&TextComponent::text(
            "Redstone warning: pistons, update timing, and machines may behave differently from vanilla Java. Test your build before relying on it.",
        ));
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_safety_covers_every_shulker_color_without_blocking_other_storage() {
        let config = LocalSafetyConfig {
            disable_shulker_boxes: true,
            ..Default::default()
        };
        for block in [
            &Block::SHULKER_BOX,
            &Block::WHITE_SHULKER_BOX,
            &Block::ORANGE_SHULKER_BOX,
            &Block::MAGENTA_SHULKER_BOX,
            &Block::LIGHT_BLUE_SHULKER_BOX,
            &Block::YELLOW_SHULKER_BOX,
            &Block::LIME_SHULKER_BOX,
            &Block::PINK_SHULKER_BOX,
            &Block::GRAY_SHULKER_BOX,
            &Block::LIGHT_GRAY_SHULKER_BOX,
            &Block::CYAN_SHULKER_BOX,
            &Block::PURPLE_SHULKER_BOX,
            &Block::BLUE_SHULKER_BOX,
            &Block::BROWN_SHULKER_BOX,
            &Block::GREEN_SHULKER_BOX,
            &Block::RED_SHULKER_BOX,
            &Block::BLACK_SHULKER_BOX,
        ] {
            assert!(shulker_box_disabled(&config, block), "{}", block.name);
            assert!(!shulker_box_disabled(&LocalSafetyConfig::default(), block));
        }
        for block in [
            &Block::CHEST,
            &Block::BARREL,
            &Block::ENDER_CHEST,
            &Block::STONE,
        ] {
            assert!(!shulker_box_disabled(&config, block));
        }
    }

    #[test]
    fn local_safety_redstone_families_exclude_ordinary_building_and_ores() {
        for block in [
            &Block::REDSTONE_WIRE,
            &Block::STICKY_PISTON,
            &Block::HOPPER,
            &Block::CRAFTER,
            &Block::OAK_BUTTON,
            &Block::HEAVY_WEIGHTED_PRESSURE_PLATE,
            &Block::WAXED_OXIDIZED_COPPER_BULB,
            &Block::DETECTOR_RAIL,
        ] {
            assert!(is_redstone_component(block), "{}", block.name);
        }
        for block in [
            &Block::STONE,
            &Block::CHEST,
            &Block::REDSTONE_ORE,
            &Block::OAK_PLANKS,
        ] {
            assert!(!is_redstone_component(block), "{}", block.name);
        }
    }

    #[test]
    fn local_safety_notices_are_independent_and_rate_limited() {
        let notices = SafetyNotices::default();
        let now = Instant::now();
        let cooldown = Duration::from_secs(30);
        assert!(take_notice(&notices.last_redstone, now, cooldown));
        assert!(!take_notice(
            &notices.last_redstone,
            now + Duration::from_secs(29),
            cooldown
        ));
        assert!(take_notice(&notices.last_shulker, now, cooldown));
        assert!(take_notice(
            &notices.last_redstone,
            now + cooldown,
            cooldown
        ));
        assert!(take_notice(
            &SafetyNotices::default().last_redstone,
            now,
            cooldown
        ));
    }
}
