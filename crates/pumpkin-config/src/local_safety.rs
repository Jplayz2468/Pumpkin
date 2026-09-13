use serde::{Deserialize, Serialize};

/// Temporary restrictions while this fork works toward vanilla gameplay parity.
#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct LocalSafetyConfig {
    /// Prevent player placement, opening, and breaking of all shulker boxes.
    pub disable_shulker_boxes: bool,
    /// Warn on player placement, use, or breaking of redstone components.
    pub warn_redstone: bool,
    /// Minimum seconds between redstone warnings for each connected player.
    pub redstone_warning_cooldown_seconds: u64,
}

impl Default for LocalSafetyConfig {
    fn default() -> Self {
        Self {
            disable_shulker_boxes: false,
            warn_redstone: false,
            redstone_warning_cooldown_seconds: 30,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::PumpkinConfig;

    #[test]
    fn local_safety_old_configs_keep_upstream_behavior() {
        let config: PumpkinConfig = toml::from_str("").unwrap();
        assert!(!config.advanced.local_safety.disable_shulker_boxes);
        assert!(!config.advanced.local_safety.warn_redstone);
    }

    #[test]
    fn local_safety_flags_are_independent_and_survive_round_trip() {
        let config: PumpkinConfig = toml::from_str(
            "[local_safety]\ndisable_shulker_boxes = true\nwarn_redstone = false\nredstone_warning_cooldown_seconds = 15",
        )
        .unwrap();
        let saved = toml::to_string(&config).unwrap();
        let restored: PumpkinConfig = toml::from_str(&saved).unwrap();
        assert!(restored.advanced.local_safety.disable_shulker_boxes);
        assert!(!restored.advanced.local_safety.warn_redstone);
        assert_eq!(
            restored
                .advanced
                .local_safety
                .redstone_warning_cooldown_seconds,
            15
        );
    }
}
