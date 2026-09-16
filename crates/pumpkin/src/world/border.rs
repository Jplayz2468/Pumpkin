use pumpkin_protocol::java::client::play::{
    CInitializeWorldBorder, CSetBorderCenter, CSetBorderLerpSize, CSetBorderSize,
    CSetBorderWarningDelay, CSetBorderWarningDistance,
};
use pumpkin_util::math::{boundingbox::BoundingBox, vector3::Vector3};

use crate::net::java::JavaClient;

use super::World;

pub struct Worldborder {
    pub center_x: f64,
    pub center_z: f64,
    pub old_diameter: f64,
    pub new_diameter: f64,
    pub speed: i64,
    pub portal_teleport_boundary: i32,
    pub warning_blocks: i32,
    pub warning_time: i32,
    pub damage_per_block: f64,
    pub buffer: f64,
    current_diameter: f64,
    previous_diameter: f64,
    lerp_duration: i64,
    lerp_remaining: i64,
}

impl Worldborder {
    #[must_use]
    pub const fn new(
        x: f64,
        z: f64,
        diameter: f64,
        speed: i64,
        warning_blocks: i32,
        warning_time: i32,
    ) -> Self {
        Self {
            center_x: x,
            center_z: z,
            old_diameter: diameter,
            new_diameter: diameter,
            speed,
            portal_teleport_boundary: 29_999_984,
            warning_blocks,
            warning_time,
            damage_per_block: 0.2,
            buffer: 5.0,
            current_diameter: diameter,
            previous_diameter: diameter,
            lerp_duration: 0,
            lerp_remaining: 0,
        }
    }

    pub fn load(folder: &std::path::Path, legacy: &pumpkin_world::world_info::LevelData) -> Self {
        let mut border = Self::new(
            legacy.border_center_x,
            legacy.border_center_z,
            legacy.border_size,
            0,
            legacy.border_warning_blocks as i32,
            (legacy.border_warning_time * 20.0) as i32,
        );
        border.damage_per_block = legacy.border_damage_per_block;
        border.buffer = legacy.border_safe_zone;
        border.start_size_change(legacy.border_size_lerp_target, legacy.border_size_lerp_time);
        if legacy.border_size_lerp_time <= 0 {
            border.start_size_change(legacy.border_size, 0);
        }
        let path = pumpkin_world::world_info::data_files::minecraft_data_dir(folder)
            .join("world_border.dat");
        if path.exists() {
            match std::fs::File::open(&path)
                .map_err(|error| error.to_string())
                .and_then(|file| {
                    pumpkin_nbt::nbt_compress::read_gzip_compound_tag(file)
                        .map_err(|error| error.to_string())
                }) {
                Ok(root) => {
                    if let Some(data) = root.get_compound("data") {
                        border.read_saved(data);
                    }
                }
                Err(error) => tracing::warn!("Failed to read {}: {error}", path.display()),
            }
        }
        border
    }

    fn read_saved(&mut self, data: &pumpkin_nbt::compound::NbtCompound) {
        self.center_x = data.get_double("center_x").unwrap_or(self.center_x);
        self.center_z = data.get_double("center_z").unwrap_or(self.center_z);
        self.damage_per_block = data
            .get_double("damage_per_block")
            .unwrap_or(self.damage_per_block);
        self.buffer = data.get_double("safe_zone").unwrap_or(self.buffer);
        self.warning_blocks = data
            .get_int("warning_blocks")
            .unwrap_or(self.warning_blocks);
        self.warning_time = data.get_int("warning_time").unwrap_or(self.warning_time);
        let size = data.get_double("size").unwrap_or(self.diameter());
        self.start_size_change(size, 0);
        let remaining = data.get_long("lerp_time").unwrap_or(0);
        if remaining > 0 {
            self.start_size_change(
                data.get_double("lerp_target").unwrap_or(size),
                remaining.saturating_mul(50),
            );
        }
    }

