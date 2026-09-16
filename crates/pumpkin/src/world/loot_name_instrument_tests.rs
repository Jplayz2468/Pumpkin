use super::*;
use pumpkin_data::{
    data_component::DataComponent,
    data_component_impl::{CustomNameImpl, DataComponentImpl, InstrumentImpl, ItemNameImpl},
};
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::codec::data_component::{deserialize, serialize};
use pumpkin_util::{random::RandomImpl, text::TextComponent, version::JavaMinecraftVersion};
use serde_json::{Value, json};
mod compiled {
    include!("loot_name_instrument_test_tables.rs");
}

fn text(value: TextComponent) -> Value {
    value
        .0
        .to_json_value_for_version(&JavaMinecraftVersion::V_26_2)
}
fn nbt_json(value: &NbtTag) -> Value {
    match value {
        NbtTag::End => Value::Null,
        NbtTag::Byte(v) => json!(v),
        NbtTag::Short(v) => json!(v),
        NbtTag::Int(v) => json!(v),
        NbtTag::Long(v) => json!(v),
        NbtTag::Float(v) => json!(v),
        NbtTag::Double(v) => json!(v),
        NbtTag::String(v) => json!(v),
        NbtTag::List(v) => Value::Array(v.iter().map(nbt_json).collect()),
        NbtTag::Compound(v) => Value::Object(
            v.child_tags
                .iter()
                .map(|(k, v)| (k.to_string(), nbt_json(v)))
                .collect(),
        ),
        NbtTag::ByteArray(v) => json!(v),
        NbtTag::IntArray(v) => json!(v),
        NbtTag::LongArray(v) => json!(v),
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
    if case["removed"].as_bool().unwrap() {
        stack.remove_data_component(DataComponent::ItemName);
        stack.remove_data_component(DataComponent::Instrument);
    }
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
            let s = &result.stack;
            output.push(json!({"count":result.visible_count(),"custom":s.get_data_component::<CustomNameImpl>().map(|v| text(v.name.clone())),"name":s.get_data_component::<ItemNameImpl>().map(|v| text(v.name.component())),"instrument":s.get_data_component::<InstrumentImpl>().map(|v| nbt_json(&v.write_data()))}));
        },
    );
    let expected: Vec<_> = case["output"].as_array().unwrap().iter().map(|v|json!({"count":v["count"],"custom":v["custom"],"name":v["name"],"instrument":v["instrument"]})).collect();
    assert_eq!(output, expected, "{case}");
    assert_eq!(
        rng.next_i64(),
        case["next"].as_i64().unwrap(),
        "following RNG: {case}"
    );
}
#[test]
fn names_and_instrument_loot_match_java_and_following_rng() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("loot_name_instrument_cases.json")).unwrap();
    assert_eq!(cases.len(), 736);
    assert_eq!(compiled::TABLES.len(), 23);
    for case in &cases {
        let seed = case["seed"].as_i64().unwrap() as u64;
        if case["kind"] == 0 {
            compare(case, LegacyRand::from_seed(seed));
        } else {
            compare(case, Xoroshiro::from_seed(seed));
        }
    }
}
fn component_json(id: DataComponent, value: &dyn DataComponentImpl) -> Value {
    use pumpkin_data::data_component_impl::get;
    match id {
        DataComponent::ItemName => text(get::<ItemNameImpl>(value).name.component()),
        DataComponent::CustomName => text(get::<CustomNameImpl>(value).name.clone()),
        DataComponent::Instrument => nbt_json(&get::<InstrumentImpl>(value).write_data()),
        _ => unreachable!(),
    }
}
#[test]
fn names_and_instruments_decode_java_network_bytes_and_reencode_without_loss() {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("name_instrument_wire_cases.json")).unwrap();
    assert_eq!(cases.len(), 20);
    for case in &cases {
        let id = DataComponent::try_from_name(case["component"].as_str().unwrap()).unwrap();
        let bytes: Vec<u8> = case["bytes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        let mut input = std::io::Cursor::new(bytes.clone());
        let decoded = deserialize(id, &mut input).unwrap();
        assert_eq!(input.position(), bytes.len() as u64, "{case}");
        assert_eq!(
            component_json(id, decoded.as_ref()),
            case["value"],
            "{case}"
        );
        if id == DataComponent::Instrument {
            use pumpkin_data::data_component_impl::{IdOr, get};
            let playback = get::<InstrumentImpl>(decoded.as_ref()).playback().unwrap();
            let sound = match playback.sound {
                IdOr::Id(sound) => json!(format!(
                    "minecraft:{}",
                    pumpkin_data::sound::Sound::NAMES[sound as usize]
                )),
                IdOr::Value(sound) => json!({"sound_id": sound.sound_name, "range": sound.range}),
            };
            assert_eq!(
                json!({"sound":sound,"range":playback.range,"duration":playback.duration_ticks}),
                case["playback"],
                "playback: {case}"
            );
        }
        assert_eq!(
            decoded.get_hash(),
            case["hash"].as_i64().unwrap() as i32,
            "component hash: {case}"
        );
        let mut encoded = Vec::new();
        serialize(id, decoded.as_ref(), &mut encoded).unwrap();
        if id == DataComponent::Instrument {
            assert_eq!(encoded, bytes, "instrument bytes: {case}");
        }
        let mut second = std::io::Cursor::new(encoded);
        let decoded_again = deserialize(id, &mut second).unwrap();
        assert_eq!(second.position(), second.get_ref().len() as u64);
        assert_eq!(
            component_json(id, decoded_again.as_ref()),
            case["value"],
            "{case}"
        );
        let persisted = decoded.write_data();
        let restored = pumpkin_data::data_component_impl::read_data(id, &persisted).unwrap();
        assert_eq!(
            component_json(id, restored.as_ref()),
            case["value"],
            "saved component: {case}"
        );
    }
}
#[test]
fn canonical_instruments_resolve_their_own_definitions_and_reject_invalid_ids() {
    use pumpkin_data::data_component_impl::InstrumentValue;
    for entry in pumpkin_data::registry_reference::entries("instrument") {
        let instrument = InstrumentImpl {
            instrument: InstrumentValue::Reference(format!("minecraft:{}", entry.name).into()),
        };
        let short = InstrumentImpl::read_data(&NbtTag::String(entry.name.into())).unwrap();
        assert!(short == instrument, "instrument reference must be canonical");
        let definition = instrument.definition().expect("registry definition");
        let value = definition.extract_compound().unwrap();
        assert!(value.get("sound_event").is_some());
        assert!(value.get("range").is_some());
        assert!(value.get("use_duration").is_some());
    }
    for bytes in [
        vec![255, 255, 255, 255, 15],
        vec![128, 128, 128, 128, 8],
        vec![255, 127],
    ] {
        assert!(deserialize(DataComponent::Instrument, &mut std::io::Cursor::new(bytes)).is_err());
    }
}
