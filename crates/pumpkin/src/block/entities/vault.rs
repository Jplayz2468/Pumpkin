use super::BlockEntity;
use crate::entity::{Entity, EntityBase, item::ItemEntity, player::Player};
use crate::world::{
    World,
    loot::{LootContextParameters, generate_loot_in_world},
};
use pumpkin_data::{
    Block,
    attributes::Attributes,
    block_properties::{VaultLikeProperties, VaultState},
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    sound::{Sound, SoundCategory},
    world::WorldEvent,
};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use pumpkin_world::world::BlockFlags;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[derive(Default)]
struct ServerData {
    rewarded: VecDeque<Uuid>,
    resumes_at: i64,
    items: Vec<ItemStack>,
    total: i32,
    last_fail: i64,
}

pub struct VaultBlockEntity {
    pub position: BlockPos,
    pub config: Mutex<Option<NbtCompound>>,
    data: Mutex<ServerData>,
    shared: Mutex<NbtCompound>,
}

fn uuid_tag(id: Uuid) -> NbtTag {
    let mut nbt = NbtCompound::new();
    nbt.put_uuid("id", id);
    nbt.child_tags.remove("id").unwrap()
}

fn item_nbt(stack: &ItemStack) -> NbtCompound {
    let mut nbt = NbtCompound::new();
    stack.write_item_stack(&mut nbt);
    nbt
}

impl VaultBlockEntity {
    pub const ID: &'static str = "minecraft:vault";

    pub fn new(position: BlockPos) -> Self {
        Self::from_nbt(&NbtCompound::new(), position)
    }

    fn key(&self) -> ItemStack {
        self.config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map_or_else(
                || ItemStack::new(1, &Item::TRIAL_KEY),
                |config| {
                    config
                        .get_compound("key_item")
                        .and_then(ItemStack::read_item_stack)
                        .unwrap_or_else(|| ItemStack::EMPTY.clone())
                },
            )
    }

    fn resolve_loot(
        &self,
        world: &World,
        player: Option<&Player>,
        tool: Option<&ItemStack>,
        display: bool,
    ) -> Vec<ItemStack> {
        let config = self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let name = config
            .as_ref()
            .and_then(|c| {
                display
                    .then(|| c.get_string("override_loot_table_to_display"))
                    .flatten()
                    .or_else(|| c.get_string("loot_table"))
            })
            .unwrap_or("minecraft:chests/trial_chambers/reward");
        let Some(table) = pumpkin_data::loot_table::get_loot_table(name) else {
            return Vec::new();
        };
        let params = LootContextParameters {
            position: Some(self.position.to_centered_f64()),
            this_entity: player.map(|_| &EntityType::PLAYER),
            luck: player.map_or(0.0, |p| {
                p.living_entity.get_attribute_value(&Attributes::LUCK) as f32
            }),
            tool: tool.cloned(),
            ..Default::default()
        };
        generate_loot_in_world(world, table, 0, &params)
    }

    fn display(&self, stack: &ItemStack) {
        let mut shared = self
            .shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if stack.is_empty() {
            shared.child_tags.remove("display_item");
        } else {
            shared.put_compound("display_item", item_nbt(stack));
        }
    }

    fn cycle_display(&self, world: &World) {
        let items = if self.key().is_empty() {
            Vec::new()
        } else {
            self.resolve_loot(world, None, None, true)
        };
        if items.is_empty() {
            self.display(&ItemStack::EMPTY);
        } else {
            self.display(&items[world.rand_bounded_i32(items.len() as i32) as usize]);
        }
    }

    fn connected(&self, world: &World, data: &ServerData, active: bool) -> VaultState {
        let config = self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let range = config
            .as_ref()
            .and_then(|c| {
                c.get_double(if active {
                    "deactivation_range"
                } else {
                    "activation_range"
                })
            })
            .unwrap_or(if active { 4.5 } else { 4.0 });
        let connected: Vec<_> = world
            .get_nearby_players(self.position.to_centered_f64(), range + 2.0)
            .iter()
            .filter(|p| !p.is_spectator() && !data.rewarded.contains(&p.gameprofile.id))
            .filter(|p| {
                p.get_entity()
                    .block_pos
                    .load()
                    .to_f64()
                    .squared_distance_to_vec(&self.position.to_f64())
                    < range * range
            })
            .map(|p| uuid_tag(p.gameprofile.id))
            .collect();
        let result = if connected.is_empty() {
            VaultState::Inactive
        } else {
            VaultState::Active
        };
        self.shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .put("connected_players", NbtTag::List(connected));
        result
    }

