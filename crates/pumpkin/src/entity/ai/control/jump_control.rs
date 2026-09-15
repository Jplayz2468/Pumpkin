//! Vanilla's `JumpControl`, ported but deliberately NOT wired in.
//!
//! In vanilla, `Mob.serverAiStep` ticks this after the move and look controls, and
//! everything that wants a mob to jump goes through `getJumpControl().jump()`. `tick`
//! then writes the flag straight onto the mob -- including writing `false` on every tick
//! nobody asked for a jump.
//!
//! This fork took the other route: the pathfinder, move control, swim goal, powder-snow
//! goal and slime all set `living_entity.jumping` directly. Ticking this control on top
//! of that would clear each of those writes within the same tick and stop mobs jumping
//! altogether. So it stays unticked until those call sites are converted to route through
//! it; adding it to the tick loop "for parity" is a regression, not a fix.

use crate::entity::ai::control::Control;
use crate::entity::mob::Mob;
use std::sync::atomic::Ordering;

#[derive(Default)]
pub struct JumpControl {
    jump: bool,
}

impl Control for JumpControl {}

impl JumpControl {
    pub const fn jump(&mut self) {
        self.jump = true;
    }

    pub fn tick(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .living_entity
            .jumping
            .store(self.jump, Ordering::SeqCst);
        self.jump = false;
    }
}
