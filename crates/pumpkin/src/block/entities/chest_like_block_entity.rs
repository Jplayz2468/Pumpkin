#[macro_export]
macro_rules! impl_block_entity_for_chest {
    ($struct_name:ty) => {
        impl $crate::block::entities::BlockEntity for $struct_name {
            fn resource_location(&self) -> &'static str {
                Self::ID
            }

            fn get_position(&self) -> BlockPos {
                self.position
            }

            fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self {
                let mut entity = Self::new(position);
                let loot = nbt
                    .get_string("LootTable")
                    .map(|key| (key.to_owned(), nbt.get_long("LootTableSeed").unwrap_or(0)));
                if loot.is_none() {
                    pumpkin_inventory::sync_read_items_from_nbt(
                        nbt,
                        entity
                            .items
                            .get_mut()
                            .unwrap_or_else(std::sync::PoisonError::into_inner),
                    );
                }
                *entity
                    .loot
                    .get_mut()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = loot;
                *entity
                    .custom_name
                    .get_mut()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) =
                    nbt.get("CustomName").map(TextComponent::from_nbt);
                entity
            }

            fn write_nbt(&self, nbt: &mut NbtCompound) {
                if let Some(name) = self
                    .custom_name
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
                {
                    nbt.put("CustomName", CustomNameImpl { name }.write_data());
                }
                if let Some((table, seed)) = self
                    .loot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
                {
                    nbt.put_string("LootTable", table);
                    if seed != 0 {
                        nbt.put_long("LootTableSeed", seed);
                    }
                } else {
                    let items = self
                        .items
                        .read()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if items.iter().any(|stack| !stack.is_empty()) {
                        sync_write_items_to_nbt(items.as_slice(), nbt);
                    }
                }
            }

            fn set_world(&self, world: Weak<World>) {
                *self
                    .world
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = world;
            }

            fn set_removed(&self) {
                self.removed.store(true, Ordering::Relaxed);
            }

            fn refresh_viewers(&self, world: &Arc<World>, source: Option<i32>) {
                if self.removed.load(Ordering::Relaxed) {
                    return;
                }
                self.viewers
                    .update_viewer_count_with_source(self, world, &self.position, source);
            }

            fn tick(&self, world: &Arc<World>) {
                if self.removed.load(Ordering::Relaxed) {
                    return;
                }
                self.viewers
                    .update_viewer_count::<Self>(self, world, &self.position);
            }

            fn get_inventory(self: Arc<Self>) -> Option<Arc<dyn Inventory>> {
                Some(self)
            }

            fn is_comparator_dirty(&self) -> bool {
                self.comparator_dirty.load(Ordering::Relaxed)
            }

            fn clear_comparator_dirty(&self) {
                self.comparator_dirty.store(false, Ordering::Relaxed);
            }

            fn is_dirty(&self) -> bool {
                self.dirty.load(Ordering::Relaxed)
            }

            fn clear_dirty(&self) {
                self.dirty.store(false, Ordering::Relaxed);
            }

            fn chunk_data_nbt(&self) -> Option<NbtCompound> {
                // Base BlockEntity.getUpdateTag is empty; inventories arrive through menus.
                Some(NbtCompound::new())
            }

            fn as_any(&self) -> &dyn Any {
                self
            }

            fn apply_components_from_item_stack(&self, stack: &ItemStack) {
                if let Some(loot) = stack.get_data_component::<ContainerLootImpl>() {
                    *self
                        .loot
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) =
                        Some((loot.loot_table.clone(), loot.seed));
                }
                *self
                    .custom_name
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = stack
                    .get_data_component::<CustomNameImpl>()
                    .map(|name| name.name.clone());
                let container = stack.get_data_component::<ContainerImpl>();
                {
                    let mut items = self
                        .items
                        .write()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    items.fill_with(|| ItemStack::EMPTY.clone());
                    for (slot, stored) in container
                        .into_iter()
                        .flat_map(|container| container.items.iter())
                    {
                        if let Some(target) = items.get_mut(*slot as usize) {
                            *target = stored.clone();
                        }
                    }
                }
                self.mark_dirty();
            }

            fn write_dropped_stack_components(&self, stack: &mut ItemStack) {
                // Built-in chest/barrel loot copies only custom_name; contents scatter separately.
                if let Some(name) = self
                    .custom_name
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
                {
                    stack.set_data_component(CustomNameImpl { name });
                }
            }

            fn take_loot_table(&self) -> Option<(String, i64)> {
                self.loot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take()
            }

            fn has_loot_table(&self) -> bool {
                <$struct_name>::has_loot_table(self)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_inventory_for_chest {
    ($struct_name:ty) => {
        impl pumpkin_inventory::Inventory for $struct_name {
            fn size(&self) -> usize {
                Self::INVENTORY_SIZE
            }

            fn is_empty(&self) -> bool {
                self.unpack_loot(None);
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items.iter().all(ItemStack::is_empty)
            }

            fn get_stack(&self, slot: usize) -> ItemStack {
                self.unpack_loot(None);
                let items = self
                    .items
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[slot].clone()
            }

            fn remove_stack(&self, slot: usize) -> ItemStack {
                self.unpack_loot(None);
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let removed = std::mem::replace(&mut items[slot], ItemStack::EMPTY.clone());
                removed
            }

            fn remove_stack_specific(&self, slot: usize, amount: u8) -> ItemStack {
                self.unpack_loot(None);
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let res = if !items[slot].is_empty() && amount > 0 {
                    items[slot].split(amount)
                } else {
                    ItemStack::EMPTY.clone()
                };
                if !res.is_empty() {
                    self.mark_dirty();
                }
                res
            }

            fn set_stack(&self, slot: usize, mut stack: ItemStack) {
                self.unpack_loot(None);
                stack.item_count = stack.item_count.min(
                    stack
                        .get_max_stack_size()
                        .min(self.get_max_count_per_stack()),
                );
                let mut items = self
                    .items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                items[slot] = stack;
                self.mark_dirty();
            }

            fn viewer_position(&self) -> Option<BlockPos> {
                Some(self.position)
            }

            fn on_open(&self) {
                if self.removed.load(Ordering::Relaxed) {
                    return;
                }
                self.viewers.open_container();
            }

            fn on_close(&self) {
                if self.removed.load(Ordering::Relaxed) {
                    return;
                }
                self.viewers.close_container();
            }

            fn mark_dirty(&self) {
                self.dirty.store(true, Ordering::Relaxed);
                self.comparator_dirty.store(true, Ordering::Relaxed);
            }

            fn as_any(&self) -> &dyn Any {
                self
            }
        }
    };
}

#[macro_export]
macro_rules! impl_clearable_for_chest {
    ($struct_name:ty) => {
        impl pumpkin_inventory::Clearable for $struct_name {
            fn clear(&self) {
                self.items
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .fill_with(|| ItemStack::EMPTY.clone());
            }
        }
    };
}

#[macro_export]
macro_rules! impl_viewer_count_listener_for_chest {
    ($struct_name:ty) => {
        impl $crate::block::viewer::ViewerCountListener for $struct_name {
            fn rechecks_viewers(&self) -> bool {
                true
            }

            fn on_container_open(
                &self,
                world: &Arc<$crate::world::World>,
                _position: &pumpkin_util::math::position::BlockPos,
            ) {
                self.play_sound(world, pumpkin_data::sound::Sound::BlockChestOpen);
            }

            fn on_container_close(
                &self,
                world: &Arc<$crate::world::World>,
                _position: &pumpkin_util::math::position::BlockPos,
            ) {
                self.play_sound(world, pumpkin_data::sound::Sound::BlockChestClose);
            }

            fn on_viewer_count_update(
                &self,
                world: &Arc<$crate::world::World>,
                position: &pumpkin_util::math::position::BlockPos,
                old: u16,
                new: u16,
            ) {
                // Trigger block animation
                world.add_synced_block_event(*position, Self::LID_ANIMATION_EVENT_TYPE, new as u8);

                // Update neighbors for redstone signal when viewer count changes
                // This is controlled by the EMITS_REDSTONE constant on the struct
                if Self::EMITS_REDSTONE && old != new {
                    // Update direct neighbors
                    world.update_neighbors(position, None);

                    // Also update neighbors of the block below (strongly powered block)
                    // This ensures redstone components adjacent to the block below are notified
                    let below_pos = position.down();
                    world.update_neighbors(&below_pos, None);
                }
            }
        }
    };
}

#[macro_export]
macro_rules! impl_chest_helper_methods {
    ($struct_name:ty) => {
        impl $struct_name {
            pub fn new(position: BlockPos) -> Self {
                Self {
                    position,
                    items: RwLock::new(from_fn(|_| ItemStack::EMPTY.clone())),
                    dirty: AtomicBool::new(false),
                    comparator_dirty: AtomicBool::new(false),
                    viewers: ViewerCountTracker::new(),
                    world: Mutex::new(Weak::new()),
                    loot: Mutex::new(None),
                    custom_name: Mutex::new(None),
                    removed: AtomicBool::new(false),
                }
            }

            pub fn display_name(&self) -> TextComponent {
                self.custom_name
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
                    .unwrap_or_else(|| {
                        pumpkin_macros::translate_cross!(
                            pumpkin_data::translation::java::CONTAINER_CHEST,
                            pumpkin_data::translation::bedrock::CONTAINER_CHEST
                        )
                    })
            }

            pub fn has_loot_table(&self) -> bool {
                self.loot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .is_some()
            }

            pub fn unpack_loot(&self, player: Option<&Player>) {
                let Some(world) = self
                    .world
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .upgrade()
                else {
                    return;
                };
                let loot = self
                    .loot
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                let Some((key, seed)) = loot else {
                    return;
                };
                if let Some(table) = pumpkin_data::loot_table::get_loot_table(&key) {
                    let seed = if seed == 0 { world.rand_i64() } else { seed };
                    crate::world::loot::fill_inventory_with_context(
                        self,
                        table,
                        seed,
                        &crate::world::loot::LootContextParameters {
                            position: Some(self.position.to_centered_f64()),
                            this_entity: player.map(|_| &pumpkin_data::entity::EntityType::PLAYER),
                            luck: player.map_or(0.0, |player| {
                                player.living_entity.get_attribute_value(
                                    &pumpkin_data::attributes::Attributes::LUCK,
                                ) as f32
                            }),
                            ..Default::default()
                        },
                    );
                }
                self.mark_dirty();
            }

            pub fn get_viewer_count(&self) -> u16 {
                self.viewers.get_viewer_count()
            }

            fn play_sound(
                &self,
                world: &Arc<$crate::world::World>,
                sound: pumpkin_data::sound::Sound,
            ) {
                let state = world.get_block_state(&self.position);
                let properties =
                    pumpkin_data::block_properties::ChestLikeProperties::from_state_id(state.id);
                let block = world.get_block(&self.position);
                let sound = if block.name.contains("copper_chest") {
                    use pumpkin_data::sound::Sound;
                    let open = sound == Sound::BlockChestOpen;
                    if block.name.contains("weathered") {
                        if open {
                            Sound::BlockCopperChestWeatheredOpen
                        } else {
                            Sound::BlockCopperChestWeatheredClose
                        }
                    } else if block.name.contains("oxidized") {
                        if open {
                            Sound::BlockCopperChestOxidizedOpen
                        } else {
                            Sound::BlockCopperChestOxidizedClose
                        }
                    } else if open {
                        Sound::BlockCopperChestOpen
                    } else {
                        Sound::BlockCopperChestClose
                    }
                } else {
                    sound
                };
                let position = match properties.r#type {
                    pumpkin_data::block_properties::ChestType::Left => return,
                    pumpkin_data::block_properties::ChestType::Single => {
                        pumpkin_util::math::vector3::Vector3::new(
                            self.position.0.x as f64 + 0.5,
                            self.position.0.y as f64 + 0.5,
                            self.position.0.z as f64 + 0.5,
                        )
                    }
                    pumpkin_data::block_properties::ChestType::Right => {
                        let direction = pumpkin_data::HorizontalFacingExt::to_block_direction(
                            &properties.facing.rotate_counter_clockwise(),
                        )
                        .to_offset();
                        pumpkin_util::math::vector3::Vector3::new(
                            self.position.0.x as f64 + 0.5 + direction.x as f64 * 0.5,
                            self.position.0.y as f64 + 0.5,
                            self.position.0.z as f64 + 0.5 + direction.z as f64 * 0.5,
                        )
                    }
                };

                world.play_sound_fine(
                    sound,
                    pumpkin_data::sound::SoundCategory::Blocks,
                    &position,
                    0.5,
                    world.rand_f32() * 0.1 + 0.9,
                );
                let bedrock_sound = match sound {
                    pumpkin_data::sound::Sound::BlockChestOpen => "chest.open",
                    pumpkin_data::sound::Sound::BlockChestClose => "chest.closed",
                    _ => return,
                };
                world.play_bedrock_level_sound(bedrock_sound, &position, 0);
            }

            pub fn custom_name(&self) -> Option<TextComponent> {
                self.custom_name
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone()
            }
        }
    };
}
