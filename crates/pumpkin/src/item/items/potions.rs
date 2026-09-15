use std::sync::Arc;

use crate::block::registry::BlockActionResult;
use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::player::Player;
use crate::entity::projectile::{
    lingering_potion::LingeringPotionEntity, splash_potion::SplashPotionEntity,
};
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockDirection};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

pub struct PotionItem;
pub struct SplashPotionItem;
pub struct LingeringPotionItem;

impl ItemMetadata for PotionItem {
    fn ids() -> Box<[u16]> {
        [Item::POTION.id].into()
    }
}

impl ItemMetadata for SplashPotionItem {
    fn ids() -> Box<[u16]> {
        [Item::SPLASH_POTION.id].into()
    }
}

impl ItemMetadata for LingeringPotionItem {
    fn ids() -> Box<[u16]> {
        [Item::LINGERING_POTION.id].into()
    }
}

const POWER: f32 = 0.5;
/// `ThrowablePotionItem.java:26` passes a `yOffset` of `-20.0F` into
/// `Projectile.spawnProjectileFromRotation`, which becomes the `roll` argument of
/// `ThrownItemEntity::set_velocity_from` (`Projectile.shootFromRotation` computes the Y
/// component of the throw direction from `xRot + yOffset`, not `xRot` alone). This is what
/// gives thrown potions their characteristic upward lob instead of flying dead level.
const THROW_PITCH_OFFSET: f32 = -20.0;

/// Returns `true` if `stack` is a potion with no custom effects whose base potion is plain
/// water. Mirrors `PotionContents.is(Potions.WATER)` (`PotionContents.java:79-81`), used by
/// `PotionItem.useOn` to decide whether the potion may be poured out to make mud.
fn is_plain_water_potion(stack: &ItemStack) -> bool {
    stack
        .get_data_component::<pumpkin_data::data_component_impl::PotionContentsImpl>()
        .is_some_and(|pc| {
            pc.potion_id == Some(pumpkin_data::potion::Potion::WATER.id as i32)
                && pc.custom_effects.is_empty()
        })
}

impl ItemBehaviour for PotionItem {
    fn normal_use(&self, _item: &Item, _player: &Player) {
        // Drinking is handled by the consumable flow in the server (active hand + consumption tick).
    }

    /// `PotionItem.useOn` (`PotionItem.java:34-70`): pouring a plain water potion onto a
    /// mud-convertible block (dirt, grass block, ...) turns that block into mud and empties
    /// the bottle. Vanilla refuses the interaction when the block's bottom face was clicked.
    #[expect(clippy::too_many_arguments)]
    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        if face == BlockDirection::Down
            || !block.has_tag(&tag::Block::MINECRAFT_CONVERTABLE_TO_MUD)
            || !is_plain_water_potion(item)
        {
            return BlockActionResult::Pass;
        }

        let world = player.world();

        // PotionItem.java:43: SoundEvents.GENERIC_SPLASH on SoundSource.BLOCKS.
        world.play_sound(
            Sound::EntityGenericSplash,
            pumpkin_data::sound::SoundCategory::Blocks,
            &location.to_f64(),
        );

        let mut glass_bottle = ItemStack::new(1, &Item::GLASS_BOTTLE);
        if item.item_count == 1 && player.gamemode.load() != pumpkin_util::GameMode::Creative {
            *item = glass_bottle;
        } else {
            item.decrement_unless_creative(player.gamemode.load(), 1);
            let was_added = player.inventory().insert_stack_anywhere(&mut glass_bottle);
            if !was_added && !glass_bottle.is_empty() {
                world.drop_stack(&player.position().to_block_pos(), glass_bottle);
            }
        }

        // PotionItem.java:48-60: 5 SPLASH particles above the clicked block.
        world.spawn_particles(
            Particle::Splash,
            Vector3::new(
                f64::from(location.0.x) + 0.5,
                f64::from(location.0.y) + 1.0,
                f64::from(location.0.z) + 0.5,
            ),
            5,
            Vector3::new(0.5, 0.0, 0.5),
            1.0,
        );

        // PotionItem.java:63: SoundEvents.BOTTLE_EMPTY on SoundSource.BLOCKS.
        world.play_sound(
            Sound::ItemBottleEmpty,
            pumpkin_data::sound::SoundCategory::Blocks,
            &location.to_f64(),
        );
        // PotionItem.java:64 also fires GameEvent.FLUID_PLACE here; Pumpkin has no
        // sculk-vibration/game-event system to emit that into yet.
        world.set_block_state(&location, Block::MUD.default_state.id, BlockFlags::NOTIFY_ALL);

