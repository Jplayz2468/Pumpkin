//! Reference standing dimensions. Pose-specific dimensions and relocation after
//! growth into an obstruction are handled separately from this size transition.
use crate::entity::{baby_dimensions_data::standing_bits, living::LivingEntity};
use pumpkin_data::attributes::Attributes;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};

pub fn refresh(living: &LivingEntity, baby: bool) {
    let entity = &living.entity;
    let Some([width, height, eye_height]) = standing_bits(entity.entity_type.resource_name, baby)
    else {
        return;
    };
    let scale = living.get_attribute_value(&Attributes::SCALE) as f32;
    let dimensions = EntityDimensions::new(
        f32::from_bits(width) * scale,
        f32::from_bits(height) * scale,
        f32::from_bits(eye_height) * scale,
    );
    let pos = entity.pos.load();
    entity.entity_dimension.store(dimensions);
    entity
        .bounding_box
        .store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
}

pub fn can_be_baby(name: &str) -> bool {
    !matches!(
        name,
        "camel_husk" | "frog" | "zombie_nautilus" | "parrot" | "magma_cube" | "slime"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn java_ageability_and_standing_dimensions() {
        let cases: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/dimensions-java-26.2.json"
        ))
        .unwrap();
        for (id, c) in cases.as_object().unwrap() {
            let name = id.strip_prefix("minecraft:").unwrap();
            let entity_type = pumpkin_data::entity::EntityType::from_name(name).unwrap();
            let speed = entity_type
                .attributes
                .iter()
                .find(|(a, _)| a.id == Attributes::MOVEMENT_SPEED.id)
                .unwrap()
                .1;
            assert_eq!(
                speed.to_bits(),
                c["speed_bits"].as_str().unwrap().parse::<u64>().unwrap(),
                "{id}: default speed lost precision"
            );
            if let Some(allowed) = c["can_be_baby"].as_bool() {
                assert_eq!(can_be_baby(name), allowed, "{id}");
            }
            for baby in [false, true] {
                let expected: [u32; 3] =
                    serde_json::from_value(c[if baby { "baby" } else { "adult" }]["bits"].clone())
                        .unwrap();
                assert_eq!(standing_bits(name, baby), Some(expected), "{id}");
            }
        }
    }
}
