use pumpkin_protocol::{bedrock::client::set_time::CSetTime, java::client::play::CUpdateTime};

use super::World;

#[derive(Clone, Debug, PartialEq)]
pub struct ClockInstance {
    pub total_ticks: i64,
    pub partial_tick: f32,
    pub rate: f32,
    pub paused: bool,
}

impl Default for ClockInstance {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockInstance {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            total_ticks: 0,
            partial_tick: 0.0,
            rate: 1.0,
            paused: false,
        }
    }

    pub const fn load_from(
        &mut self,
        total_ticks: i64,
        partial_tick: f32,
        rate: f32,
        paused: bool,
    ) {
        self.total_ticks = total_ticks;
        self.partial_tick = partial_tick;
        self.rate = rate;
        self.paused = paused;
    }

    pub fn tick(&mut self) {
        if !self.paused {
            self.partial_tick += self.rate;
            let full_ticks = self.partial_tick.floor() as i32;
            self.partial_tick -= full_ticks as f32;
            self.total_ticks = self.total_ticks.wrapping_add(full_ticks as i64);
        }
    }

    pub const fn set_total_ticks(&mut self, total_ticks: i64) {
        self.total_ticks = total_ticks;
        self.partial_tick = 0.0;
    }

    pub fn add_ticks(&mut self, ticks: i64) {
        self.total_ticks = (self.total_ticks + ticks).max(0);
    }

    pub const fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub const fn set_rate(&mut self, rate: f32) {
        self.rate = rate;
    }

    #[must_use]
    pub const fn pack_network_state(&self, advance_time: bool) -> (i64, f32, f32) {
        let paused = self.paused || !advance_time;
        let rate = if paused { 0.0 } else { self.rate };
        (self.total_ticks, self.partial_tick, rate)
    }
}

#[derive(Clone, Debug)]
pub struct LevelTime {
    pub time_of_day: i64,
    pub world_age: i64,
    pub partial_tick: f32,
    pub rate: f32,
    pub paused: bool,
}

impl Default for LevelTime {
    fn default() -> Self {
        Self::new()
    }
}

impl LevelTime {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            time_of_day: 0,
            world_age: 0,
            partial_tick: 0.0,
            rate: 1.0,
            paused: false,
        }
    }

    pub fn from_level_data(data: &pumpkin_world::world_info::LevelData) -> Self {
        let clock = data
            .world_clocks
            .clocks
            .get("minecraft:overworld")
            .cloned()
            .unwrap_or_default();
        Self {
            world_age: data.game_time,
            time_of_day: data.day_time,
            partial_tick: clock.partial_tick,
            rate: clock.rate,
            paused: clock.paused,
        }
    }

    pub fn write_level_data(&self, data: &mut pumpkin_world::world_info::LevelData) {
        data.game_time = self.world_age;
        data.day_time = self.time_of_day;
        data.world_clocks.clocks.insert(
            "minecraft:overworld".into(),
            pumpkin_world::world_info::data_files::DimensionClock {
                total_ticks: self.time_of_day,
                partial_tick: self.partial_tick,
                rate: self.rate,
                paused: self.paused,
            },
        );
    }

    pub const fn load_from(&mut self, time_of_day: i64, world_age: i64) {
        self.time_of_day = time_of_day;
        self.world_age = world_age;
    }

    pub fn tick(&mut self, advance_time: bool) {
        self.world_age = self.world_age.wrapping_add(1);
        if advance_time && !self.paused {
            self.partial_tick += self.rate;
            let full_ticks = self.partial_tick.floor() as i32;
            self.partial_tick -= full_ticks as f32;
            self.time_of_day = self.time_of_day.wrapping_add(full_ticks as i64);
        }
    }

    pub fn send_time(&self, world: &World) {
        let advance_time = {
            let lock = world.level_info.load();
            lock.game_rules.advance_time
        };

        let (total_ticks, partial_tick, rate) = self.pack_network_state(advance_time);

        world.broadcast_editioned(
            &CUpdateTime::new_clock(self.world_age, 0, total_ticks, partial_tick, rate),
            &CSetTime::new(self.time_of_day as _), // TODO do we need to tell bedrock that time is frozen?
        );
    }

    pub fn add_time(&mut self, time: i64) {
        self.time_of_day = (self.time_of_day + time).max(0);
    }

    pub const fn set_time(&mut self, time: i64) {
        self.time_of_day = time;
        self.partial_tick = 0.0;
    }

    pub const fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    pub const fn set_rate(&mut self, rate: f32) {
        self.rate = rate;
    }

    #[must_use]
    pub const fn pack_network_state(&self, advance_time: bool) -> (i64, f32, f32) {
        let paused = self.paused || !advance_time;
        let rate = if paused { 0.0 } else { self.rate };
        (self.time_of_day, self.partial_tick, rate)
    }

    #[must_use]
    pub const fn query_daytime(&self) -> i64 {
        self.time_of_day % 24000
    }

    #[must_use]
    pub const fn query_gametime(&self) -> i64 {
        self.world_age
    }

    #[must_use]
    pub const fn query_day(&self) -> i64 {
        self.time_of_day / 24000
    }

    #[must_use]
    pub const fn is_night(&self) -> bool {
        (self.time_of_day % 24000) >= 12000 && (self.time_of_day % 24000) <= 23999
    }
}

