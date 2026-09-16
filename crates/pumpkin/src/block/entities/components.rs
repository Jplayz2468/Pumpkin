//! Components retained by a placed block entity after its own fields consume theirs.
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{DataComponentImpl, read_data};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use std::sync::Mutex;

type Components = Vec<(DataComponent, Box<dyn DataComponentImpl>)>;

#[derive(Default)]
pub struct BlockEntityComponents(Mutex<Components>);

impl BlockEntityComponents {
    pub const fn new() -> Self {
        Self(Mutex::new(Vec::new()))
    }

    pub fn read_nbt(&self, nbt: &NbtCompound) {
        let values = nbt
            .get_compound("components")
            .into_iter()
            .flat_map(|data| data.child_tags.iter())
            .filter_map(|(name, value)| {
                let kind = DataComponent::try_from_name(name)?;
                (kind != DataComponent::AdditionalTradeCost)
                    .then(|| read_data(kind, value).map(|value| (kind, value)))
                    .flatten()
            })
            .collect();
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = values;
    }

    pub fn write_nbt(&self, nbt: &mut NbtCompound) {
        let mut components = NbtCompound::new();
        for (kind, value) in self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
        {
            if *kind == DataComponent::AdditionalTradeCost {
                continue;
            }
            let name = kind.to_name();
            let key = if name.contains(':') {
                name.to_owned()
            } else {
                format!("minecraft:{name}")
            };
            let Some(data) = value.try_write_data() else {
                return;
            };
            components.put(&key, data);
        }
        nbt.put_compound("components", components);
    }

    pub fn apply(&self, stack: &ItemStack, consumed: &[DataComponent]) {
        // Java retains only added patch entries, never prototype defaults or removals.
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
            .patch
            .iter()
            .filter(|(kind, _)| {
                !consumed.contains(kind)
                    && !matches!(
                        kind,
                        DataComponent::BlockEntityData | DataComponent::BlockState
                    )
            })
            .filter_map(|(kind, value)| value.as_ref().map(|value| (*kind, value.clone())))
            .collect();
    }