    fn saved_data(&self) -> pumpkin_nbt::compound::NbtCompound {
        let mut data = pumpkin_nbt::compound::NbtCompound::new();
        data.put_double("center_x", self.center_x);
        data.put_double("center_z", self.center_z);
        data.put_double("damage_per_block", self.damage_per_block);
        data.put_double("safe_zone", self.buffer);
        data.put_int("warning_blocks", self.warning_blocks);
        data.put_int("warning_time", self.warning_time);
        data.put_double("size", self.diameter());
        data.put_long("lerp_time", self.lerp_remaining);
        data.put_double("lerp_target", self.new_diameter);
        data
    }

    pub fn save(&self, folder: &std::path::Path) -> Result<(), String> {
        let dir = pumpkin_world::world_info::data_files::ensure_minecraft_data_dir(folder)
            .map_err(|error| error.to_string())?;
        let path = dir.join("world_border.dat");
        let temporary = dir.join("world_border.dat.tmp");
        let mut root = pumpkin_nbt::compound::NbtCompound::new();
        root.put_int("DataVersion", 4903);
        root.put_compound("data", self.saved_data());
        let bytes = pumpkin_nbt::nbt_compress::write_gzip_compound_tag_to_bytes(root)
            .map_err(|error| error.to_string())?;
        std::fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
        std::fs::rename(temporary, path).map_err(|error| error.to_string())
    }

    pub fn damage_at(&self, position: Vector3<f64>, bounds: BoundingBox) -> Option<f32> {
        if self.contains_box(&bounds) || self.damage_per_block <= 0.0 {
            return None;
        }
        let [min_x, min_z, max_x, max_z] = self.bounds();
        let distance = (position.x - min_x)
            .min(max_x - position.x)
            .min(position.z - min_z)
            .min(max_z - position.z)
            + self.buffer;
        (distance < 0.0).then(|| ((-distance * self.damage_per_block).floor() as i32).max(1) as f32)
    }

    pub fn init_client(&self, client: &JavaClient) {
        if let Ok(data) = client.serialize_packet(&CInitializeWorldBorder::new(
            self.center_x,
            self.center_z,
            self.diameter(),
            self.new_diameter,
            self.speed.into(),
            self.portal_teleport_boundary.into(),
            self.warning_blocks.into(),
            self.warning_time.into(),
        )) {
            client.try_enqueue_packet(data);
        }
    }

    pub fn set_center(&mut self, world: &World, x: f64, z: f64) {
        self.center_x = x;
        self.center_z = z;

        world.broadcast_packet_all(&CSetBorderCenter::new(self.center_x, self.center_z));
    }

    pub fn set_diameter(&mut self, world: &World, diameter: f64, speed: Option<i64>) {
        self.start_size_change(diameter, speed.unwrap_or(0));

        match speed {
            Some(speed) => {
                world.broadcast_packet_all(&CSetBorderLerpSize::new(
                    self.old_diameter,
                    self.new_diameter,
                    speed.into(),
                ));
            }
            None => {
                world.broadcast_packet_all(&CSetBorderSize::new(self.new_diameter));
            }
        }
    }

    pub fn add_diameter(&mut self, world: &World, offset: f64, speed: Option<i64>) {
        self.set_diameter(world, self.diameter() + offset, speed);
    }

    pub fn set_warning_delay(&mut self, world: &World, delay: i32) {
        self.warning_time = delay;

        world.broadcast_packet_all(&CSetBorderWarningDelay::new(self.warning_time.into()));
    }

    pub fn set_warning_distance(&mut self, world: &World, distance: i32) {
        self.warning_blocks = distance;

        world.broadcast_packet_all(&CSetBorderWarningDistance::new(self.warning_blocks.into()));
    }

    pub const fn set_damage_buffer(&mut self, buffer: f32) {
        self.buffer = buffer as f64;
    }

    pub const fn set_damage_per_block(&mut self, damage: f32) {
        self.damage_per_block = damage as f64;
    }

