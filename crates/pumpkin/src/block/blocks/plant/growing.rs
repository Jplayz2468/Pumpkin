use crate::{
    block::{
        BonemealArgs, CanPlaceAtArgs, GetStateForNeighborUpdateArgs, OnPlaceArgs,
        OnScheduledTickArgs, RandomTickArgs,
    },
    world::World,
};
use pumpkin_data::block_properties::{
    CaveVinesLikeProperties, CaveVinesPlantLikeProperties, KelpLikeProperties,
};
use pumpkin_data::{
    Block, BlockDirection, BlockState, BlockStateId,
    fluid::Fluid,
    tag::{self, Taggable},
};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::{
    tick::TickPriority,
    world::{BlockAccessor, BlockFlags},
};

/// GrowingPlantBlock / GrowingPlantHeadBlock / GrowingPlantBodyBlock behavior.
pub(super) struct GrowingPlant {
    head: &'static Block,
    body: &'static Block,
    direction: BlockDirection,
    probability: f64,
}

pub(super) const KELP: GrowingPlant = GrowingPlant {
    head: &Block::KELP,
    body: &Block::KELP_PLANT,
    direction: BlockDirection::Up,
    probability: 0.14,
};
pub(super) const TWISTING: GrowingPlant = GrowingPlant {
    head: &Block::TWISTING_VINES,
    body: &Block::TWISTING_VINES_PLANT,
    direction: BlockDirection::Up,
    probability: 0.1,
};
pub(super) const WEEPING: GrowingPlant = GrowingPlant {
    head: &Block::WEEPING_VINES,
    body: &Block::WEEPING_VINES_PLANT,
    direction: BlockDirection::Down,
    probability: 0.1,
};
pub(super) const CAVE: GrowingPlant = GrowingPlant {
    head: &Block::CAVE_VINES,
    body: &Block::CAVE_VINES_PLANT,
    direction: BlockDirection::Down,
    probability: 0.1,
};

impl GrowingPlant {
    fn is_kelp(&self) -> bool {
        self.head == &Block::KELP
    }
    fn is_cave(&self) -> bool {
        self.head == &Block::CAVE_VINES
    }
    fn contains(&self, block: &Block) -> bool {
        block == self.head || block == self.body
    }
    fn forward(&self, pos: &BlockPos) -> BlockPos {
        pos.offset(self.direction.to_offset())
    }

    pub fn can_survive(&self, accessor: &dyn BlockAccessor, pos: &BlockPos) -> bool {
        let (block, state) =
            accessor.get_block_and_state(&pos.offset(self.direction.opposite().to_offset()));
        (!self.is_kelp() || !block.has_tag(&tag::Block::MINECRAFT_CANNOT_SUPPORT_KELP))
            && (self.contains(block) || state.is_side_solid(self.direction))
    }

