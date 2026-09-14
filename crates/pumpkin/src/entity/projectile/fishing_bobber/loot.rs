//! Java 26.2 fishing categories, quality weights and item functions.
use pumpkin_data::{Enchantment, item::Item, item_stack::ItemStack};
use pumpkin_util::random::RandomImpl;

pub fn category_weights(luck: f32, open_water: bool) -> [i32; 3] {
    [
        (10.0 - 2.0 * luck).floor().max(0.0) as i32,
        if open_water {
            (5.0 + 2.0 * luck).floor().max(0.0) as i32
        } else {
            0
        },
        (85.0 - luck).floor().max(0.0) as i32,
    ]
}
fn choose(rng: &mut impl RandomImpl, weights: &[i32]) -> usize {
    let mut ticket = rng.next_bounded_i32(weights.iter().sum());
    weights
        .iter()
        .position(|weight| {
            ticket -= weight;
            ticket < 0
        })
        .unwrap_or(0)
}

pub fn catch(rng: &mut impl RandomImpl, luck: f32, open_water: bool, jungle: bool) -> ItemStack {
    let category = choose(rng, &category_weights(luck, open_water));
    let (item, damage, enchanted, count) = match category {
        0 => {
            let mut weights = [17, 10, 10, 10, 10, 5, 2, 10, 5, 1, 10, 10, 0];
            if jungle {
                weights[12] = 10;
            }
            let i = choose(rng, &weights);
            let items = [
                &Item::LILY_PAD,
                &Item::LEATHER_BOOTS,
                &Item::LEATHER,
                &Item::BONE,
                &Item::POTION,
                &Item::STRING,
                &Item::FISHING_ROD,
                &Item::BOWL,
                &Item::STICK,
                &Item::INK_SAC,
                &Item::TRIPWIRE_HOOK,
                &Item::ROTTEN_FLESH,
                &Item::BAMBOO,
            ];
            (
                items[i],
                if i == 1 || i == 6 { Some(0.9) } else { None },
                false,
                if i == 9 { 10 } else { 1 },
            )
        }
        1 => {
            let i = rng.next_bounded_i32(6) as usize;
            (
                [
                    &Item::NAME_TAG,
                    &Item::SADDLE,
                    &Item::BOW,
                    &Item::FISHING_ROD,
                    &Item::BOOK,
                    &Item::NAUTILUS_SHELL,
                ][i],
                if i == 2 || i == 3 { Some(0.25) } else { None },
                (2..=4).contains(&i),
                1,
            )
        }
        _ => (
            [
                &Item::COD,
                &Item::SALMON,
                &Item::TROPICAL_FISH,
                &Item::PUFFERFISH,
            ][choose(rng, &[60, 25, 2, 13])],
            None,
            false,
            1,
        ),
    };
    let mut stack = ItemStack::new(count, item);
    if let Some(max_remaining) = damage {
        let remaining = rng.next_f32() * max_remaining;
        if let Some(max) = stack.get_max_damage() {
            stack.set_damage(((1.0 - remaining) * max as f32).floor() as i32);
        }
    }
    if item == &Item::POTION {
        stack.set_data_component(pumpkin_data::data_component_impl::PotionContentsImpl {
            potion_id: Some(pumpkin_data::potion::Potion::WATER.id as i32),
            custom_color: None,
            custom_effects: vec![],
            custom_name: None,
        });
    }
    if enchanted {
        enchant_loot(rng, &mut stack, 30);
    }
    stack
}

fn enchant_loot(rng: &mut impl RandomImpl, stack: &mut ItemStack, base_level: i32) {
    let item = stack.item;
    let value = stack
        .get_data_component::<pumpkin_data::data_component_impl::EnchantableImpl>()
        .map_or(0, |e| e.value);
    if value <= 0 {
        return;
    }
    let mut level =
        base_level + 1 + rng.next_bounded_i32(value / 4 + 1) + rng.next_bounded_i32(value / 4 + 1);
    let bonus = (rng.next_f32() + rng.next_f32() - 1.0) * 0.15;
    level = (level as f32 + level as f32 * bonus).round().max(1.0) as i32;
    let mut options: Vec<_> = pumpkin_data::tag::Enchantment::MINECRAFT_ON_RANDOM_LOOT
        .1
        .iter()
        .filter_map(|id| Enchantment::from_id(*id as u8))
        .filter(|e| item == &Item::BOOK || e.can_enchant(item))
        .filter_map(|e| {
            (1..=e.max_level)
                .rev()
                .find(|n| (e.min_cost.calculate(*n)..=e.max_cost.calculate(*n)).contains(&level))
                .map(|n| (e, n))
        })
        .collect();
    if item == &Item::BOOK {
        *stack = ItemStack::new(1, &Item::ENCHANTED_BOOK);
    }
    let mut selected = Vec::new();
    while !options.is_empty() {
        let index = choose(
            rng,
            &options.iter().map(|(e, _)| e.weight).collect::<Vec<_>>(),
        );
        let (enchantment, n) = options[index];
        selected.push((enchantment, n));
        if rng.next_bounded_i32(50) > level {
            break;
        }
        options.retain(|(candidate, _)| candidate.are_compatible(enchantment));
        level /= 2;
    }
    pumpkin_inventory::anvil::anvil_screen_handler::set_enchantments(stack, selected);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_util::random::legacy_rand::LegacyRand;
    #[test]
    fn quality_weights_match_java_floor_and_open_water_gate() {
        assert_eq!(category_weights(0.0, true), [10, 5, 85]);
        assert_eq!(category_weights(3.0, true), [4, 11, 82]);
        assert_eq!(category_weights(0.5, true), [9, 6, 84]);
        assert_eq!(category_weights(10.0, false), [0, 0, 75]);
    }
    #[test]
    fn catches_include_all_categories_and_preserve_item_functions() {
        let mut rng = LegacyRand::from_seed(42);
        let mut names = std::collections::HashSet::new();
        for _ in 0..10000 {
            let stack = catch(&mut rng, 3.0, true, true);
            assert!(!stack.is_empty());
            names.insert(stack.item.registry_key);
            if stack.item == &Item::INK_SAC {
                assert_eq!(stack.item_count, 10);
            }
            if stack.item == &Item::ENCHANTED_BOOK {
                assert!(
                    !pumpkin_inventory::anvil::anvil_screen_handler::get_enchantments_for_crafting(
                        &stack
                    )
                    .is_empty()
                );
            }
        }
        for name in ["cod", "salmon", "bamboo", "name_tag", "enchanted_book"] {
            assert!(names.contains(name), "{name}");
        }
    }
}
