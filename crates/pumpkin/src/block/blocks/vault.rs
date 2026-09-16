use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{VaultLikeProperties, VaultState};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

use crate::block::entities::vault::VaultBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{BlockBehaviour, OnPlaceArgs, UseWithItemArgs};

#[pumpkin_block("minecraft:vault")]
pub struct VaultBlock;

impl BlockBehaviour for VaultBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = VaultLikeProperties::default(args.block);
        props.facing = args
            .player
            .living_entity
            .entity
            .get_horizontal_facing()
            .opposite();
        props.to_state_id(args.block)
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state =
            VaultLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        if args.item_stack.is_empty() || state.vault_state != VaultState::Active {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(vault_entity) = block_entity.as_any().downcast_ref::<VaultBlockEntity>()
        {
            let key = vault_entity
                .config
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .as_ref()
                .and_then(|config| config.get_compound("key_item"))
                .and_then(ItemStack::read_item_stack)
                .unwrap_or_else(|| ItemStack::new(1, &Item::TRIAL_KEY));
            if key.is_empty() {
                return BlockActionResult::Success;
            }
            if !args.item_stack.are_items_and_components_equal(&key)
                || args.item_stack.item_count < key.item_count
            {
                args.world.play_sound(
                    Sound::BlockVaultInsertItemFail,
                    SoundCategory::Blocks,
                    &args.position.to_centered_f64(),
                );
                return BlockActionResult::Success;
            }
            let player_uuid = args.player.gameprofile.id;

            if vault_entity.has_rewarded(&player_uuid) {
                args.world.play_sound(
                    Sound::BlockVaultRejectRewardedPlayer,
                    SoundCategory::Blocks,
                    &args.position.to_f64(),
                );
                return BlockActionResult::Success;
            }

            vault_entity.mark_rewarded(player_uuid);

            args.item_stack
                .decrement_unless_creative(args.player.gamemode.load(), key.item_count);

            args.world.play_sound(
                Sound::BlockVaultInsertItem,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );
            args.world.play_sound(
                Sound::BlockVaultOpenShutter,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );

            let state_id = args.world.get_block_state_id(args.position);
            let mut props = VaultLikeProperties::from_state_id(state_id);
            props.vault_state = VaultState::Ejecting;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );

            args.world.play_sound(
                Sound::BlockVaultEjectItem,
                SoundCategory::Blocks,
                &args.position.to_f64(),
            );

            // Spawn trial vault loot (emeralds, diamond, iron)
            let loot_stacks = vec![
                ItemStack::new(4, &Item::EMERALD),
                ItemStack::new(1, &Item::DIAMOND),
                ItemStack::new(2, &Item::IRON_INGOT),
            ];

            for stack in loot_stacks {
                args.world.drop_stack(args.position, stack);
            }

            props.vault_state = VaultState::Active;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );

            return BlockActionResult::Success;
        }

        BlockActionResult::PassToDefaultBlockAction
    }
}
