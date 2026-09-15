use crate::{
    block::{
        BlockBehaviour, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, NormalUseArgs,
        OnEntityCollisionArgs, RandomTickArgs, UseWithItemArgs,
        blocks::plant::{PlantBlockBase, crop::CropBlockBase},
        registry::BlockActionResult,
    },
    world::World,
};
use pumpkin_data::{
    Block, BlockStateId,
    block_properties::NetherWartLikeProperties,
    damage::DamageType,
    entity::EntityType,
    item::Item,
    sound::{Sound, SoundCategory},
};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::{BlockAccessor, BlockFlags};

#[pumpkin_block("minecraft:sweet_berry_bush")]
pub struct SweetBerryBushBlock;

impl BlockBehaviour for SweetBerryBushBlock {
    fn is_valid_bonemeal_target(&self, args: crate::block::BonemealArgs<'_>) -> bool {
        <Self as CropBlockBase>::is_valid_bonemeal_target(self, args.world, args.position)
    }

    fn perform_bonemeal(&self, args: crate::block::BonemealArgs<'_>) {
        <Self as CropBlockBase>::perform_bonemeal(self, args.world, args.position);
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state_id = args.world.get_block_state_id(args.position);
        let mut props = NetherWartLikeProperties::from_state_id(state_id);
        match props.age {
            2 | 3 => {
                if !super::super::harvest_loot(
                    &args,
                    &pumpkin_data::loot_table::HARVEST_SWEET_BERRY_BUSH,
                ) {
                    return BlockActionResult::Pass;
                }
                args.world.play_sound_fine(
                    Sound::BlockSweetBerryBushPickBerries,
                    SoundCategory::Blocks,
                    &args.position.to_centered_f64(),
                    1.0,
                    0.8_f32 + args.world.rand_f32() * 0.4_f32,
                );
                props.age = 1;
                args.world.set_block_state(
                    args.position,
                    props.to_state_id(&Block::SWEET_BERRY_BUSH),
                    BlockFlags::NOTIFY_LISTENERS,
                );
                args.world.emit_game_event_from_entity(
                    "block_change",
                    args.position.to_centered_f64(),
                    Some(args.player.as_ref()),
                    Some(props.to_state_id(&Block::SWEET_BERRY_BUSH)),
                );
                BlockActionResult::SuccessServer
            }
            _ => BlockActionResult::Pass,
        }
    }

    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let state_id = args.world.get_block_state_id(args.position);
        let props = NetherWartLikeProperties::from_state_id(state_id);
        if props.age != 3 && args.item_stack.get_item() == &Item::BONE_MEAL {
            BlockActionResult::Pass
        } else {
            BlockActionResult::PassToDefaultBlockAction
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        <Self as PlantBlockBase>::can_place_at(self, args.block_accessor, args.position)
    }

    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        <Self as PlantBlockBase>::get_state_for_neighbor_update(
            self,
            args.world,
            args.position,
            args.state_id,
        )
    }

    fn on_entity_collision(&self, args: OnEntityCollisionArgs<'_>) {
        let entity = args.entity.get_entity();

        let living_entity_opt = args.entity.get_living_entity();
        let Some(living_entity) = living_entity_opt else {
            return;
        };
        if entity.entity_type == &EntityType::FOX || entity.entity_type == &EntityType::BEE {
            return;
        }
        entity.slow_movement(
            args.state,
            Vector3::new(f64::from(0.8_f32), f64::from(0.75_f32), f64::from(0.8_f32)),
        );
        let mov = if living_entity.is_player() {
            living_entity.get_movement()
        } else {
            entity.last_pos.load() - entity.pos.load()
        };

        let state_id = args.world.get_block_state_id(args.position);
        let props = NetherWartLikeProperties::from_state_id(state_id);
        if props.age == 0 {
            return;
        }

        if mov.horizontal_length_squared() <= 0.0
            || (mov.x.abs() < f64::from(0.003_f32) && mov.z.abs() < f64::from(0.003_f32))
        {
            return;
        }

        args.entity
            .damage(args.entity, 1.0, DamageType::SWEET_BERRY_BUSH);
    }

    fn random_tick(&self, mut args: RandomTickArgs<'_>) {
        let mut props =
            NetherWartLikeProperties::from_state_id(args.world.get_block_state_id(args.position));
        if props.age < 3
            && args.rand_bounded_i32(5) == 0
            && args.world.get_raw_brightness(&args.position.up(), 0) >= 9
        {
            props.age += 1;
            let state = props.to_state_id(args.block);
            args.world
                .set_block_state(args.position, state, BlockFlags::NOTIFY_LISTENERS);
            args.world.emit_game_event_from_entity(
                "block_change",
                args.position.to_centered_f64(),
                None,
                Some(state),
            );
        }
    }
}

impl PlantBlockBase for SweetBerryBushBlock {
    fn get_state_for_neighbor_update(
        &self,
        block_accessor: &dyn BlockAccessor,
        block_pos: &BlockPos,
        block_state: BlockStateId,
    ) -> BlockStateId {
        if !<Self as PlantBlockBase>::can_place_at(self, block_accessor, block_pos) {
            return Block::AIR.default_state.id;
        }
        block_state
    }

    fn can_place_at(&self, block_accessor: &dyn BlockAccessor, block_pos: &BlockPos) -> bool {
        <Self as PlantBlockBase>::can_plant_on_top(self, block_accessor, &block_pos.down())
    }
}

impl CropBlockBase for SweetBerryBushBlock {
    fn bonemeal_age_increase(&self, _world: &World) -> i32 {
        1
    }

    fn max_age(&self) -> i32 {
        3
    }

    fn get_age(&self, state: BlockStateId, _block: &Block) -> i32 {
        let props = NetherWartLikeProperties::from_state_id(state);
        i32::from(props.age)
    }

    fn state_with_age(&self, block: &Block, state: BlockStateId, age: i32) -> BlockStateId {
        let mut props = NetherWartLikeProperties::from_state_id(state);
        props.age = age as u8;
        props.to_state_id(block)
    }
}
