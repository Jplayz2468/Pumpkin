use crate::block::BlockBehaviour;
use pumpkin_macros::pumpkin_block;

#[pumpkin_block("minecraft:trial_spawner")]
pub struct TrialSpawnerBlock;

// Creation, removal and ticking use the registry's block-entity lifecycle.
impl BlockBehaviour for TrialSpawnerBlock {}
