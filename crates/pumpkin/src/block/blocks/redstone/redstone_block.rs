use pumpkin_macros::pumpkin_block;

use crate::block::{BlockBehaviour, EmitsRedstonePowerArgs, GetRedstonePowerArgs};

#[pumpkin_block("minecraft:redstone_block")]
pub struct RedstoneBlock;

impl BlockBehaviour for RedstoneBlock {
    fn get_weak_redstone_power(&self, _args: GetRedstonePowerArgs<'_>) -> u8 {
        15
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_data::{Block, BlockDirection};

    #[test]
    fn redstone_block_constants() {
        let block = &Block::REDSTONE_BLOCK;
        // In vanilla PoweredBlock.java:
        // isSignalSource() -> true
        // ownSignal() -> 15 (weak power)
        // getDirectSignal() -> 0 (strong power, default implementation)
        assert_eq!(block.id, Block::REDSTONE_BLOCK.id);
        let rb = RedstoneBlock;
        assert!(rb.emits_redstone_power(EmitsRedstonePowerArgs {
            block,
            state: &block.default_state,
            direction: BlockDirection::North,
        }));
    }
}

