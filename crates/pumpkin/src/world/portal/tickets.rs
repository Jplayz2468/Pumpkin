//! Portal destinations remain loaded for 300 ticks, refreshed by subsequent arrivals.
use crate::world::World;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_world::chunk_system::chunk_loading::ChunkLoading;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Default)]
pub struct PortalTickets(FxHashMap<Vector2<i32>, i64>);

impl PortalTickets {
    fn refresh(&mut self, pos: Vector2<i32>, now: i64) -> bool {
        self.0.insert(pos, now.saturating_add(300)).is_none()
    }
    fn expire(&mut self, now: i64) -> Vec<Vector2<i32>> {
        let mut expired = Vec::new();
        self.0.retain(|pos, deadline| {
            if now > *deadline {
                expired.push(*pos);
                false
            } else {
                true
            }
        });
        expired
    }
    fn entity_chunks(&self) -> FxHashSet<Vector2<i32>> {
        let mut chunks = FxHashSet::default();
        for center in self.0.keys() {
            for x in -1..=1 {
                for z in -1..=1 {
                    chunks.insert(Vector2::new(center.x + x, center.y + z));
                }
            }
        }
        chunks
    }
}

impl World {
    pub(crate) fn place_portal_ticket(&self, pos: Vector2<i32>) {
        let mut tickets = self
            .portal_tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if tickets.refresh(pos, self.get_world_age()) {
            let mut loading = self
                .level
                .chunk_loading
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            loading.add_ticket(pos, ChunkLoading::FULL_CHUNK_LEVEL - 3);
            loading.send_change();
        }
    }

    pub(crate) fn portal_ticking_chunks(&self) -> FxHashSet<Vector2<i32>> {
        let mut tickets = self
            .portal_tickets
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let expired = tickets.expire(self.get_world_age());
        if !expired.is_empty() {
            let mut loading = self
                .level
                .chunk_loading
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            for pos in expired {
                loading.remove_ticket(pos, ChunkLoading::FULL_CHUNK_LEVEL - 3);
            }
            loading.send_change();
        }
        tickets.entity_chunks()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portal_tickets_refresh_overlap_and_expire_after_the_last_tick() {
        let mut tickets = PortalTickets::default();
        let a = Vector2::new(-4, 7);
        let b = Vector2::new(-3, 7);
        assert!(tickets.refresh(a, 10));
        assert_eq!(tickets.entity_chunks().len(), 9);
        assert!(tickets.refresh(b, 20));
        assert_eq!(tickets.entity_chunks().len(), 12);
        assert!(!tickets.refresh(a, 100));
        assert!(tickets.expire(320).is_empty());
        assert_eq!(tickets.expire(321), vec![b]);
        assert_eq!(tickets.entity_chunks().len(), 9);
        assert!(tickets.expire(400).is_empty());
        assert_eq!(tickets.expire(401), vec![a]);
        assert!(tickets.entity_chunks().is_empty());
    }
}