        BlockActionResult::Success
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ItemBehaviour for SplashPotionItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let position = player.position();
        let world = player.world();
        // SplashPotionItem.use (SplashPotionItem.java:20-33): entity.splash_potion.throw on
        // the PLAYERS category, volume 0.5 and a pitch randomized the same way as every
        // other throwable (0.4F / (nextFloat() * 0.4F + 0.8F)).
        world.play_sound_fine(
            Sound::EntitySplashPotionThrow,
            pumpkin_data::sound::SoundCategory::Players,
            &position,
            0.5,
            0.4 / (rand::random::<f32>() * 0.4 + 0.8),
        );
        let entity = Entity::new(world.clone(), position, &EntityType::SPLASH_POTION);
        let splash = SplashPotionEntity::new_shot(entity, player.get_entity());

        // Copy the held item stack data into the projectile
        let main_s = player.inventory.held_item();
        let mut used_main = true;
        let mut stack = (!main_s.is_empty()
            && main_s.item.id == pumpkin_data::item::Item::SPLASH_POTION.id)
            .then_some(main_s);
        if stack.is_none() {
            let off_s = player.inventory.off_hand_item();
            if !off_s.is_empty() && off_s.item.id == pumpkin_data::item::Item::SPLASH_POTION.id {
                stack = Some(off_s);
                used_main = false;
            }
        }
        let stack = stack.unwrap_or_else(|| ItemStack::EMPTY.clone());
        splash.set_item_stack(stack);

        let (yaw, pitch) = player.rotation();
        splash
            .thrown
            .set_velocity_from(pitch, yaw, THROW_PITCH_OFFSET, POWER, 1.0);

        world.spawn_entity(Arc::new(splash));

        // Decrement the used stack (clear)
        if used_main {
            let mut s = player.inventory.held_item();
            s.decrement_unless_creative(player.gamemode.load(), 1);
            player.inventory.set_held_item(s);
        } else {
            let mut s = player.inventory.off_hand_item();
            s.decrement_unless_creative(player.gamemode.load(), 1);
            player
                .inventory
                .set_stack_in_hand(pumpkin_util::Hand::Left, s);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ItemBehaviour for LingeringPotionItem {
    fn normal_use(&self, _item: &Item, player: &Player) {
        let position = player.position();
        let world = player.world();
        // LingeringPotionItem.use (LingeringPotionItem.java:20-33): entity.lingering_potion.throw
        // on the NEUTRAL category, volume 0.5 and the same randomized pitch as every other
        // throwable.
        world.play_sound_fine(
            Sound::EntityLingeringPotionThrow,
            pumpkin_data::sound::SoundCategory::Neutral,
            &position,
            0.5,
            0.4 / (rand::random::<f32>() * 0.4 + 0.8),
        );
        let entity = Entity::new(world.clone(), position, &EntityType::LINGERING_POTION);
        let ling = LingeringPotionEntity::new_shot(entity, player.get_entity());

        // Copy the held item stack data into the projectile
        let main_s = player.inventory.held_item();
        let mut used_main = true;
        let mut stack = (!main_s.is_empty()
            && main_s.item.id == pumpkin_data::item::Item::LINGERING_POTION.id)
            .then_some(main_s);
        if stack.is_none() {
            let off_s = player.inventory.off_hand_item();
            if !off_s.is_empty() && off_s.item.id == pumpkin_data::item::Item::LINGERING_POTION.id {
                stack = Some(off_s);
                used_main = false;
            }
        }
        let stack = stack.unwrap_or_else(|| ItemStack::EMPTY.clone());
        ling.set_item_stack(stack);

        let (yaw, pitch) = player.rotation();
        ling.thrown
            .set_velocity_from(pitch, yaw, THROW_PITCH_OFFSET, POWER, 1.0);

        world.spawn_entity(Arc::new(ling));

        // Decrement the used stack (clear)
        if used_main {
            let mut s = player.inventory.held_item();
            s.decrement_unless_creative(player.gamemode.load(), 1);
            player.inventory.set_held_item(s);
        } else {
            let mut s = player.inventory.off_hand_item();
            s.decrement_unless_creative(player.gamemode.load(), 1);
            player
                .inventory
                .set_stack_in_hand(pumpkin_util::Hand::Left, s);
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
