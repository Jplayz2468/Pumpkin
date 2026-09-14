//! Warden anger arithmetic, suspect priority and loaded/unloaded transitions.
//! The small ordered hash table preserves Java's observable UUID resolution order.
use std::cmp::Ordering;

trait JavaHash: Copy + Eq {
    fn java_hash(self) -> i32;
}
impl JavaHash for i32 {
    fn java_hash(self) -> i32 {
        self
    }
}
impl JavaHash for u128 {
    fn java_hash(self) -> i32 {
        let folded = (self >> 64) as u64 ^ self as u64;
        (folded >> 32) as i32 ^ folded as i32
    }
}
#[derive(Debug, Clone)]
struct JavaMap<K> {
    slots: Vec<Option<(K, i32)>>,
    size: usize,
    min_size: usize,
}
impl<K: JavaHash> JavaMap<K> {
    fn new(expected: usize) -> Self {
        let n = ((expected as f64 / 0.75).ceil() as usize)
            .next_power_of_two()
            .max(2);
        Self {
            slots: vec![None; n],
            size: 0,
            min_size: n,
        }
    }
    fn max_fill(&self) -> usize {
        ((self.slots.len() * 3).div_ceil(4)).min(self.slots.len() - 1)
    }
    fn index(&self, key: K) -> usize {
        let hash = (key.java_hash() as u32).wrapping_mul(0x9e3779b9);
        let mut i = ((hash ^ (hash >> 16)) as usize) & (self.slots.len() - 1);
        while self.slots[i].is_some_and(|(k, _)| k != key) {
            i = (i + 1) & (self.slots.len() - 1);
        }
        i
    }
    fn get(&self, key: K) -> Option<i32> {
        self.slots[self.index(key)].map(|(_, v)| v)
    }
    fn put(&mut self, key: K, value: i32) {
        let i = self.index(key);
        let fresh = self.slots[i].is_none();
        self.slots[i] = Some((key, value));
        if fresh {
            let old_size = self.size;
            self.size += 1;
            if old_size >= self.max_fill() {
                self.rehash(((self.size + 1) as f64 / 0.75).ceil() as usize);
            }
        }
    }
    fn rehash(&mut self, expected_slots: usize) {
        let entries = self.entries();
        self.slots = vec![None; expected_slots.next_power_of_two().max(2)];
        for (key, v) in entries {
            let i = self.index(key);
            self.slots[i] = Some((key, v));
        }
    }
    fn shift_keys(&mut self, mut gap: usize, mut wrapped: Option<&mut Vec<K>>) {
        let mask = self.slots.len() - 1;
        loop {
            let last = gap;
            gap = (gap + 1) & mask;
            while let Some((key, _)) = self.slots[gap] {
                let hash = (key.java_hash() as u32).wrapping_mul(0x9e3779b9);
                let home = ((hash ^ (hash >> 16)) as usize) & mask;
                if if last <= gap {
                    last >= home || home > gap
                } else {
                    last >= home && home > gap
                } {
                    break;
                }
                gap = (gap + 1) & mask;
            }
            if gap < last {
                if let (Some(keys), Some((key, _))) = (wrapped.as_mut(), self.slots[gap]) {
                    keys.push(key);
                }
            }
            self.slots[last] = self.slots[gap];
            if self.slots[gap].is_none() {
                break;
            }
        }
    }
    fn remove(&mut self, key: K, shrink: bool) -> Option<i32> {
        let gap = self.index(key);
        let (_, value) = self.slots[gap]?;
        self.size -= 1;
        self.shift_keys(gap, None);
        if shrink
            && self.slots.len() > self.min_size
            && self.slots.len() > 16
            && self.size < self.max_fill() / 4
        {
            self.rehash(self.slots.len() / 2);
        }
        Some(value)
    }
    /// Iterator removal preserves keys shifted across the table boundary. A
    /// snapshot of the original entries changes tied suspect order on reload.
    fn update(&mut self, mut visit: impl FnMut(K, i32) -> Option<i32>) {
        let mut pos = self.slots.len() as isize;
        let mut wrapped = Vec::new();
        for _ in 0..self.size {
            let index = loop {
                pos -= 1;
                if pos < 0 {
                    break self.index(wrapped[(-pos - 1) as usize]);
                }
                if self.slots[pos as usize].is_some() {
                    break pos as usize;
                }
            };
            let (key, value) = self.slots[index].unwrap();
            if let Some(value) = visit(key, value) {
                self.slots[index] = Some((key, value));
            } else if pos < 0 {
                self.remove(key, true);
            } else {
                self.size -= 1;
                self.shift_keys(index, Some(&mut wrapped));
            }
        }
    }
    fn entries(&self) -> Vec<(K, i32)> {
        self.slots.iter().rev().filter_map(|e| *e).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suspect {
    pub id: i32,
    pub uuid: u128,
    pub player: bool,
    pub living: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Killed,
    Discarded,
    Unloaded,
}
#[derive(Debug, Clone)]
pub struct AngerManagement {
    pub conversion_delay: i32,
    pub highest: i32,
    pub suspects: Vec<Suspect>,
    active: JavaMap<i32>,
    pending: JavaMap<u128>,
}
impl AngerManagement {
    pub fn new(delay: i32, saved: &[(u128, i32)]) -> Self {
        let mut pending = JavaMap::new(saved.len());
        for &(id, value) in saved {
            pending.put(id, value);
        }
        Self {
            conversion_delay: delay,
            highest: 0,
            suspects: vec![],
            active: JavaMap::new(16),
            pending,
        }
    }
    pub fn anger(&self, id: Option<i32>) -> i32 {
        id.map_or(self.highest, |id| self.active.get(id).unwrap_or(0))
    }
    pub fn saved(&self) -> Vec<(u128, i32)> {
        self.pending.entries()
    }
    fn sort(&mut self) {
        let mut highest = 0;
        self.suspects.sort_by(|a, b| {
            if a.id == b.id {
                return Ordering::Equal;
            }
            let x = self.active.get(a.id).unwrap_or(0);
            let y = self.active.get(b.id).unwrap_or(0);
            highest = highest.max(x.max(y));
            (y >= 80)
                .cmp(&(x >= 80))
                .then_with(|| b.player.cmp(&a.player))
                .then_with(|| y.cmp(&x))
        });
        self.highest = if self.suspects.len() == 1 {
            self.active.get(self.suspects[0].id).unwrap_or(0)
        } else {
            highest
        };
    }
    pub fn increase(&mut self, suspect: Suspect, amount: i32) -> i32 {
        let previous = self.active.get(suspect.id);
        let mut value = previous.unwrap_or(0).wrapping_add(amount).min(150);
        if previous.is_none() {
            value = value.wrapping_add(self.pending.remove(suspect.uuid, true).unwrap_or(0));
            self.suspects.push(suspect);
        }
        self.active.put(suspect.id, value);
        self.sort();
        value
    }
    pub fn clear(&mut self, id: i32) {
        self.active.remove(id, true);
        if let Some(i) = self.suspects.iter().position(|s| s.id == id) {
            self.suspects.remove(i);
        }
        self.sort();
    }
    pub fn active_entity(&self, mut eligible: impl FnMut(i32) -> bool) -> Option<i32> {
        self.suspects
            .iter()
            .find(|s| eligible(s.id))
            .filter(|s| s.living)
            .map(|s| s.id)
    }
    pub fn tick(
        &mut self,
        mut resolve: impl FnMut(u128) -> Option<Suspect>,
        mut status: impl FnMut(i32) -> (bool, Option<Removal>),
    ) {
        self.conversion_delay = self.conversion_delay.wrapping_sub(1);
        if self.conversion_delay <= 0 {
            self.pending.update(|uuid, anger| {
                if let Some(suspect) = resolve(uuid) {
                    self.active.put(suspect.id, anger);
                    self.suspects.push(suspect);
                    None
                } else {
                    Some(anger)
                }
            });
            self.conversion_delay = 2;
        }
        self.pending
            .update(|_, value| if value <= 1 { None } else { Some(value - 1) });
        self.active.update(|id, value| {
            let (valid, removed) = status(id);
            if value <= 1 || !valid || removed.is_some() {
                if let Some(i) = self.suspects.iter().position(|s| s.id == id) {
                    let suspect = self.suspects.remove(i);
                    if value > 1 && removed == Some(Removal::Unloaded) {
                        self.pending.put(suspect.uuid, value - 1);
                    }
                }
                None
            } else {
                Some(value - 1)
            }
        });
        self.sort();
    }
}
