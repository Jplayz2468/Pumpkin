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
    /// Attach the Brain framework to zombies.
    ///
    /// Vanilla zombies are a goal mob, not a brain mob, so this is a deliberate
    /// divergence: it exists to exercise the Brain framework against a live server using
    /// a mob that is easy to spawn and observe. It runs alongside the zombie's normal
    /// goals and does not replace them.
    pub zombie_brain_lab: bool,
    /// Give every mob except zombies no AI at all, so a test world is quiet enough to
    /// watch one mob's behaviour without interference. Deliberate divergence.
    pub only_zombie_ai: bool,
}

impl Default for LocalSafetyConfig {
    fn default() -> Self {
        Self {
            disable_shulker_boxes: false,
            warn_redstone: false,
            redstone_warning_cooldown_seconds: 30,
            zombie_brain_lab: false,
            only_zombie_ai: false,
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
        // The lab switches are divergences from vanilla and must stay off by default.
        assert!(!config.advanced.local_safety.zombie_brain_lab);
        assert!(!config.advanced.local_safety.only_zombie_ai);
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
