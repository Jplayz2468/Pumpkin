//! Warden-local Brain behavior phase order. Memories and sensors run before this pass.
//! Activity selection happens after it; a running behavior may outlive its activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activity {
    Emerge,
    Dig,
    Roar,
    Fight,
    Investigate,
    Sniff,
    Idle,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behavior {
    Swim,
    SetLook,
    Look,
    Move,
    Emerge,
    Dig,
    SetRoar,
    Investigate,
    Sniff,
    Roar,
    TrySniff,
    Idle,
    ValidateAttack,
    FightLook,
    Pursue,
    Sonic,
    Melee,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Start,
    Run,
}
/// All eligible start attempts precede the running pass. One-shot work happens
/// during Start. The callback ignores Run for a behavior that is not running.
pub fn behaviors(activity: Activity, mut apply: impl FnMut(Behavior, Phase)) {
    use Behavior::*;
    for b in [Swim, SetLook, Look, Move] {
        apply(b, Phase::Start);
    }
    let selected: &[Behavior] = match activity {
        Activity::Emerge => &[Emerge],
        Activity::Dig => &[Dig],
        Activity::Roar => &[Roar],
        Activity::Fight => &[ValidateAttack, FightLook, Pursue, Sonic, Melee],
        Activity::Investigate => &[SetRoar, Investigate],
        Activity::Sniff => &[SetRoar, Sniff],
        Activity::Idle => &[SetRoar, TrySniff, Idle],
    };
    for &b in selected {
        apply(b, Phase::Start);
    }
    for b in [Swim, Look, Move, Emerge, Dig, Sniff, Roar, Idle, Sonic] {
        apply(b, Phase::Run);
    }
}
