//! Lookups against the same ordered registry entries sent to 26.2 clients.
use crate::registry::{REGISTRY_V_26_2, StaticRegistryEntry};
use pumpkin_nbt::{deserializer::NbtReadHelperJava, tag::NbtTag};

pub fn entries(registry: &str) -> &'static [StaticRegistryEntry] {
    REGISTRY_V_26_2
        .iter()
        .find(|r| r.registry_id == registry.strip_prefix("minecraft:").unwrap_or(registry))
        .map_or(&[], |registry| registry.entries)
}
pub fn id(registry: &str, name: &str) -> Option<i32> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    entries(registry)
        .iter()
        .position(|entry| entry.name == name)
        .map(|id| id as i32)
}
pub fn name(registry: &str, id: i32) -> Option<String> {
    let entry = entries(registry).get(usize::try_from(id).ok()?)?;
    Some(format!("minecraft:{}", entry.name))
}
pub fn definition(registry: &str, name: &str) -> Option<NbtTag> {
    let entry = entries(registry).get(id(registry, name)? as usize)?;
    NbtTag::deserialize(&mut NbtReadHelperJava::new(&mut std::io::Cursor::new(
        entry.data,
    )))
    .ok()
}
