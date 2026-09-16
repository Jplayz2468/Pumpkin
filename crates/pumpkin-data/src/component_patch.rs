//! Application of codec-decoded component patches, with Java's strict rollback.
use crate::{
    data_component::DataComponent,
    data_component_impl::{
        BeesImpl, BundleContentsImpl, ChargedProjectilesImpl, ContainerImpl, DataComponentImpl,
        MaxDamageImpl, read_data,
    },
    item::Item,
    item_stack::ItemStack,
};
use pumpkin_nbt::{deserializer::NbtReadHelperJava, tag::NbtTag};
pub type ComponentPatch = Vec<(DataComponent, Option<Box<dyn DataComponentImpl>>)>;

/// Input is canonical NBT produced by the Java component codecs during data export.
pub fn decode(bytes: &[u8]) -> Option<ComponentPatch> {
    let value = NbtTag::deserialize(&mut NbtReadHelperJava::new(&mut std::io::Cursor::new(
        bytes,
    )))
    .ok()?;
    value
        .extract_compound()?
        .child_tags
        .iter()
        .map(|(name, value)| {
            if let Some(name) = name.strip_prefix('!') {
                Some((DataComponent::try_from_name(name)?, None))
            } else {
                let id = DataComponent::try_from_name(name)?;
                Some((id, Some(read_data(id, value)?)))
            }
        })
        .collect()
}

impl ItemStack {
    /// Setting the prototype value removes the override, matching PatchedDataComponentMap.
    pub fn set_data_component_dyn(&mut self, value: Box<dyn DataComponentImpl>) {
        let id = value.get_self_enum();
        if self
            .item
            .components
            .iter()
            .any(|(kind, default)| *kind == id && value.equal(*default))
        {
            self.patch.retain(|(kind, _)| *kind != id);
        } else if let Some((_, target)) = self.patch.iter_mut().find(|(kind, _)| *kind == id) {
            *target = Some(value);
        } else {
            self.patch.push((id, Some(value)));
        }
    }

    /// Count is separate so raw loot's Java i32 count is validated before stack splitting.
    pub fn apply_components_and_validate(&mut self, patch: &ComponentPatch, count: i32) -> bool {
        let old = self.patch.clone();
        for (id, value) in patch {
            if let Some(value) = value {
                self.set_data_component_dyn(value.clone());
            } else {
                self.remove_data_component(*id);
            }
        }
        if !valid_stack(self, count) {
            self.patch = old;
            return false;
        }
        true
    }
}
fn valid_stack(stack: &ItemStack, count: i32) -> bool {
    // Java's getComponents() is empty while the stack itself is empty.
    if count <= 0 || stack.item == &Item::AIR {
        return true;
    }
    if count > i32::from(stack.get_max_stack_size()) {
        return false;
    }
    if stack.get_data_component::<MaxDamageImpl>().is_some() && stack.get_max_stack_size() > 1 {
        return false;
    }
    if let Some(container) = stack.get_data_component::<ContainerImpl>()
        && container
            .items
            .iter()
            .any(|(_, item)| !item.is_empty() && item.item_count > item.get_max_stack_size())
    {
        return false;
    }
    if let Some(bundle) = stack.get_data_component::<BundleContentsImpl>() {
        if bundle
            .items
            .iter()
            .any(|item| item.item_count > item.get_max_stack_size())
            || bundle_weight(bundle).is_none()
        {
            return false;
        }
    }
    if let Some(charged) = stack.get_data_component::<ChargedProjectilesImpl>()
        && charged.projectiles.iter().any(|nbt| {
            ItemStack::read_item_stack(nbt)
                .is_none_or(|item| item.item_count > item.get_max_stack_size())
        })
    {
        return false;
    }
    true
}

// Apache Fraction bounds its reduced numerator and denominator to signed i32.
// There is no capacity-one check in validateStrict; only arithmetic overflow fails.
fn fraction(n: u128, d: u128) -> Option<(u128, u128)> {
    if d == 0 {
        return None;
    }
    let (mut a, mut b) = (n, d);
    while b != 0 {
        (a, b) = (b, a % b);
    }
    let (n, d) = (n / a, d / a);
    (n <= i32::MAX as u128 && d <= i32::MAX as u128).then_some((n, d))
}
fn add(a: (u128, u128), b: (u128, u128)) -> Option<(u128, u128)> {
    fraction(a.0 * b.1 + b.0 * a.1, a.1 * b.1)
}
fn bundle_weight(bundle: &BundleContentsImpl) -> Option<(u128, u128)> {
    let mut sum = (0, 1);
    for item in &bundle.items {
        let weight = if let Some(nested) = item.get_data_component::<BundleContentsImpl>() {
            add(bundle_weight(nested)?, (1, 16))?
        } else if item
            .get_data_component::<BeesImpl>()
            .is_some_and(|bees| !bees.bees.is_empty())
        {
            (1, 1)
        } else {
            fraction(1, u128::from(item.get_max_stack_size()))?
        };
        sum = add(
            sum,
            fraction(weight.0 * u128::from(item.item_count), weight.1)?,
        )?;
    }
    Some(sum)
}
