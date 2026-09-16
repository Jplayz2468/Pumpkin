use crate::block::entities::{BlockEntity, jukebox::JukeboxBlockEntity};
use pumpkin_data::{
    data_component::DataComponent,
    data_component_impl::{DataComponentImpl, JukeboxPlayableImpl, JukeboxSongValue},
    item::Item,
    item_stack::ItemStack,
};
use pumpkin_nbt::{compound::NbtCompound, deserializer::NbtReadHelperJava, tag::NbtTag};
use pumpkin_protocol::codec::data_component::{deserialize, serialize};
use pumpkin_util::{math::position::BlockPos, serde_json::Value};
fn bytes(value: &Value) -> Vec<u8> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_u64().unwrap() as u8)
        .collect()
}
fn nbt(value: &Value) -> NbtTag {
    NbtTag::deserialize(&mut NbtReadHelperJava::new(&mut std::io::Cursor::new(
        bytes(value),
    )))
    .unwrap()
}
#[test]
fn jukebox_components_match_java_wire_codec_hash_predicates_and_playback() {
    let fixture: Value =
        pumpkin_util::serde_json::from_str(include_str!("jukebox_component_cases.json")).unwrap();
    let predicates: Vec<_> = fixture["predicates"]
        .as_array()
        .unwrap()
        .iter()
        .map(nbt)
        .collect();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 35);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let mut cursor = std::io::Cursor::new(bytes(&case["wire"]));
        let value = deserialize(DataComponent::JukeboxPlayable, &mut cursor).unwrap();
        assert_eq!(cursor.position(), cursor.get_ref().len() as u64);
        let component = value
            .as_any()
            .downcast_ref::<JukeboxPlayableImpl>()
            .unwrap();
        let mut output = Vec::new();
        serialize(DataComponent::JukeboxPlayable, component, &mut output).unwrap();
        assert_eq!(output, bytes(&case["wire"]), "song wire {index}");
        let roundtrip = deserialize(
            DataComponent::JukeboxPlayable,
            &mut std::io::Cursor::new(output),
        )
        .unwrap();
        assert!(
            component.equal(roundtrip.as_ref()),
            "holder equality {index}"
        );
        let playback = component.playback().unwrap();
        assert_eq!(
            playback.registry_id,
            case["registry_id"].as_i64().unwrap() as i32
        );
        assert_eq!(
            playback.length_in_ticks,
            case["length_ticks"].as_i64().unwrap() as i32
        );
        assert_eq!(
            playback.comparator_output,
            case["comparator"].as_i64().unwrap() as i32
        );
        if let JukeboxSongValue::Inline(song) = &component.song {
            assert_eq!(
                song.length_in_seconds.to_bits(),
                case["seconds_bits"].as_i64().unwrap() as u32
            );
        }
        for resume in case["resume"].as_array().unwrap() {
            assert_eq!(
                !playback.has_finished(resume["tick"].as_i64().unwrap()),
                resume["playing"].as_bool().unwrap(),
                "duration boundary {index}: {resume}"
            );
        }
        let mut stack = ItemStack::new(1, &Item::STICK);
        stack.set_data_component(component.clone());
        let mut saved_item = NbtCompound::new();
        saved_item.put_int("sentinel", 7);
        assert_eq!(
            stack.try_write_item_stack(&mut saved_item),
            case["item_persistent"].as_bool().unwrap()
        );
        if !case["item_persistent"].as_bool().unwrap() {
            assert_eq!(saved_item.child_tags.len(), 1);
        }
        for (component, key) in [
            (
                pumpkin_data::data_component_impl::BundleContentsImpl {
                    items: vec![stack.clone()],
                }
                .to_dyn(),
                "bundle_persistent",
            ),
            (
                pumpkin_data::data_component_impl::ContainerImpl {
                    items: vec![(2, stack.clone())],
                }
                .to_dyn(),
                "container_persistent",
            ),
        ] {
            let mut outer = ItemStack::new(1, &Item::STICK);
            outer.set_data_component_dyn(component);
            assert_eq!(
                outer.try_write_item_stack(&mut NbtCompound::new()),
                case[key].as_bool().unwrap()
            );
        }
        for (pindex, p) in predicates.iter().enumerate() {
            assert_eq!(
                super::predicate::matches(p.extract_compound().unwrap(), &stack),
                case["matches"][pindex].as_bool().unwrap(),
                "song {index}, predicate {pindex}"
            );
        }
        let entity = JukeboxBlockEntity::new(BlockPos::new(0, 0, 0));
        entity.set_record(stack.clone());
        assert!(
            entity.is_playing(),
            "insertion starts even a zero/negative duration until its first tick"
        );
        assert_eq!(
            JukeboxBlockEntity::song_from_stack(&entity.get_record()),
            Some(playback)
        );
        let mut saved_entity = NbtCompound::new();
        entity.write_nbt(&mut saved_entity);
        assert_eq!(
            saved_entity.get("RecordItem").is_some(),
            nbt(&case["saved_record"])
                .extract_compound()
                .unwrap()
                .get("RecordItem")
                .is_some()
        );
        entity.stop_playing();
        assert!(!entity.is_playing());
        if case["persistent"].as_bool().unwrap() {
            assert_eq!(component.write_data(), nbt(&case["nbt"]));
            assert_eq!(component.get_hash(), case["hash"].as_i64().unwrap() as i32);
            assert_eq!(
                JukeboxPlayableImpl::read_data(&nbt(&case["nbt"])).as_ref(),
                Some(component)
            );
            let mut item = NbtCompound::new();
            stack.write_item_stack(&mut item);
            let restored = ItemStack::read_item_stack(&item).unwrap();
            assert_eq!(
                restored.get_data_component::<JukeboxPlayableImpl>(),
                Some(component)
            );
            for resume in case["resume"].as_array().unwrap() {
                let mut saved = NbtCompound::new();
                saved.put_compound("RecordItem", item.clone());
                saved.put_long("ticks_since_song_started", resume["tick"].as_i64().unwrap());
                let loaded = JukeboxBlockEntity::from_nbt(&saved, BlockPos::new(0, 0, 0));
                assert_eq!(
                    loaded.is_playing(),
                    resume["playing"].as_bool().unwrap(),
                    "saved playback {index}: {resume}"
                );
            }
        } else {
            assert_eq!(
                component.write_data(),
                NbtTag::End,
                "direct holders have no persistent codec"
            );
        }
    }
    for case in fixture["codec"].as_array().unwrap() {
        assert_eq!(
            JukeboxPlayableImpl::read_data(&nbt(&case["nbt"])).is_some(),
            case["valid"].as_bool().unwrap()
        );
    }
}
#[test]
fn jukebox_rejects_invalid_song_and_sound_registry_ids() {
    use pumpkin_protocol::{codec::var_int::VarInt, ser::NetworkWriteExt};
    for id in [-1, i32::MAX, 23] {
        let mut wire = Vec::new();
        wire.write_var_int(&VarInt(id)).unwrap();
        assert!(
            deserialize(
                DataComponent::JukeboxPlayable,
                &mut std::io::Cursor::new(wire)
            )
            .is_err()
        );
    }
    for id in [-1, i32::MIN, i32::MAX] {
        let mut wire = Vec::new();
        wire.write_var_int(&VarInt(0)).unwrap();
        wire.write_var_int(&VarInt(id)).unwrap();
        assert!(
            deserialize(
                DataComponent::JukeboxPlayable,
                &mut std::io::Cursor::new(wire)
            )
            .is_err()
        );
    }
    let unknown = JukeboxPlayableImpl {
        song: JukeboxSongValue::Reference("example:cat".into()),
    };
    assert!(serialize(DataComponent::JukeboxPlayable, &unknown, &mut Vec::new()).is_err());
}
