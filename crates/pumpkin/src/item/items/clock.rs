use std::any::Any;

use crate::item::{ItemBehaviour, ItemMetadata};
use pumpkin_data::item::Item;

pub struct ClockItem;

impl ItemMetadata for ClockItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::CLOCK.id])
    }
}

// Items.java:1318 registers CLOCK as a plain `Item` with no subclass, and Item.java's
// base `use` (Item.java:189-...) only special-cases the Consumable / Equippable /
// BlocksAttacks components, none of which a clock has. Vanilla therefore plays no
// sound and has no special behaviour on right-click; this behaviour exists only for
// registration (item metadata / ids()) and intentionally does nothing on use.
impl ItemBehaviour for ClockItem {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
