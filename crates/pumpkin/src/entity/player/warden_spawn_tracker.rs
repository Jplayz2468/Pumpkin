//! Player-owned shrieker warning state, following Java 26.2 WardenSpawnTracker.
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WardenSpawnTracker {
    pub ticks_since_last_warning: i32,
    pub warning_level: i32,
    pub cooldown_ticks: i32,
}

impl WardenSpawnTracker {
    pub fn eligible_nearby_player(delta: [f64; 3], alive: bool, spectator: bool) -> bool {
        !spectator
            && delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2] < 256.0
            && alive
    }

    pub fn tick(&mut self) {
        // Java checks before incrementing: starting at zero, decay is tick 12001.
        if self.ticks_since_last_warning >= 12_000 {
            self.set_warning_level(self.warning_level.wrapping_sub(1));
            self.ticks_since_last_warning = 0;
        } else {
            self.ticks_since_last_warning = self.ticks_since_last_warning.wrapping_add(1);
        }
        if self.cooldown_ticks > 0 {
            self.cooldown_ticks -= 1;
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub const fn on_cooldown(&self) -> bool {
        self.cooldown_ticks > 0
    }

    pub fn increase_warning_level(&mut self) {
        if !self.on_cooldown() {
            self.ticks_since_last_warning = 0;
            self.cooldown_ticks = 200;
            self.set_warning_level(self.warning_level.wrapping_add(1));
        }
    }

    pub fn set_warning_level(&mut self, level: i32) {
        self.warning_level = level.clamp(0, 4);
    }

    /// NbtOps reads all numeric tags through Number.intValue before validating.
    /// A malformed present field rejects the entire codec; absent fields default.
    pub fn from_nbt(nbt: Option<&NbtCompound>) -> Self {
        fn number(nbt: &NbtCompound, key: &str) -> Option<i32> {
            let value = match nbt.get(key) {
                None => 0,
                Some(NbtTag::Byte(v)) => i32::from(*v),
                Some(NbtTag::Short(v)) => i32::from(*v),
                Some(NbtTag::Int(v)) => *v,
                Some(NbtTag::Long(v)) => *v as i32,
                Some(NbtTag::Float(v)) => *v as i32,
                Some(NbtTag::Double(v)) => *v as i32,
                _ => return None,
            };
            (value >= 0).then_some(value)
        }
        let read = || {
            let nbt = nbt?;
            Some(Self {
                ticks_since_last_warning: number(nbt, "ticks_since_last_warning")?,
                warning_level: number(nbt, "warning_level")?,
                cooldown_ticks: number(nbt, "cooldown_ticks")?,
            })
        };
        read().unwrap_or_default()
    }

    pub fn to_nbt(self) -> NbtCompound {
        let mut nbt = NbtCompound::new();
        nbt.put_int("ticks_since_last_warning", self.ticks_since_last_warning);
        nbt.put_int("warning_level", self.warning_level);
        nbt.put_int("cooldown_ticks", self.cooldown_ticks);
        nbt
    }

    /// Caller supplies eligible nearby players plus the triggering player once.
    /// Select Java's first maximum, then copy all three fields to every player.
    pub fn try_warn_group(trackers: &mut [Self], nearby_warden: bool) -> Option<i32> {
        if nearby_warden || trackers.iter().any(Self::on_cooldown) {
            return None;
        }
        let mut highest = *trackers.first()?;
        for tracker in &trackers[1..] {
            if tracker.warning_level > highest.warning_level {
                highest = *tracker;
            }
        }
        highest.increase_warning_level();
        trackers.fill(highest);
        Some(highest.warning_level)
    }
}