    pub fn collect(&self, stack: &mut ItemStack) {
        for (kind, value) in self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
        {
            if let Some((_, target)) = stack.patch.iter_mut().find(|(key, _)| key == kind) {
                *target = Some(value.clone());
            } else {
                stack.patch.push((*kind, Some(value.clone())));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::entities::{
        BlockEntity, banner::BannerBlockEntity, barrel::BarrelBlockEntity,
        beehive::BeehiveBlockEntity, chest::ChestBlockEntity,
        decorated_pot::DecoratedPotBlockEntity, enchanting_table::EnchantingTableBlockEntity,
        shulker_box::ShulkerBoxBlockEntity, skull::SkullBlockEntity,
        trapped_chest::TrappedChestBlockEntity,
    };
    use pumpkin_data::data_component_impl::{
        BannerPatternLayer, BannerPatternsImpl, ContainerImpl, CustomNameImpl, DamageImpl,
        ItemNameImpl, NoteBlockSoundImpl, ProfileImpl,
    };
    use pumpkin_data::item::Item;
    use pumpkin_util::{math::position::BlockPos, text::TextComponent};

    fn template() -> ItemStack {
        let mut stack = ItemStack::new(1, &Item::STONE);
        stack.set_data_component(CustomNameImpl {
            name: TextComponent::text("styled name").bold(),
        });
        stack.set_data_component(ItemNameImpl {
            name: "retained item name".into(),
        });
        stack.set_data_component(DamageImpl { damage: 37 });
        stack
    }

    fn round_trip<E: BlockEntity>(entity: E) {
        let source = template();
        entity.apply_components_from_item_stack(&source);
        let mut nbt = NbtCompound::new();
        entity.write_nbt(&mut nbt);
        let retained = nbt.get_compound("components").unwrap();
        assert!(retained.get("minecraft:item_name").is_some());
        assert!(
            retained.get("minecraft:max_stack_size").is_none(),
            "prototype defaults must not become stored patches"
        );
        let loaded = E::from_nbt(&nbt, BlockPos::new(1, 2, 3));
        let mut collected = ItemStack::new(1, &Item::AIR);
        loaded.collect_components(&mut collected);
        for (kind, value) in &source.patch {
            let actual = collected
                .patch
                .iter()
                .find(|(key, _)| key == kind)
                .and_then(|(_, v)| v.as_ref())
                .unwrap();
            assert!(
                actual.equal(value.as_ref().unwrap().as_ref()),
                "{} failed to round trip {}",
                entity.resource_location(),
                kind.to_name()
            );
        }
        // Reusing an existing entity with a plain item must clear retained values too.
        loaded.apply_components_from_item_stack(&ItemStack::new(1, &Item::STONE));
        let mut cleared = ItemStack::new(1, &Item::AIR);
        loaded.collect_components(&mut cleared);
        assert!(
            !cleared
                .patch
                .iter()
                .any(|(kind, _)| *kind == DataComponent::ItemName)
        );
        assert!(cleared.get_data_component::<CustomNameImpl>().is_none());
    }

    #[test]
    fn component_storage_survives_placement_and_nbt_without_copying_prototype_defaults() {
        let pos = BlockPos::new(1, 2, 3);
        round_trip(BannerBlockEntity::new(pos));
        round_trip(SkullBlockEntity::new(pos));
        round_trip(ChestBlockEntity::new(pos));
        round_trip(TrappedChestBlockEntity::new(pos));
        round_trip(BarrelBlockEntity::new(pos));
        round_trip(ShulkerBoxBlockEntity::new(pos));
        round_trip(DecoratedPotBlockEntity::new(pos));
        round_trip(BeehiveBlockEntity::new(pos));
        round_trip(EnchantingTableBlockEntity::new(pos));
    }

    #[test]
    fn banner_patterns_and_skull_profile_sound_survive_nbt() {
        let pos = BlockPos::new(1, 2, 3);
        let mut source = template();
        let patterns = BannerPatternsImpl {
            layers: vec![BannerPatternLayer {
                pattern: "minecraft:stripe_bottom".into(),
                color: pumpkin_data::dye_color::DyeColor::Red,
            }],
        };
        source.set_data_component(patterns.clone());
        let banner = BannerBlockEntity::new(pos);
        banner.apply_components_from_item_stack(&source);
        let mut nbt = NbtCompound::new();
        banner.write_nbt(&mut nbt);
        let mut result = ItemStack::new(1, &Item::AIR);
        BannerBlockEntity::from_nbt(&nbt, pos).collect_components(&mut result);
        assert_eq!(
            result.get_data_component::<BannerPatternsImpl>(),
            Some(&patterns)
        );
        assert!(
            nbt.get_compound("components")
                .unwrap()
                .get("minecraft:banner_patterns")
                .is_none()
        );

        let profile = ProfileImpl {
            name: Some("Alex".into()),
            ..Default::default()
        };
        let sound = NoteBlockSoundImpl {
            sound: "minecraft:block.note_block.bell".into(),
        };
        source.set_data_component(profile.clone());
        source.set_data_component(sound.clone());
        let skull = SkullBlockEntity::new(pos);
        skull.apply_components_from_item_stack(&source);
        let mut nbt = NbtCompound::new();
        skull.write_nbt(&mut nbt);
        let mut result = ItemStack::new(1, &Item::AIR);
        SkullBlockEntity::from_nbt(&nbt, pos).collect_components(&mut result);
        assert_eq!(result.get_data_component::<ProfileImpl>(), Some(&profile));
        assert_eq!(
            result.get_data_component::<NoteBlockSoundImpl>(),
            Some(&sound)
        );
    }

    #[test]
    fn stored_removals_and_consumed_values_do_not_reappear() {
        let storage = BlockEntityComponents::new();
        let mut source = template();
        source.remove_data_component(DataComponent::ItemName);
        source.set_data_component(pumpkin_data::data_component_impl::BlockStateImpl {
            properties: vec![("facing".into(), "north".into())].into(),
        });
        storage.apply(&source, &[DataComponent::CustomName]);
        let mut collected = ItemStack::new(1, &Item::AIR);
        storage.collect(&mut collected);
        assert_eq!(collected.patch.len(), 1);
        assert!(
            !collected
                .patch
                .iter()
                .any(|(kind, _)| *kind == DataComponent::ItemName)
        );
        assert!(collected.get_data_component::<CustomNameImpl>().is_none());
        assert_eq!(
            collected.get_data_component::<DamageImpl>().unwrap().damage,
            37
        );
    }

    #[test]
    fn built_in_tables_filter_collected_components_instead_of_copying_everything() {
        use crate::world::loot::{LootContextParameters, generate_loot_with_context};
        let pos = BlockPos::new(1, 2, 3);
        let mut source = template();
        source.set_data_component(ContainerImpl {
            items: vec![(2, ItemStack::new(3, &Item::DIAMOND))],
        });
        let chest = ChestBlockEntity::new(pos);
        chest.apply_components_from_item_stack(&source);
        let banner = BannerBlockEntity::new(pos);
        banner.apply_components_from_item_stack(&source);
        for (key, entity, copies_item_name) in [
            ("minecraft:blocks/chest", &chest as &dyn BlockEntity, false),
            (
                "minecraft:blocks/red_banner",
                &banner as &dyn BlockEntity,
                true,
            ),
        ] {
            let params = LootContextParameters::default().with_block_entity(Some(entity));
            let table = pumpkin_data::loot_table::get_loot_table(key).unwrap();
            let drops = generate_loot_with_context(table, 0, &params);
            assert_eq!(drops.len(), 1, "{key}");
            assert_eq!(
                drops[0].get_data_component::<CustomNameImpl>(),
                source.get_data_component::<CustomNameImpl>()
            );
            assert_eq!(
                drops[0]
                    .patch
                    .iter()
                    .any(|(kind, value)| *kind == DataComponent::ItemName && value.is_some()),
                copies_item_name
            );
            assert!(drops[0].get_data_component::<DamageImpl>().is_none());
            assert!(
                drops[0]
                    .get_data_component::<ContainerImpl>()
                    .is_none_or(|value| value.items.is_empty())
            );
        }
        // Empty is a present container value: collecting onto a used carrier clears it.
        let empty = ShulkerBoxBlockEntity::new(pos);
        empty.collect_components(&mut source);
        assert!(
            source
                .get_data_component::<ContainerImpl>()
                .unwrap()
                .items
                .is_empty()
        );
    }
    #[test]
    fn pot_does_not_convert_its_pending_table_into_an_implicit_loot_component() {
        use pumpkin_data::data_component_impl::ContainerLootImpl;
        let pos = BlockPos::new(0, 0, 0);
        let mut nbt = NbtCompound::new();
        nbt.put_string("LootTable", "minecraft:pots/trial_chambers/corridor".into());
        let pot = DecoratedPotBlockEntity::from_nbt(&nbt, pos);
        let mut collected = ItemStack::new(1, &Item::AIR);
        pot.collect_components(&mut collected);
        assert!(
            collected
                .get_data_component::<ContainerLootImpl>()
                .is_none()
        );
        let mut source = template();
        source.set_data_component(ContainerLootImpl {
            loot_table: "minecraft:empty".into(),
            seed: 7,
        });
        pot.apply_components_from_item_stack(&source);
        let mut saved = NbtCompound::new();
        pot.write_nbt(&mut saved);
        assert_eq!(saved.get_string("LootTable"), nbt.get_string("LootTable"));
        assert!(
            saved
                .get_compound("components")
                .unwrap()
                .get("minecraft:container_loot")
                .is_some()
        );
    }
}
