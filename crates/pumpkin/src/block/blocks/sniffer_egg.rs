use crate::block::{BlockBehaviour, OnScheduledTickArgs, PathComputationType, PlacedArgs};
use crate::entity::r#type::from_type;
use pumpkin_data::{
    BlockState,
    block_properties::SnifferEggLikeProperties,
    entity::EntityType,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    world::WorldEvent,
};
use pumpkin_macros::pumpkin_block;
use pumpkin_world::{tick::TickPriority, world::BlockFlags};
use uuid::Uuid;

#[pumpkin_block("minecraft:sniffer_egg")]
pub struct SnifferEggBlock;
impl BlockBehaviour for SnifferEggBlock {
    fn placed(&self, args: PlacedArgs<'_>) {
        let boosted = args
            .world
            .get_block(&args.position.down())
            .has_tag(&tag::Block::MINECRAFT_SNIFFER_EGG_HATCH_BOOST);
        if boosted {
            args.world
                .sync_world_event(WorldEvent::ParticlesEggCrack, *args.position, 0);
        }
        args.world.emit_game_event_from_entity(
            "block_place",
            args.position.to_centered_f64(),
            None,
            Some(args.state_id),
        );
        let progression_delay = if boosted { 4000 } else { 8000 };
        args.world.schedule_block_tick(
            args.block,
            *args.position,
            progression_delay + args.world.rand_bounded_i32(300) as u32,
            TickPriority::Normal,
        );
    }
    fn state_changed(&self, args: PlacedArgs<'_>) {
        self.placed(args);
    }
    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let mut props =
            SnifferEggLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        let sound = if props.hatch < 2 {
            Sound::BlockSnifferEggCrack
        } else {
            Sound::BlockSnifferEggHatch
        };
        args.world.play_sound_fine(
            sound,
            SoundCategory::Blocks,
            &args.position.to_centered_f64(),
            0.7,
            0.9 + args.world.rand_f32() * 0.2,
        );
        if props.hatch < 2 {
            props.hatch += 1;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_LISTENERS,
            );
        } else {
            args.world.break_block(
                args.position,
                None,
                BlockFlags::NOTIFY_ALL | BlockFlags::SKIP_DROPS,
            );
            let sniffer = from_type(
                &EntityType::SNIFFER,
                args.position.to_centered_f64(),
                args.world,
                Uuid::new_v4(),
            );
            if let Some(ageable) = sniffer.get_mob().and_then(|mob| mob.as_ageable()) {
                ageable.set_baby(true);
            }
            let mut yaw = args.world.rand_f32() * 360.0;
            if yaw >= 180.0 {
                yaw -= 360.0;
            }
            sniffer.get_entity().set_rotation(yaw, 0.0);
            args.world.spawn_entity(sniffer);
        }
    }
    fn is_pathfindable(&self, _state: &BlockState, _kind: PathComputationType) -> bool {
        false
    }
}
