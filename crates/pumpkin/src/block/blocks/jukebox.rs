use crate::block::entities::jukebox::JukeboxBlockEntity;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, EmitsRedstonePowerArgs, GetComparatorOutputArgs, GetRedstonePowerArgs,
    NormalUseArgs, OnStateReplacedArgs, PlayerPlacedArgs, UseWithItemArgs,
};
use pumpkin_data::block_properties::JukeboxLikeProperties;
use pumpkin_data::data_component_impl::{BlockEntityDataImpl, JukeboxPlayableImpl};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;

#[pumpkin_block("minecraft:jukebox")]
pub struct JukeboxBlock;

impl BlockBehaviour for JukeboxBlock {
    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        if JukeboxLikeProperties::from_state_id(args.world.get_block_state_id(args.position))
            .has_record
            && let Some(entity) = args.world.get_block_entity(args.position)
            && let Some(jukebox) = entity.as_any().downcast_ref::<JukeboxBlockEntity>()
        {
            jukebox.pop_out_item();
            BlockActionResult::Success
        } else {
            BlockActionResult::Pass
        }
    }

    fn player_placed(&self, args: PlayerPlacedArgs<'_>) {
        if let Some(data) = args.item_stack.get_data_component::<BlockEntityDataImpl>()
            && data.nbt.child_tags.contains_key("RecordItem")
        {
            args.world.set_block_state(
                args.position,
                JukeboxLikeProperties { has_record: true }.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
        }
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state_id(args.position);
        if JukeboxLikeProperties::from_state_id(state).has_record
            || args
                .item_stack
                .get_data_component::<JukeboxPlayableImpl>()
                .is_none()
        {
            return BlockActionResult::PassToDefaultBlockAction;
        }
        // JukeboxPlayable consumes and awards the statistic even if the block entity is missing.
        let record = args
            .item_stack
            .split_unless_creative(args.player.gamemode.load(), 1);
        if let Some(entity) = args.world.get_block_entity(args.position)
            && let Some(jukebox) = entity.as_any().downcast_ref::<JukeboxBlockEntity>()
        {
            jukebox.set_record(record);
            args.world.emit_game_event_from_entity(
                "block_change",
                args.position.to_centered_f64(),
                Some(args.player.as_ref()),
                Some(state),
            );
        }
        args.player.increment_stat(
            pumpkin_data::statistic::StatisticCategory::Custom,
            pumpkin_data::statistic::CustomStatistic::PlayRecord as i32,
            1,
        );
        BlockActionResult::Success
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        args.world
            .update_neighbour_for_output_signal(args.position, args.block);
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if let Some(entity) = args.world.get_block_entity(args.position)
            && let Some(jukebox) = entity.as_any().downcast_ref::<JukeboxBlockEntity>()
            && jukebox.is_playing()
        {
            15
        } else {
            0
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        Some(
            args.world
                .get_block_entity(args.position)
                .and_then(|entity| {
                    entity
                        .as_any()
                        .downcast_ref::<JukeboxBlockEntity>()
                        .and_then(|jukebox| {
                            JukeboxBlockEntity::song_from_stack(&jukebox.get_record())
                        })
                })
                .map_or(0, |song| song.comparator_output()),
        )
    }
}