impl World {
    pub(crate) fn sync_time_to_level_info(&self) {
        if self.dimension.minecraft_name
            != pumpkin_data::dimension::Dimension::OVERWORLD.minecraft_name
        {
            return;
        }
        let time = self
            .level_time
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        self.level_info.rcu(|current| {
            let mut data = (**current).clone();
            time.write_level_data(&mut data);
            data
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn world_restart_preserves_game_time_and_fractional_paused_clocks() {
        use arc_swap::ArcSwap;
        use pumpkin_data::dimension::Dimension;
        use pumpkin_util::world_seed::Seed;
        use pumpkin_world::{
            level::Level,
            world_info::{
                LevelData, WorldInfoReader, WorldInfoWriter, anvil::AnvilLevelInfo,
                data_files::DimensionClock,
            },
        };
        use std::sync::{Arc, Weak};
        let dir = tempfile::tempdir().unwrap();
        let mut data = LevelData::default(Seed(262));
        data.game_time = 1_000_000;
        data.day_time = 17_000;
        data.world_clocks.clocks.insert(
            "minecraft:overworld".into(),
            DimensionClock {
                total_ticks: 17_000,
                partial_tick: 0.25,
                rate: 0.5,
                paused: true,
            },
        );
        data.world_clocks.clocks.insert(
            "test:custom".into(),
            DimensionClock {
                total_ticks: 123,
                partial_tick: 0.75,
                rate: 2.0,
                paused: true,
            },
        );
        AnvilLevelInfo.write_world_info(&data, dir.path()).unwrap();
        let create = || {
            let level = Level::from_root_folder(
                &pumpkin_config::world::LevelConfig::default(),
                dir.path().to_path_buf(),
                262,
                Dimension::OVERWORLD,
            );
            World::load(
                level,
                Arc::new(ArcSwap::from_pointee(
                    AnvilLevelInfo.read_world_info(dir.path()).unwrap(),
                )),
                Dimension::OVERWORLD,
                Arc::new(crate::block::registry::BlockRegistry::default()),
                Weak::new(),
            )
        };
        let world = create();
        assert_eq!(world.get_world_age(), 1_000_000);
        assert_eq!(
            world
                .level
                .game_time
                .load(std::sync::atomic::Ordering::SeqCst),
            1_000_000
        );
        world.tick_environment();
        assert_eq!(world.level_time.lock().unwrap().time_of_day, 17_000);
        world.level_time.lock().unwrap().paused = false;
        world.tick_environment();
        world.tick_environment();
        assert_eq!(world.level_time.lock().unwrap().partial_tick, 0.25);
        assert_eq!(world.level_time.lock().unwrap().time_of_day, 17_001);
        world.level_info.rcu(|current| {
            let mut data = (**current).clone();
            data.game_rules.advance_time = false;
            data
        });
        world.tick_environment();
        assert_eq!(world.get_world_age(), 1_000_004);
        assert_eq!(world.level_time.lock().unwrap().time_of_day, 17_001);
        world.save().await.unwrap();
        AnvilLevelInfo
            .write_world_info(&world.level_info.load(), dir.path())
            .unwrap();
        world.shutdown().await;
        let restarted = create();
        let time = restarted.level_time.lock().unwrap().clone();
        assert_eq!(time.world_age, 1_000_004);
        assert_eq!(time.time_of_day, 17_001);
        assert_eq!(time.partial_tick, 0.25);
        assert_eq!(time.rate, 0.5);
        assert!(!time.paused);
        assert_eq!(
            restarted.level_info.load().world_clocks.clocks["test:custom"],
            data.world_clocks.clocks["test:custom"]
        );
        let root = pumpkin_nbt::nbt_compress::read_gzip_compound_tag(
            std::fs::File::open(dir.path().join("data/minecraft/world_clocks.dat")).unwrap(),
        )
        .unwrap();
        assert_eq!(root.get_int("DataVersion"), Some(4903));
        assert!(
            root.get_compound("data")
                .unwrap()
                .get_int("DataVersion")
                .is_none()
        );
        restarted.shutdown().await;
    }

    #[test]
    fn clock_instance_ticking() {
        let mut clock = ClockInstance::new();
        assert_eq!(clock.total_ticks, 0);
        assert_eq!(clock.partial_tick, 0.0);
        assert_eq!(clock.rate, 1.0);
        assert!(!clock.paused);

        // Standard tick at rate 1.0
        clock.tick();
        assert_eq!(clock.total_ticks, 1);
        assert_eq!(clock.partial_tick, 0.0);

        // Half rate tick
        clock.set_rate(0.5);
        clock.tick();
        assert_eq!(clock.total_ticks, 1);
        assert_eq!(clock.partial_tick, 0.5);
        clock.tick();
        assert_eq!(clock.total_ticks, 2);
        assert_eq!(clock.partial_tick, 0.0);

        // Double rate tick
        clock.set_rate(2.0);
        clock.tick();
        assert_eq!(clock.total_ticks, 4);
        assert_eq!(clock.partial_tick, 0.0);

        // Paused clock
        clock.set_paused(true);
        clock.tick();
        assert_eq!(clock.total_ticks, 4);
    }

    #[test]
    fn level_time_set_and_add() {
        let mut time = LevelTime::new();
        time.set_time(1000);
        assert_eq!(time.time_of_day, 1000);
        assert_eq!(time.partial_tick, 0.0);

        time.add_time(500);
        assert_eq!(time.time_of_day, 1500);

        time.add_time(-2000);
        assert_eq!(time.time_of_day, 0);
    }
}