    pub fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        self.can_survive(args.block_accessor, args.position)
            && (!self.is_kelp()
                || args.use_item_on.is_none()
                || super::full_water_at(args.block_accessor, args.position))
    }

    fn berries(&self, state: BlockStateId) -> bool {
        if !self.is_cave() {
            return false;
        }
        if state.to_block() == self.head {
            CaveVinesLikeProperties::from_state_id(state).berries
        } else {
            CaveVinesPlantLikeProperties::from_state_id(state).berries
        }
    }

    fn head_state(&self, age: u8, berries: bool) -> BlockStateId {
        if self.is_cave() {
            CaveVinesLikeProperties { age, berries }.to_state_id(self.head)
        } else {
            KelpLikeProperties { age }.to_state_id(self.head)
        }
    }

    fn body_state(&self, state: BlockStateId) -> BlockStateId {
        if self.is_cave() {
            CaveVinesPlantLikeProperties {
                berries: self.berries(state),
            }
            .to_state_id(self.body)
        } else {
            self.body.default_state.id
        }
    }

    fn age(&self, state: BlockStateId) -> u8 {
        if self.is_cave() {
            CaveVinesLikeProperties::from_state_id(state).age
        } else {
            KelpLikeProperties::from_state_id(state).age
        }
    }

    pub fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if self.contains(args.world.get_block(&self.forward(args.position))) {
            self.body.default_state.id
        } else {
            self.head_state(args.world.rand_bounded_i32(25) as u8, false)
        }
    }

    pub fn neighbor_state(&self, args: GetStateForNeighborUpdateArgs<'_>) -> BlockStateId {
        let is_head = args.block == self.head;
        if args.direction == self.direction.opposite() {
            if !self.can_survive(args.world, args.position) {
                args.world
                    .schedule_block_tick(args.block, *args.position, 1, TickPriority::Normal);
            } else if is_head && self.contains(args.world.get_block(&self.forward(args.position))) {
                return self.body_state(args.state_id);
            }
        }
        if args.direction == self.direction {
            let connected = self.contains(args.neighbor_state_id.to_block());
            if is_head && connected {
                return self.body_state(args.state_id);
            }
            if !is_head && !connected {
                return self.head_state(
                    args.world.rand_bounded_i32(25) as u8,
                    self.berries(args.state_id),
                );
            }
        }
        if self.is_kelp() {
            args.world.schedule_fluid_tick(
                &Fluid::WATER,
                *args.position,
                Fluid::WATER.flow_speed as u32,
                TickPriority::Normal,
            );
        }
        args.state_id
    }

    pub fn scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        if !self.can_survive(args.world.as_ref(), args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
        }
    }

    fn can_grow_into(&self, state: &BlockState) -> bool {
        if self.is_kelp() {
            state.id.to_block() == &Block::WATER
        } else {
            state.is_air()
        }
    }

    pub fn random_tick(&self, mut args: RandomTickArgs<'_>) {
        if args.block != self.head {
            return;
        }
        let state = args.world.get_block_state_id(args.position);
        let age = self.age(state);
        if age < 25 && args.rand_f64() < self.probability {
            let forward = self.forward(args.position);
            if self.can_grow_into(args.world.get_block_state(&forward)) {
                let berries = self.is_cave() && args.rand_f32() < 0.11_f32;
                args.world.set_block_state(
                    &forward,
                    self.head_state(age + 1, berries),
                    BlockFlags::NOTIFY_ALL,
                );
            }
        }
    }

    fn head_pos(&self, world: &World, pos: &BlockPos) -> Option<BlockPos> {
        let mut pos = *pos;
        while world.is_in_height_limit(pos.0.y) {
            let block = world.get_block(&pos);
            if block == self.head {
                return Some(pos);
            }
            if block != self.body {
                return None;
            }
            pos = self.forward(&pos);
        }
        None
    }

    pub fn bonemeal_target(&self, args: BonemealArgs<'_>) -> bool {
        if self.is_cave() {
            return !self.berries(args.state_id);
        }
        self.head_pos(args.world, args.position).is_some_and(|pos| {
            let forward = self.forward(&pos);
            self.can_grow_into(args.world.get_block_state(&forward))
                && args.world.is_in_height_limit(forward.0.y)
        })
    }

    pub fn bonemeal(&self, args: BonemealArgs<'_>) {
        if self.is_cave() {
            let state = if args.block == self.head {
                self.head_state(self.age(args.state_id), true)
            } else {
                CaveVinesPlantLikeProperties { berries: true }.to_state_id(self.body)
            };
            args.world
                .set_block_state(args.position, state, BlockFlags::NOTIFY_LISTENERS);
            return;
        }
        let Some(head) = self.head_pos(args.world, args.position) else {
            return;
        };
        let mut age = (self.age(args.world.get_block_state_id(&head)) + 1).min(25);
        let count = if self.is_kelp() {
            1
        } else {
            let mut probability = 1.0;
            let mut count = 0;
            while args.world.rand_f64() < probability {
                probability *= 0.826;
                count += 1;
            }
            count
        };
        let mut forward = self.forward(&head);
        for _ in 0..count {
            if !self.can_grow_into(args.world.get_block_state(&forward))
                || !args.world.is_in_height_limit(forward.0.y)
            {
                break;
            }
            args.world.set_block_state(
                &forward,
                self.head_state(age, false),
                BlockFlags::NOTIFY_ALL,
            );
            forward = self.forward(&forward);
            age = (age + 1).min(25);
        }
    }
}