    fn transition(&self, world: &Arc<World>, mut props: VaultLikeProperties, state: VaultState) {
        let old = props.vault_state;
        if old == state {
            return;
        }
        props.vault_state = state;
        world.set_block_state(
            &self.position,
            props.to_state_id(&Block::VAULT),
            BlockFlags::NOTIFY_ALL,
        );
        if old == VaultState::Ejecting {
            world.play_sound(
                Sound::BlockVaultCloseShutter,
                SoundCategory::Blocks,
                &self.position.to_centered_f64(),
            );
        }
        match state {
            VaultState::Inactive => {
                self.display(&ItemStack::EMPTY);
                world.sync_world_event(
                    WorldEvent::AnimationVaultDeactivate,
                    self.position,
                    i32::from(props.ominous),
                );
            }
            VaultState::Active => {
                let has_display = self
                    .shared
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get_compound("display_item")
                    .is_some();
                if !has_display {
                    self.cycle_display(world);
                }
                world.sync_world_event(
                    WorldEvent::AnimationVaultActivate,
                    self.position,
                    i32::from(props.ominous),
                );
            }
            VaultState::Unlocking => world.play_sound(
                Sound::BlockVaultInsertItem,
                SoundCategory::Blocks,
                &self.position.to_centered_f64(),
            ),
            VaultState::Ejecting => world.play_sound(
                Sound::BlockVaultOpenShutter,
                SoundCategory::Blocks,
                &self.position.to_centered_f64(),
            ),
        }
    }

    fn sync(&self, world: &World) {
        if let Some(entity) = world.get_block_entity(&self.position) {
            world.update_block_entity(&entity);
        }
    }

    pub fn try_insert(&self, world: &Arc<World>, player: &Player, stack: &mut ItemStack) {
        let props = VaultLikeProperties::from_state_id(world.get_block_state_id(&self.position));
        if props.vault_state != VaultState::Active {
            return;
        }
        let key = self.key();
        if key.is_empty() {
            return;
        }
        let mut data = self
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let fail =
            if !stack.are_items_and_components_equal(&key) || stack.item_count < key.item_count {
                Some(Sound::BlockVaultInsertItemFail)
            } else if data.rewarded.contains(&player.gameprofile.id) {
                Some(Sound::BlockVaultRejectRewardedPlayer)
            } else {
                None
            };
        if let Some(sound) = fail {
            if world.get_world_age() >= data.last_fail + 15 {
                data.last_fail = world.get_world_age();
                world.play_sound(
                    sound,
                    SoundCategory::Blocks,
                    &self.position.to_centered_f64(),
                );
            }
            return;
        }
        let items = self.resolve_loot(world, Some(player), Some(stack), false);
        if items.is_empty() {
            return;
        }
        player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Used,
            stack.item.id as i32,
            1,
        );
        stack.decrement_unless_creative(player.gamemode.load(), key.item_count);
        data.total = items.len() as i32;
        data.items = items;
        self.display(data.items.last().unwrap());
        data.resumes_at = world.get_world_age() + 14;
        data.rewarded.push_back(player.gameprofile.id);
        if data.rewarded.len() > 128 {
            data.rewarded.pop_front();
        }
        self.connected(world, &data, true);
        drop(data);
        self.transition(world, props, VaultState::Unlocking);
        self.sync(world);
    }
}

