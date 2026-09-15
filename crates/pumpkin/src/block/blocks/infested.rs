use uuid::Uuid;

use pumpkin_data::entity::EntityType;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::math::vector3::Vector3;

use crate::block::BlockBehaviour;
use crate::entity::r#type::from_type;

// The `c:cobblestones/infested` tag only lists `infested_cobblestone`
// (assets/datapacks/26_2/data/c/tags/block/cobblestones/infested.json), so the stone/stone-brick
// variants must be registered explicitly here instead of via that tag.
#[pumpkin_block(
    "infested_stone",
    "infested_cobblestone",
    "infested_stone_bricks",
    "infested_mossy_stone_bricks",
    "infested_cracked_stone_bricks",
    "infested_chiseled_stone_bricks"
)]
pub struct InfestedBlock;

impl BlockBehaviour for InfestedBlock {
    fn spawn_after_break(&self, args: crate::block::SpawnAfterBreakArgs<'_>) {
        if !args.world.level_info.load().game_rules.block_drops {
            return;
        }
        if args
            .params
            .tool
            .as_ref()
            .and_then(|tool| {
                tool.get_data_component::<pumpkin_data::data_component_impl::EnchantmentsImpl>()
            })
            .is_some_and(|data| {
                data.enchantment.iter().any(|(enchantment, _)| {
                    pumpkin_data::tag::Taggable::has_tag(
                        *enchantment,
                        &pumpkin_data::tag::Enchantment::MINECRAFT_PREVENTS_INFESTED_SPAWNS,
                    )
                })
            })
        {
            return;
        }
        let pos = args.position.0.to_f64();
        let silverfish = from_type(
            &EntityType::SILVERFISH,
            Vector3::new(pos.x + 0.5, pos.y, pos.z + 0.5),
            args.world,
            Uuid::new_v4(),
        );
        silverfish.get_entity().set_rotation(0.0, 0.0);
        args.world.spawn_entity(silverfish.clone());
        args.world.send_entity_status(
            silverfish.get_entity(),
            pumpkin_data::entity::EntityStatus::SilverfishMergeAnim,
            None,
        );
    }
}
