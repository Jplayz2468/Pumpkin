//! SpawnUtil's triggered Warden attempt ordering. World/entity operations stay outside RNG locks.
pub trait SpawnAccess {
    type Mob;
    fn next_int(&mut self, bound: i32) -> i32;
    fn within_border(&mut self, pos: [i32; 3]) -> bool;
    fn find_floor(&mut self, pos: &mut [i32; 3]) -> bool;
    fn create(&mut self, pos: [i32; 3]) -> Option<Self::Mob>;
    fn check_rules(&mut self, mob: &Self::Mob) -> bool;
    fn check_obstruction(&mut self, mob: &Self::Mob) -> bool;
    fn discard(&mut self, mob: Self::Mob);
    fn add_and_play_ambient(&mut self, mob: Self::Mob);
}

pub fn try_spawn(access: &mut impl SpawnAccess, origin: [i32; 3]) -> bool {
    for _ in 0..20 {
        let dx = access.next_int(11) - 5;
        let dz = access.next_int(11) - 5;
        let mut pos = [
            origin[0].wrapping_add(dx),
            origin[1].wrapping_add(6),
            origin[2].wrapping_add(dz),
        ];
        if !access.within_border(pos) || !access.find_floor(&mut pos) {
            continue;
        }
        // The early type-AABB check is disabled by the shrieker call; the
        // finalized entity's normal rules and obstruction checks still run.
        if let Some(mob) = access.create(pos) {
            if access.check_rules(&mob) && access.check_obstruction(&mob) {
                access.add_and_play_ambient(mob);
                return true;
            }
            access.discard(mob);
        }
    }
    false
}
