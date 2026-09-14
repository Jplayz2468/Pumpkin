//! Shared Java vibration state, admission, geometry and persistent event attribution.
use std::sync::{Mutex, atomic::AtomicBool};
use pumpkin_nbt::{compound::NbtCompound,tag::NbtTag};
use uuid::Uuid;

pub(crate) mod block_ray;
pub(crate) mod events;
pub(crate) mod vibration_listener;
pub(crate) mod vibration_nbt;
pub(crate) mod vibration_selector;
pub(crate) mod vibration_ticker;

#[derive(Clone)]
pub struct Source {
    pub event: &'static events::Spec,
    pub entity: Option<Uuid>,
    pub projectile_owner: Option<Uuid>,
}
pub struct Receiver {
    pub data: Mutex<vibration_ticker::Data<Source>>,
    pub delivering: AtomicBool,
}
impl Default for Receiver {fn default()->Self{Self{data:Mutex::new(vibration_ticker::Data::default()),delivering:AtomicBool::new(false)}}}
fn uuid(value:[i32;4])->Uuid {
    let bytes:[u8;16]=std::array::from_fn(|i|value[i/4].to_be_bytes()[i%4]);Uuid::from_bytes(bytes)
}
fn ints(value:Uuid)->[i32;4] {
    let bytes=value.as_bytes();std::array::from_fn(|i|i32::from_be_bytes(bytes[i*4..i*4+4].try_into().unwrap()))
}
fn event(value:vibration_nbt::Event)->vibration_ticker::Vibration<Source>{
    let spec=events::by_name(&value.kind).unwrap();
    vibration_ticker::Vibration{position:value.position,distance:value.distance,frequency:spec.frequency,source:Source{event:spec,entity:value.source.map(uuid),projectile_owner:value.owner.map(uuid)}}
}
fn saved(value:&vibration_ticker::Vibration<Source>)->vibration_nbt::Event {
    vibration_nbt::Event{kind:format!("minecraft:{}",value.source.event.name),distance:value.distance,position:value.position,source:value.source.entity.map(ints),owner:value.source.projectile_owner.map(ints)}
}
impl Receiver {
    pub fn read(nbt:&NbtCompound)->Self {
        let parsed=nbt.get("listener").and_then(|v|vibration_nbt::Data::read(v,|s|events::by_name(s).is_some())).unwrap_or_default();
        Self{data:Mutex::new(vibration_ticker::Data::from_saved(parsed.current.map(event),parsed.pending.map(|(v,t)|(event(v),t)),parsed.delay,parsed.reload)),delivering:AtomicBool::new(false)}
    }
    pub fn write(&self,nbt:&mut NbtCompound){
        let data=self.data.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let saved=vibration_nbt::Data{current:data.current.as_ref().map(saved),pending:data.pending().map(|(v,t)|(saved(v),t)),delay:data.travel,reload:data.reload};
        nbt.put("listener",NbtTag::Compound(saved.write()));
    }
}