impl BlockEntity for VaultBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn tick(&self, world: &Arc<World>) {
        if world.get_block(&self.position) != &Block::VAULT {
            return;
        }
        let props = VaultLikeProperties::from_state_id(world.get_block_state_id(&self.position));
        let now = world.get_world_age();
        let cycle = now % 20 == 0 && props.vault_state == VaultState::Active;
        if cycle {
            self.cycle_display(world);
        }
        let mut data = self
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if now < data.resumes_at {
            drop(data);
            if cycle {
                self.sync(world);
            }
            return;
        }
        data.resumes_at = now + 20;
        let next = match props.vault_state {
            VaultState::Inactive => self.connected(world, &data, false),
            VaultState::Active => self.connected(world, &data, true),
            VaultState::Unlocking => VaultState::Ejecting,
            VaultState::Ejecting => {
                let progress = if data.total == 1 {
                    1.0
                } else {
                    1.0 - (data.items.len() as f32 - 1.0) / (data.total as f32 - 1.0)
                };
                if let Some(stack) = data.items.pop() {
                    let item = ItemEntity::new(
                        Entity::new(
                            world.clone(),
                            self.position.to_f64().add_raw(0.5, 1.075, 0.5),
                            &EntityType::ITEM,
                        ),
                        stack,
                    );
                    // DefaultDispenseItemBehavior.spawnItem: UP, accuracy 2.
                    let _power = world.rand_f64() * 0.1 + 0.2;
                    let triangle = |mean| mean + 0.034455 * (world.rand_f64() - world.rand_f64());
                    item.get_entity().velocity.store(Vector3::new(
                        triangle(0.0),
                        triangle(0.2),
                        triangle(0.0),
                    ));
                    world.spawn_entity(Arc::new(item));
                    world.sync_world_event(WorldEvent::AnimationVaultEjectItem, self.position, 0);
                    world.play_sound_fine(
                        Sound::BlockVaultEjectItem,
                        SoundCategory::Blocks,
                        &self.position.to_centered_f64(),
                        1.0,
                        0.8 + 0.4 * progress,
                    );
                    self.display(data.items.last().unwrap_or(&ItemStack::EMPTY));
                    VaultState::Ejecting
                } else {
                    data.total = 0;
                    self.connected(world, &data, true)
                }
            }
        };
        drop(data);
        self.transition(world, props, next);
        self.sync(world);
    }

    fn from_nbt(nbt: &NbtCompound, position: BlockPos) -> Self {
        let mut data = ServerData::default();
        if let Some(server) = nbt.get_compound("server_data") {
            if let Some(ids) = server.get_list("rewarded_players") {
                for tag in ids {
                    let mut id = NbtCompound::new();
                    id.put("id", tag.clone());
                    if let Some(id) = id.get_uuid("id")
                        && !data.rewarded.contains(&id)
                    {
                        data.rewarded.push_back(id);
                    }
                }
            }
            data.resumes_at = server.get_long("state_updating_resumes_at").unwrap_or(0);
            data.total = server.get_int("total_ejections_needed").unwrap_or(0);
            data.items = server
                .get_list("items_to_eject")
                .map(|list| {
                    list.iter()
                        .filter_map(|tag| {
                            tag.extract_compound().and_then(ItemStack::read_item_stack)
                        })
                        .collect()
                })
                .unwrap_or_default();
        }
        Self {
            position,
            config: Mutex::new(nbt.get_compound("config").cloned()),
            data: Mutex::new(data),
            shared: Mutex::new(nbt.get_compound("shared_data").cloned().unwrap_or_default()),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(config) = self
            .config
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            nbt.put_compound("config", config.clone());
        }
        let data = self
            .data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut server = NbtCompound::new();
        server.put(
            "rewarded_players",
            NbtTag::List(data.rewarded.iter().copied().map(uuid_tag).collect()),
        );
        server.put_long("state_updating_resumes_at", data.resumes_at);
        server.put_int("total_ejections_needed", data.total);
        server.put(
            "items_to_eject",
            NbtTag::List(
                data.items
                    .iter()
                    .map(|stack| NbtTag::Compound(item_nbt(stack)))
                    .collect(),
            ),
        );
        nbt.put_compound("server_data", server);
        nbt.put_compound(
            "shared_data",
            self.shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        );
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        nbt.put_compound(
            "shared_data",
            self.shared
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        );
        Some(nbt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reward_history_and_in_flight_ejection_survive_reload() {
        let mut server = NbtCompound::new();
        let ids = [
            Uuid::from_u128(0x123456789abcdef0123456789abcdef0),
            Uuid::from_u128(u128::MAX),
        ];
        server.put(
            "rewarded_players",
            NbtTag::List(ids.into_iter().map(uuid_tag).collect()),
        );
        server.put_long("state_updating_resumes_at", 1234);
        server.put_int("total_ejections_needed", 3);
        server.put(
            "items_to_eject",
            NbtTag::List(vec![NbtTag::Compound(item_nbt(&ItemStack::new(
                2,
                &Item::DIAMOND,
            )))]),
        );
        let mut nbt = NbtCompound::new();
        nbt.put_compound("server_data", server.clone());
        let vault = VaultBlockEntity::from_nbt(&nbt, BlockPos::new(0, 64, 0));
        let mut saved = NbtCompound::new();
        vault.write_nbt(&mut saved);
        assert_eq!(saved.get_compound("server_data"), Some(&server));
        assert!(
            vault
                .chunk_data_nbt()
                .unwrap()
                .get_compound("server_data")
                .is_none()
        );
        assert_eq!(
            vault
                .data
                .lock()
                .unwrap()
                .rewarded
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            ids
        );
    }
}
