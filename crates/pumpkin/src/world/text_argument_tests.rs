use pumpkin_data::component_hash;
use pumpkin_nbt::{deserializer::NbtReadHelperJava, tag::NbtTag};
use pumpkin_util::{text::TextComponent, version::JavaMinecraftVersion};
use serde_json::Value;
fn nbt(v: &Value) -> NbtTag {
    let bytes: Vec<u8> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect();
    NbtTag::deserialize(&mut NbtReadHelperJava::new(&mut std::io::Cursor::new(
        bytes,
    )))
    .unwrap()
}
fn check(component: TextComponent, expected: &Value) {
    assert_eq!(
        component
            .0
            .to_json_value_for_version(&JavaMinecraftVersion::V_26_2),
        expected["json"],
        "JSON: {expected}"
    );
    assert_eq!(
        component.to_nbt_tag_for_version(&JavaMinecraftVersion::V_26_2),
        nbt(&expected["nbt"]),
        "NBT: {expected}"
    );
    assert_eq!(
        component_hash::text(&component) as i32,
        expected["hash"].as_i64().unwrap() as i32,
        "hash: {expected}"
    );
    assert_eq!(
        component.get_text(),
        expected["text"].as_str().unwrap(),
        "text: {expected}"
    );
}
#[test]
fn translation_arguments_fallbacks_and_format_errors_match_java() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("text_argument_cases.json")).unwrap();
    assert_eq!(cases.len(), 69);
    for case in &cases {
        if let Some(input) = case.get("input") {
            check(
                serde_json::from_value(input.clone()).unwrap(),
                &case["direct"],
            );
            check(
                TextComponent::from_nbt(&nbt(&case["direct"]["nbt"])),
                &case["restored"],
            );
        } else {
            check(
                TextComponent::from_nbt(&nbt(&case["input_nbt"])),
                &case["restored"],
            );
        }
    }
}
