//! MobEffectInstance update and duration/hidden-effect transitions.
use pumpkin_data::potion::Effect;
#[derive(Clone)]
pub struct EffectInstance {
    pub effect: Effect,
    pub hidden: Option<Box<EffectInstance>>,
}
impl std::ops::Deref for EffectInstance {
    type Target = Effect;
    fn deref(&self) -> &Effect {
        &self.effect
    }
}
impl std::ops::DerefMut for EffectInstance {
    fn deref_mut(&mut self) -> &mut Effect {
        &mut self.effect
    }
}
impl EffectInstance {
    pub fn new(effect: Effect) -> Self {
        Self {
            effect,
            hidden: None,
        }
    }
    fn shorter_than(&self, other: &Effect) -> bool {
        self.duration != -1 && (self.duration < other.duration || other.duration == -1)
    }
    pub fn update(&mut self, incoming: &Effect) -> bool {
        let mut changed = false;
        if incoming.amplifier > self.amplifier {
            let shorter = incoming.duration != -1
                && (incoming.duration < self.duration || self.duration == -1);
            if shorter {
                self.hidden = Some(Box::new(Self {
                    effect: self.effect.clone(),
                    hidden: self.hidden.take(),
                }));
            }
            self.effect.amplifier = incoming.amplifier;
            self.effect.duration = incoming.duration;
            changed = true;
        } else if self.shorter_than(incoming) {
            if incoming.amplifier == self.amplifier {
                self.effect.duration = incoming.duration;
                changed = true;
            } else if let Some(hidden) = &mut self.hidden {
                hidden.update(incoming);
            } else {
                self.hidden = Some(Box::new(Self::new(incoming.clone())));
            }
        }
        if (!incoming.ambient && self.ambient) || changed {
            self.effect.ambient = incoming.ambient;
            changed = true;
        }
        if incoming.show_particles != self.show_particles {
            self.effect.show_particles = incoming.show_particles;
            changed = true;
        }
        if incoming.show_icon != self.show_icon {
            self.effect.show_icon = incoming.show_icon;
            changed = true;
        }
        changed
    }
    pub fn has_remaining_duration(&self) -> bool {
        self.duration == -1 || self.duration > 0
    }
    fn tick_down(&mut self) {
        if let Some(hidden) = &mut self.hidden {
            hidden.tick_down();
        }
        if self.duration != -1 && self.duration != 0 {
            self.effect.duration = self.duration.wrapping_sub(1);
        }
    }
    /// Called after the active effect's gameplay action. Returns alive and downgraded.
    pub fn tick_duration(&mut self) -> (bool, bool) {
        if !self.has_remaining_duration() {
            return (false, false);
        }
        self.tick_down();
        let mut downgraded = false;
        if self.duration == 0
            && let Some(hidden) = self.hidden.take()
        {
            let blend = self.effect.blend;
            self.effect = hidden.effect;
            self.effect.blend = blend;
            self.hidden = hidden.hidden;
            downgraded = true;
        }
        (self.has_remaining_duration(), downgraded)
    }
}
