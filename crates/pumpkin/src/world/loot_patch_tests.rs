use super::*;
use pumpkin_data::{
    data_component::DataComponent,
    data_component_impl::{CustomNameImpl, DataComponentImpl},
};
use pumpkin_nbt::{compound::NbtCompound, deserializer::NbtReadHelperJava, tag::NbtTag};
use pumpkin_protocol::codec::data_component::{deserialize, serialize};
use pumpkin_util::{random::RandomImpl, text::TextComponent};
use serde_json::Value;
mod compiled {
    include!("loot_patch_test_tables.rs");
}
fn bytes(v: &Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn nbt(v: &Value) -> NbtTag {
    NbtTag::deserialize(&mut NbtReadHelperJava::new(&mut std::io::Cursor::new(
        bytes(v),
    )))
    .unwrap()
}
// ItemStackTemplate omits count=1 and empty components; compare the equivalent values.
fn normalize(v: &mut NbtTag) {
    match v {
        NbtTag::List(items) => {
            for item in items {
                normalize(item);
            }
        }
        NbtTag::Compound(c) => {
            if c.get_string("id").is_some() {
                if c.get("count").is_none() {
                    c.put_int("count", 1);
                }
                if c.get("components").is_none() {
                    c.put_compound("components", NbtCompound::new());
                }
            }
            for child in c.child_tags.values_mut() {
                normalize(child);
            }
        }
        _ => {}
    }
}
fn compare<R: RandomImpl>(case: &Value, mut rng: R) {
    let item = Item::from_registry_key(
        case["item"]
            .as_str()
            .unwrap()
            .strip_prefix("minecraft:")
            .unwrap(),
    )
    .unwrap();
    let mut stack = ItemStack::new(case["count"].as_u64().unwrap() as u8, item);
    stack.set_data_component(CustomNameImpl {
        name: TextComponent::text("old"),
    });
    let mut params = LootContextParameters::default();
    params
        .dynamic_drops
        .insert("minecraft:input".into(), vec![stack]);
    let mut output = Vec::new();
    run_table(
        &compiled::TABLES[case["table"].as_u64().unwrap() as usize],
        &params,
        LootFacts::from_params(&params),
        &mut rng,
        &mut Vec::new(),
        &mut |result, _| {
            let mut c = NbtCompound::new();
            if result.visible_count() > 0 {
                for (id, value) in &result.stack.patch {
                    if let Some(value) = value {
                        c.put(id.to_name(), value.write_data());
                    } else {
                        c.put_compound(&format!("!{}", id.to_name()), NbtCompound::new());
                    }
                }
            }
            let mut patch = NbtTag::Compound(c);
            normalize(&mut patch);
            output.push((result.visible_count(), patch));
        },
    );
    let expected: Vec<_> = case["output"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let mut patch = nbt(&v["patch"]);
            normalize(&mut patch);
            (v["count"].as_i64().unwrap() as i32, patch)
        })
        .collect();
    assert_eq!(output, expected, "{case}");
    assert_eq!(
        rng.next_i64(),
        case["next"].as_i64().unwrap(),
        "following RNG: {case}"
    );
}
#[test]
fn component_patches_match_java_validation_rollback_and_rng() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("loot_patch_cases.json")).unwrap();
    assert_eq!(cases.len(), 828);
    for case in cases {
        let seed = case["seed"].as_i64().unwrap() as u64;
        if case["kind"] == 0 {
            compare(&case, LegacyRand::from_seed(seed));
        } else {
            compare(&case, Xoroshiro::from_seed(seed));
        }
    }
}
#[test]
fn trim_components_match_java_network_persistence_and_hashes() {
    let cases: Vec<Value> = serde_json::from_str(include_str!("trim_wire_cases.json")).unwrap();
    assert_eq!(cases.len(), 201);
    for case in &cases {
        let raw = bytes(&case["bytes"]);
        let mut input = std::io::Cursor::new(raw.clone());
        let decoded = deserialize(DataComponent::Trim, &mut input).unwrap();
        assert_eq!(input.position(), raw.len() as u64);
        let expected = nbt(&case["nbt"]);
        assert_eq!(decoded.write_data(), expected, "{case}");
        assert_eq!(
            decoded.get_hash(),
            case["hash"].as_i64().unwrap() as i32,
            "hash: {case}"
        );
        let mut output = Vec::new();
        serialize(DataComponent::Trim, decoded.as_ref(), &mut output).unwrap();
        if case["reference"] == true {
            assert_eq!(output, raw, "{case}");
        }
        assert_eq!(
            deserialize(DataComponent::Trim, &mut std::io::Cursor::new(output))
                .unwrap()
                .write_data(),
            expected
        );
        let saved =
            pumpkin_data::data_component_impl::read_data(DataComponent::Trim, &expected).unwrap();
        assert_eq!(saved.write_data(), expected);
    }
    for raw in [
        vec![255, 255, 255, 255, 15],
        vec![128, 128, 128, 128, 8],
        vec![255, 127],
        vec![1, 255, 127],
    ] {
        assert!(deserialize(DataComponent::Trim, &mut std::io::Cursor::new(raw)).is_err());
    }
}
