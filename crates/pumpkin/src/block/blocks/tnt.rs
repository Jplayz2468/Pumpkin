use pumpkin_data::Block;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::TntLikeProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::sound::SoundCategory;
use pumpkin_data::translation;
use pumpkin_macros::pumpkin_block;
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::text::TextComponent;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

use super::redstone::block_receives_redstone_power;
use crate::block::registry::BlockActionResult;
use crate::block::{
    BlockBehaviour, BrokenArgs, ExplodeArgs, OnNeighborUpdateArgs, OnProjectileHitArgs, PlacedArgs,
    UseWithItemArgs,
};
use crate::entity::tnt::TNTEntity;
use crate::entity::{Entity, EntityBase};
use crate::world::World;

#[pumpkin_block("minecraft:tnt")]
pub struct TNTBlock;

const DEFAULT_FUSE: u32 = 80;
const DEFAULT_POWER: f32 = 4.0;

impl TNTBlock {
    /// Primes TNT at `location`. `primed_by_player` mirrors vanilla's
    /// `TntBlock#prime(Level, BlockPos, LivingEntity)` owner argument: it is only `true`
    /// when a player directly ignites the TNT (flint and steel / fire charge), matching
    /// `TntBlock.java`'s `useItemOn` (passes `player`) versus its redstone/`playerWillDestroy`
    /// call sites (pass no source). This is later used to decide whether the resulting
    /// explosion drops experience from broken ore (`BlockBehaviour.java:180`).
    pub fn prime(world: &Arc<World>, location: &BlockPos, primed_by_player: bool) -> bool {
        let primed = Self::prime_with_source(world, location, primed_by_player, None);
        if primed {
            world.set_block_state(location, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
        }
        primed
    }

    fn prime_with_source(
        world: &Arc<World>,
        location: &BlockPos,
        primed_by_player: bool,
        source: Option<&dyn EntityBase>,
    ) -> bool {
        if !world.level_info.load().game_rules.tnt_explodes {
            return false;
        }

        let mut event = crate::plugin::api::events::block::tnt_prime::TNTPrimeEvent::new(
            *location,
            "REDSTONE".to_string(),
        );
        if let Some(server) = world.server.upgrade() {
            server.plugin_manager.fire_blocking(&server, &mut event);
        }
        if event.cancelled {
            return false;
        }

        let spawn_pos = Vector3::new(
            location.0.x as f64 + 0.5,
            location.0.y as f64,
            location.0.z as f64 + 0.5,
        );
        let entity = Entity::new(world.clone(), spawn_pos, &EntityType::TNT);
        let mut prime_event =
            crate::plugin::api::events::entity::explosion_prime::ExplosionPrimeEvent::new(
                entity.entity_id,
                DEFAULT_POWER,
                false,
            );
        if let Some(server) = world.server.upgrade() {
            server
                .plugin_manager
                .fire_blocking(&server, &mut prime_event);
        }
        if prime_event.cancelled {
            return false;
        }

        let tnt = Arc::new(TNTEntity::new(
            entity,
            DEFAULT_POWER,
            DEFAULT_FUSE,
            primed_by_player,
        ));
        world.spawn_entity(tnt);
        world.play_sound(
            pumpkin_data::sound::Sound::EntityTntPrimed,
            SoundCategory::Blocks,
            &spawn_pos,
        );
        world.emit_game_event_from_entity(
            "minecraft:prime_fuse",
            location.to_centered_f64(),
            source,
            None,
        );
        true
    }
}

impl BlockBehaviour for TNTBlock {
    fn use_with_item(&self, args: UseWithItemArgs<'_>) -> BlockActionResult {
        let item_id = args.item_stack.item.id;
        if item_id != Item::FLINT_AND_STEEL.id && item_id != Item::FIRE_CHARGE.id {
            return BlockActionResult::PassToDefaultBlockAction;
        }

        if Self::prime_with_source(args.world, args.position, true, Some(args.player.as_ref())) {
            args.world
                .set_block_state(args.position, BlockStateId::AIR, BlockFlags::NOTIFY_ALL);
            if args.player.gamemode.load() != GameMode::Creative {
                if item_id == Item::FLINT_AND_STEEL.id {
                    let _ = args.item_stack.damage_item(1);
                } else {
                    args.item_stack.decrement(1);
                }
            }
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Used,
                item_id.into(),
                1,
            );
            BlockActionResult::Success
        } else if !args.world.level_info.load().game_rules.tnt_explodes {
            args.player.send_system_message_raw(
                &TextComponent::translate(translation::java::BLOCK_MINECRAFT_TNT_DISABLED, []),
                true,
            );
            BlockActionResult::Pass
        } else {
            BlockActionResult::Success
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.block != Block::from_state_id(args.old_state_id)
            && block_receives_redstone_power(args.world, args.position)
        {
            Self::prime(args.world, args.position, false);
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if args.world.get_block(args.position) == args.block
            && block_receives_redstone_power(args.world, args.position)
        {
            Self::prime(args.world, args.position, false);
        }
    }

    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() != GameMode::Creative {
            let props = TntLikeProperties::from_state_id(args.state.id);
            if props.r#unstable {
                // Vanilla's `playerWillDestroy` calls the no-source `prime` overload
                // (`TntBlock.java`'s public `prime(Level, BlockPos)`), so the breaking
                // player is NOT credited as the indirect source here either.
                Self::prime_with_source(args.world, args.position, false, None);
            }
        }
    }

    fn on_projectile_hit(&self, args: OnProjectileHitArgs<'_>) {
        if args.projectile.get_entity().is_on_fire()
            && crate::entity::projectile::may_interact(args.projectile, args.world, args.position)
        {
            let owner = args.projectile.get_projectile_owner();
            let source = owner
                .as_deref()
                .filter(|entity| entity.get_living_entity().is_some());
            if Self::prime_with_source(
                args.world,
                args.position,
                source.is_some_and(|entity| entity.get_player().is_some()),
                source,
            ) {
                args.world.set_block_state(
                    args.position,
                    BlockStateId::AIR,
                    BlockFlags::NOTIFY_ALL,
                );
            }
        }
    }

    fn explode(&self, args: ExplodeArgs<'_>) {
        if !args.world.level_info.load().game_rules.tnt_explodes {
            return;
        }
        let spawn_pos = Vector3::new(
            args.position.0.x as f64 + 0.5,
            args.position.0.y as f64,
            args.position.0.z as f64 + 0.5,
        );
        let entity = Entity::new(args.world.clone(), spawn_pos, &EntityType::TNT);
        let fuse = args.world.rand_bounded_i32((DEFAULT_FUSE / 4) as i32) as u32 + DEFAULT_FUSE / 8;
        // Vanilla propagates the triggering explosion's indirect source entity onto the
        // newly spawned PrimedTnt (`TntBlock.java` `wasExploded`), so a chain reaction
        // that started with a player-primed explosion keeps crediting that player.
        let tnt = Arc::new(TNTEntity::new(
            entity,
            DEFAULT_POWER,
            fuse,
            args.caused_by_player,
        ));
        args.world.spawn_entity(tnt);
    }

    fn should_drop_items_on_explosion(&self) -> bool {
        false
    }
}