    pub fn reset(&mut self, world: &World) {
        self.center_x = 0.0;
        self.center_z = 0.0;
        self.start_size_change(59_999_968.0, 0);
        self.speed = 0;
        self.portal_teleport_boundary = 29_999_984;
        self.warning_blocks = 5;
        self.warning_time = 300;
        self.damage_per_block = 0.2;
        self.buffer = 5.0;

        world.broadcast_packet_all(&CInitializeWorldBorder::new(
            self.center_x,
            self.center_z,
            self.diameter(),
            self.new_diameter,
            self.speed.into(),
            self.portal_teleport_boundary.into(),
            self.warning_blocks.into(),
            self.warning_time.into(),
        ));
    }

    // Public packet/API durations remain milliseconds; simulation advances in ticks.
    fn start_size_change(&mut self, diameter: f64, milliseconds: i64) {
        self.old_diameter = self.current_diameter;
        self.new_diameter = diameter;
        self.previous_diameter = self.current_diameter;
        self.lerp_duration = if self.old_diameter == diameter {
            0
        } else {
            milliseconds.max(0).saturating_add(49) / 50
        };
        self.lerp_remaining = self.lerp_duration;
        self.speed = self.lerp_remaining.saturating_mul(50);
        if self.lerp_remaining == 0 {
            self.current_diameter = diameter;
            self.previous_diameter = diameter;
        }
    }

    pub fn tick(&mut self) {
        if self.lerp_remaining > 0 {
            self.previous_diameter = self.current_diameter;
            self.lerp_remaining -= 1;
            let progress =
                (self.lerp_duration - self.lerp_remaining) as f64 / self.lerp_duration as f64;
            self.current_diameter =
                self.old_diameter + progress * (self.new_diameter - self.old_diameter);
            self.speed = self.lerp_remaining.saturating_mul(50);
            if self.lerp_remaining == 0 {
                self.previous_diameter = self.new_diameter;
            }
        }
    }

    pub fn diameter(&self) -> f64 {
        self.current_diameter
    }

    /// getMin/Max without a partial tick uses the previous tick's extent in Java.
    pub fn bounds(&self) -> [f64; 4] {
        let half = self.previous_diameter / 2.0;
        let limit = f64::from(self.portal_teleport_boundary);
        [
            (self.center_x - half).clamp(-limit, limit),
            (self.center_z - half).clamp(-limit, limit),
            (self.center_x + half).clamp(-limit, limit),
            (self.center_z + half).clamp(-limit, limit),
        ]
    }

    pub fn collision_boxes(&self, position: Vector3<f64>, area: BoundingBox) -> Vec<BoundingBox> {
        let [min_x, min_z, max_x, max_z] = self.bounds();
        let size = area.max - area.min;
        let margin = size.x.abs().max(size.z.abs()).max(1.0);
        let distance = (position.x - min_x)
            .min(max_x - position.x)
            .min(position.z - min_z)
            .min(max_z - position.z);
        if distance >= margin * 2.0
            || position.x < min_x - margin
            || position.x >= max_x + margin
            || position.z < min_z - margin
            || position.z >= max_z + margin
        {
            return Vec::new();
        }
        let n = f64::NEG_INFINITY;
        let p = f64::INFINITY;
        vec![
            BoundingBox::new_array([n, n, n], [min_x.floor(), p, p]),
            BoundingBox::new_array([max_x.ceil(), n, n], [p, p, p]),
            BoundingBox::new_array([n, n, n], [p, p, min_z.floor()]),
            BoundingBox::new_array([n, n, max_z.ceil()], [p, p, p]),
        ]
    }

    #[must_use]
    pub fn contains(&self, x: f64, z: f64) -> bool {
        let [min_x, min_z, max_x, max_z] = self.bounds();
        x >= min_x && x < max_x && z >= min_z && z < max_z
    }

    #[must_use]
    pub fn contains_block(&self, x: i32, z: i32) -> bool {
        self.contains(f64::from(x), f64::from(z))
    }

