//! Java's shared selection/update rules for loot and enchanting tables.
use crate::{
    Enchantment,
    data_component_impl::{EnchantableImpl, EnchantmentsImpl, StoredEnchantmentsImpl},
    item::Item,
    item_stack::ItemStack,
};

pub trait EnchantmentRandom {
    fn enchantment_int(&mut self, bound: i32) -> i32;
    fn enchantment_float(&mut self) -> f32;
}
impl<R: pumpkin_util::random::RandomImpl> EnchantmentRandom for R {
    fn enchantment_int(&mut self, bound: i32) -> i32 {
        self.next_bounded_i32(bound)
    }
    fn enchantment_float(&mut self) -> f32 {
        self.next_f32()
    }
}

pub type EnchantmentList = Vec<(&'static Enchantment, i32)>;

/// Updates the appropriate existing component; absent components do not call the updater.
pub fn update(stack: &mut ItemStack, updater: impl FnOnce(&mut EnchantmentList)) {
    if stack.is_empty() {
        return;
    }
    let book = stack.item == &Item::ENCHANTED_BOOK;
    let old = if book {
        stack
            .get_data_component::<StoredEnchantmentsImpl>()
            .map(|v| v.enchantment.to_vec())
    } else {
        stack
            .get_data_component::<EnchantmentsImpl>()
            .map(|v| v.enchantment.to_vec())
    };
    let Some(mut values) = old else {
        return;
    };
    updater(&mut values);
    if book {
        stack.set_data_component(StoredEnchantmentsImpl {
            enchantment: values.into(),
        });
    } else {
        stack.set_data_component(EnchantmentsImpl {
            enchantment: values.into(),
        });
    }
}

pub fn set(values: &mut EnchantmentList, enchantment: &'static Enchantment, level: i32) {
    if level <= 0 {
        values.retain(|(value, _)| *value != enchantment);
    } else if let Some((_, previous)) = values.iter_mut().find(|(value, _)| *value == enchantment) {
        *previous = level.min(255);
    } else {
        values.push((enchantment, level.min(255)));
    }
}

pub fn upgrade(stack: &mut ItemStack, enchantment: &'static Enchantment, level: i32) {
    update(stack, |values| {
        if level > 0 {
            let old = values
                .iter()
                .find(|(value, _)| *value == enchantment)
                .map_or(0, |(_, level)| *level);
            set(values, enchantment, old.max(level.min(255)));
        }
    });
}

pub fn table_cost<R: EnchantmentRandom + ?Sized>(
    rng: &mut R,
    slot: usize,
    bookcases: i32,
    stack: &ItemStack,
) -> i32 {
    if stack.is_empty() || stack.get_data_component::<EnchantableImpl>().is_none() {
        return 0;
    }
    let bookcases = bookcases.min(15);
    let selected =
        rng.enchantment_int(8) + 1 + (bookcases >> 1) + rng.enchantment_int(bookcases + 1);
    match slot {
        0 => (selected / 3).max(1),
        1 => selected * 2 / 3 + 1,
        _ => selected.max(bookcases * 2),
    }
}

fn weighted<R: EnchantmentRandom + ?Sized>(
    rng: &mut R,
    candidates: &[(&'static Enchantment, i32)],
) -> Option<(&'static Enchantment, i32)> {
    let total: i32 = candidates.iter().map(|(value, _)| value.weight).sum();
    if total == 0 {
        return None;
    }
    let mut ticket = rng.enchantment_int(total);
    candidates.iter().copied().find(|(value, _)| {
        ticket -= value.weight;
        ticket < 0
    })
}

pub fn select<R: EnchantmentRandom + ?Sized>(
    rng: &mut R,
    stack: &ItemStack,
    cost: i32,
    source: impl IntoIterator<Item = &'static Enchantment>,
) -> EnchantmentList {
    if stack.is_empty() {
        return Vec::new();
    }
    let Some(enchantable) = stack.get_data_component::<EnchantableImpl>() else {
        return Vec::new();
    };
    let mut cost = cost
        .wrapping_add(1)
        .wrapping_add(rng.enchantment_int(enchantable.value / 4 + 1))
        .wrapping_add(rng.enchantment_int(enchantable.value / 4 + 1));
    let span = (rng.enchantment_float() + rng.enchantment_float() - 1.0) * 0.15;
    let value = cost as f32 + cost as f32 * span;
    cost = ((f64::from(value) + 0.5).floor() as i32).max(1);
    let mut candidates: EnchantmentList = source
        .into_iter()
        .filter(|enchantment| {
            stack.item == &Item::BOOK
                || (enchantment.can_enchant(stack.item)
                    && enchantment
                        .primary_items
                        .is_none_or(|tag| tag.1.contains(&stack.item.id)))
        })
        .filter_map(|enchantment| {
            (1..=enchantment.max_level)
                .rev()
                .find(|level| {
                    cost >= enchantment.min_cost.calculate(*level)
                        && cost <= enchantment.max_cost.calculate(*level)
                })
                .map(|level| (enchantment, level))
        })
        .collect();
    let mut result = Vec::new();
    if let Some(first) = weighted(rng, &candidates) {
        result.push(first);
        while rng.enchantment_int(50) <= cost {
            let last = result.last().unwrap().0;
            candidates.retain(|(enchantment, _)| last.are_compatible(enchantment));
            if candidates.is_empty() {
                break;
            }
            if let Some(selected) = weighted(rng, &candidates) {
                result.push(selected);
            }
            cost /= 2;
        }
    }
    result
}

pub fn table_enchantments<R: EnchantmentRandom + ?Sized>(
    rng: &mut R,
    stack: &ItemStack,
    cost: i32,
) -> EnchantmentList {
    let source = crate::tag::Enchantment::MINECRAFT_IN_ENCHANTING_TABLE
        .1
        .iter()
        .filter_map(|id| Enchantment::from_id(*id as u8));
    let mut result = select(rng, stack, cost, source);
    if stack.item == &Item::BOOK && !stack.is_empty() && result.len() > 1 {
        let index = rng.enchantment_int(result.len() as i32) as usize;
        result.remove(index);
    }
    result
}
