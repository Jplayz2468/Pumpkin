//! Shared controlling-passenger rules, separate from each vehicle's riding AI.
#[derive(Clone, Copy, Debug)]
pub enum Control {
    None,
    Boat,
    Mob,
    Saddled,
    Steered,
    Harness,
}
#[derive(Default)]
pub struct Facts {
    pub no_ai: bool,
    pub saddled: bool,
    pub harness: bool,
    pub still_timeout: bool,
    pub player: bool,
    pub living: bool,
    pub mob: bool,
    pub can_control: bool,
    pub steering_item: bool,
}

pub fn accepts(control: Control, f: Facts) -> bool {
    let mob_fallback = !f.no_ai && f.mob && f.can_control;
    match control {
        Control::None => false,
        Control::Boat => f.living,
        Control::Mob => mob_fallback,
        Control::Saddled => (f.saddled && f.player) || mob_fallback,
        Control::Steered => (f.saddled && f.player && f.steering_item) || mob_fallback,
        Control::Harness => (f.harness && !f.still_timeout && f.player) || mob_fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn player_controls_require_vehicle_equipment_but_boats_accept_living_riders() {
        let player = || Facts {
            player: true,
            living: true,
            can_control: true,
            ..Facts::default()
        };
        assert!(!accepts(Control::None, player()));
        assert!(!accepts(Control::Mob, player()));
        assert!(accepts(Control::Boat, player()));
        assert!(!accepts(Control::Saddled, player()));
        assert!(accepts(
            Control::Saddled,
            Facts {
                saddled: true,
                no_ai: true,
                ..player()
            }
        ));
        assert!(!accepts(
            Control::Steered,
            Facts {
                saddled: true,
                ..player()
            }
        ));
        assert!(accepts(
            Control::Steered,
            Facts {
                saddled: true,
                steering_item: true,
                ..player()
            }
        ));
        assert!(accepts(
            Control::Harness,
            Facts {
                harness: true,
                ..player()
            }
        ));
        assert!(!accepts(
            Control::Harness,
            Facts {
                harness: true,
                still_timeout: true,
                ..player()
            }
        ));
    }
    #[test]
    fn special_vehicles_retain_the_mob_controller_fallback() {
        for control in [
            Control::Mob,
            Control::Saddled,
            Control::Steered,
            Control::Harness,
        ] {
            let mob = || Facts {
                mob: true,
                living: true,
                can_control: true,
                ..Facts::default()
            };
            assert!(accepts(control, mob()));
            assert!(!accepts(
                control,
                Facts {
                    no_ai: true,
                    ..mob()
                }
            ));
            assert!(!accepts(
                control,
                Facts {
                    can_control: false,
                    ..mob()
                }
            ));
        }
        assert!(accepts(
            Control::Boat,
            Facts {
                living: true,
                ..Facts::default()
            }
        ));
        assert!(!accepts(Control::Boat, Facts::default()));
    }
}
