use pumpkin_data::{
    data_component::DataComponent,
    data_component_impl::{AttributeModifierDisplay, AttributeModifiersImpl, DataComponentImpl},
    item::Item,
    item_stack::ItemStack,
};
use pumpkin_nbt::{deserializer::NbtReadHelperJava, tag::NbtTag};
use pumpkin_protocol::codec::data_component::{deserialize, serialize};
use pumpkin_util::serde_json::Value;
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
fn same_nbt(a: &NbtTag, b: &NbtTag) -> bool {
    match (a, b) {
        (NbtTag::Double(a), NbtTag::Double(b)) => a.to_bits() == b.to_bits(),
        (NbtTag::Float(a), NbtTag::Float(b)) => a.to_bits() == b.to_bits(),
        (NbtTag::List(a), NbtTag::List(b)) => {
            a.len() == b.len() && a.iter().zip(b).all(|(a, b)| same_nbt(a, b))
        }
        (NbtTag::Compound(a), NbtTag::Compound(b)) => {
            a.child_tags.len() == b.child_tags.len()
                && a.child_tags
                    .iter()
                    .all(|(k, v)| b.get(k).is_some_and(|b| same_nbt(v, b)))
        }
        _ => a == b,
    }
}
#[test]
fn attribute_components_match_java_codecs_hashes_prototypes_and_predicates() {
    let fixture: Value =
        pumpkin_util::serde_json::from_str(include_str!("attribute_component_cases.json")).unwrap();
    let predicates: Vec<_> = fixture["predicates"]
        .as_array()
        .unwrap()
        .iter()
        .map(nbt)
        .collect();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 1688);
    for (index, case) in fixture["cases"].as_array().unwrap().iter().enumerate() {
        let tag = nbt(&case["nbt"]);
        let component = AttributeModifiersImpl::read_data(&tag).unwrap();
        assert!(
            same_nbt(&component.write_data(), &tag),
            "saved attribute {index}"
        );
        assert_eq!(
            component.get_hash(),
            case["hash"].as_i64().unwrap() as i32,
            "attribute hash {index}"
        );
        let mut cursor = std::io::Cursor::new(bytes(&case["wire"]));
        let decoded = deserialize(DataComponent::AttributeModifiers, &mut cursor).unwrap();
        assert!(component.equal(decoded.as_ref()), "Java wire {index}");
        assert_eq!(cursor.position(), cursor.get_ref().len() as u64);
        let mut encoded = Vec::new();
        serialize(DataComponent::AttributeModifiers, &component, &mut encoded).unwrap();
        if !component
            .attribute_modifiers
            .iter()
            .any(|m| matches!(m.display, AttributeModifierDisplay::Override(_)))
        {
            assert_eq!(encoded, bytes(&case["wire"]), "wire bytes {index}");
        }
        let mut cursor = std::io::Cursor::new(encoded);
        let decoded = deserialize(DataComponent::AttributeModifiers, &mut cursor).unwrap();
        assert!(component.equal(decoded.as_ref()));
        assert_eq!(cursor.position(), cursor.get_ref().len() as u64);
        let prototype = case["prototype"].as_str().unwrap();
        if !prototype.is_empty() {
            let item = Item::from_registry_key(prototype).unwrap();
            let stack = ItemStack::new(1, item);
            assert_eq!(
                stack.get_data_component::<AttributeModifiersImpl>(),
                Some(&component),
                "prototype {prototype}"
            );
        }
        let mut stack = ItemStack::new(1, &Item::STICK);
        stack.set_data_component(component);
        for (pindex, p) in predicates.iter().enumerate() {
            assert_eq!(
                super::predicate::matches(p.extract_compound().unwrap(), &stack),
                case["matches"][pindex].as_bool().unwrap(),
                "attribute {index}, predicate {pindex}"
            );
        }
    }
    for (index, case) in fixture["codec"].as_array().unwrap().iter().enumerate() {
        assert_eq!(
            AttributeModifiersImpl::read_data(&nbt(&case["nbt"])).is_some(),
            case["valid"].as_bool().unwrap(),
            "codec {index}"
        );
    }
    assert_eq!(fixture["network"].as_array().unwrap().len(), 96);
    for case in fixture["network"].as_array().unwrap() {
        let mut cursor = std::io::Cursor::new(bytes(&case["input"]));
        let decoded = deserialize(DataComponent::AttributeModifiers, &mut cursor).unwrap();
        assert!(same_nbt(&decoded.write_data(), &nbt(&case["nbt"])));
        assert_eq!(cursor.position(), cursor.get_ref().len() as u64);
        let mut wire = Vec::new();
        serialize(
            DataComponent::AttributeModifiers,
            decoded.as_ref(),
            &mut wire,
        )
        .unwrap();
        assert_eq!(wire, bytes(&case["wire"]));
    }
}
#[test]
fn invalid_attribute_ids_lengths_and_identifier_bytes_reject() {
    use pumpkin_protocol::{codec::var_int::VarInt, ser::NetworkWriteExt};
    for id in [-1, i32::MAX] {
        let mut bytes = Vec::new();
        bytes.write_var_int(&VarInt(1)).unwrap();
        bytes.write_var_int(&VarInt(id)).unwrap();
        assert!(
            deserialize(
                DataComponent::AttributeModifiers,
                &mut std::io::Cursor::new(bytes)
            )
            .is_err()
        );
    }
    let mut bytes = Vec::new();
    bytes.write_var_int(&VarInt(-1)).unwrap();
    assert!(
        deserialize(
            DataComponent::AttributeModifiers,
            &mut std::io::Cursor::new(bytes)
        )
        .is_err()
    );
    for id in ["example:Bad", "bad space", "a:b:c", "🐝"] {
        let mut bytes = Vec::new();
        bytes.write_var_int(&VarInt(1)).unwrap();
        bytes.write_var_int(&VarInt(0)).unwrap();
        bytes.write_string(id).unwrap();
        assert!(
            deserialize(
                DataComponent::AttributeModifiers,
                &mut std::io::Cursor::new(bytes)
            )
            .is_err()
        );
    }
}
