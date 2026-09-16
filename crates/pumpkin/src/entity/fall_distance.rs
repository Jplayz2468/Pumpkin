//! Entity fall distance is a double; each downward movement is widened from float.
use pumpkin_nbt::compound::NbtCompound;

pub fn read(nbt: &NbtCompound) -> f64 {
    nbt.get_double("fall_distance")
        .or_else(|| nbt.get_float("fall_distance").map(f64::from))
        .or_else(|| nbt.get_double("FallDistance"))
        .or_else(|| nbt.get_float("FallDistance").map(f64::from))
        .unwrap_or(0.0)
}

pub fn accumulate(distance: f64, movement_y: f64, in_water: bool) -> f64 {
    if !in_water && movement_y < 0.0 {
        distance - f64::from(movement_y as f32)
    } else {
        distance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_accumulation_preserves_double_totals_and_float_steps() {
        let cases: Vec<(u64, u64, bool, u64)> =
            serde_json::from_str(include_str!("fall_distance_cases.json")).unwrap();
        for (index, (previous, movement, water, expected)) in cases.into_iter().enumerate() {
            assert_eq!(
                accumulate(f64::from_bits(previous), f64::from_bits(movement), water).to_bits(),
                expected,
                "case {index}"
            );
        }
    }

    #[test]
    fn modern_double_wins_and_legacy_float_still_loads() {
        let mut nbt = NbtCompound::new();
        nbt.put_float("FallDistance", 4.5);
        assert_eq!(read(&nbt), 4.5);
        let precise = 16_777_216.125;
        nbt.put_double("fall_distance", precise);
        assert_eq!(read(&nbt).to_bits(), precise.to_bits());
        assert_ne!(read(&nbt), f64::from(precise as f32));
    }
    #[test]
    fn movement_is_float_but_the_accumulator_does_not_round_back_to_float() {
        let distance = 16_777_216.0;
        assert_eq!(accumulate(distance, -0.125, false), distance + 0.125);
        assert_eq!(accumulate(0.0, -0.1, false), f64::from(0.1_f32));
        assert_eq!(accumulate(distance, -1.0, true), distance);
        assert_eq!(accumulate(distance, 1.0, false), distance);
    }
}