    #[must_use]
    pub fn contains_box(&self, bounds: &BoundingBox) -> bool {
        let epsilon = f64::from(1.0e-5_f32);
        self.contains(bounds.min.x, bounds.min.z)
            && self.contains(bounds.max.x - epsilon, bounds.max.z - epsilon)
    }

    #[must_use]
    pub fn clamp_block(&self, x: i32, z: i32) -> (i32, i32) {
        let [min_x, min_z, max_x, max_z] = self.bounds();
        let clamp = |value: i32, min: f64, max: f64| {
            f64::from(value)
                .max(min)
                .min(max - f64::from(1.0e-5_f32))
                .floor() as i32
        };
        (clamp(x, min_x, max_x), clamp(z, min_z, max_z))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn border_extent_matches_java_tick_and_retarget_trace() {
        #[derive(serde::Deserialize)]
        struct Case {
            op: u8,
            a: f64,
            b: f64,
            values: [u64; 5],
            remaining: i64,
        }
        let cases: Vec<Case> = serde_json::from_str(include_str!("border_cases.json")).unwrap();
        let mut border = Worldborder::new(0.0, 0.0, 59_999_968.0, 0, 5, 300);
        for (index, case) in cases.into_iter().enumerate() {
            match case.op {
                0 => border.start_size_change(case.a, 0),
                1 => border.start_size_change(case.a, case.b as i64 * 50),
                2 => border.tick(),
                3 => {
                    border.center_x = case.a;
                    border.center_z = case.b;
                }
                4 => {
                    for _ in 0..case.b as i32 {
                        border.tick();
                    }
                }
                5 => border.portal_teleport_boundary = case.b as i32,
                _ => unreachable!(),
            }
            let [a, b, c, d] = border.bounds();
            assert_eq!(
                [border.diameter(), a, b, c, d],
                case.values.map(f64::from_bits),
                "Java extent case {index}"
            );
            assert_eq!(
                border.lerp_remaining, case.remaining,
                "Java duration case {index}"
            );
        }
    }
    #[test]
    fn saved_lerp_resumes_at_current_size_and_damage_respects_buffer() {
        let mut border = Worldborder::new(0.0, 0.0, 100.0, 0, 5, 300);
        border.start_size_change(20.0, 1000);
        for _ in 0..7 {
            border.tick();
        }
        let mut restored = Worldborder::new(0.0, 0.0, 1.0, 0, 5, 300);
        restored.read_saved(&border.saved_data());
        assert_eq!(restored.diameter(), border.diameter());
        assert_eq!(restored.lerp_remaining, 13);
        for _ in 0..13 {
            restored.tick();
        }
        assert_eq!(restored.diameter(), 20.0);
        let bounds = |x| BoundingBox::new_array([x - 0.3, 0.0, -0.3], [x + 0.3, 1.8, 0.3]);
        assert_eq!(
            restored.damage_at(Vector3::new(15.0, 0.0, 0.0), bounds(15.0)),
            None
        );
        assert_eq!(
            restored.damage_at(Vector3::new(16.0, 0.0, 0.0), bounds(16.0)),
            Some(1.0)
        );
        assert_eq!(
            restored.damage_at(Vector3::new(30.0, 0.0, 0.0), bounds(30.0)),
            Some(3.0)
        );
    }
    #[test]
    fn collision_plane_rounding_and_outside_margin() {
        let border = Worldborder::new(0.25, 0.25, 10.5, 0, 5, 300);
        let area = BoundingBox::new_array([4.7, 0.0, -0.3], [6.0, 1.8, 0.3]);
        let shapes = border.collision_boxes(Vector3::new(5.0, 0.0, 0.0), area);
        assert_eq!(shapes.len(), 4);
        assert_eq!(shapes[1].min.x, 6.0);
        assert!(
            border
                .collision_boxes(Vector3::new(50.0, 0.0, 0.0), area)
                .is_empty()
        );
    }
}
