//! Registry-aware vibration save data; malformed optional events are discarded leniently.
use pumpkin_nbt::{compound::NbtCompound,tag::NbtTag};
#[derive(Clone)]
pub struct Event {pub kind:String,pub distance:f32,pub position:[f64;3],pub source:Option<[i32;4]>,pub owner:Option<[i32;4]>}
#[derive(Default)]
pub struct Data {pub current:Option<Event>,pub pending:Option<(Event,i64)>,pub delay:i32,pub reload:bool}
fn double(v:&NbtTag)->Option<f64>{Some(match v{NbtTag::Byte(v)=>f64::from(*v),NbtTag::Short(v)=>f64::from(*v),NbtTag::Int(v)=>f64::from(*v),NbtTag::Long(v)=>*v as f64,NbtTag::Float(v)=>f64::from(*v),NbtTag::Double(v)=>*v,_=>return None})}
fn long(v:&NbtTag)->Option<i64>{Some(match v{NbtTag::Byte(v)=>i64::from(*v),NbtTag::Short(v)=>i64::from(*v),NbtTag::Int(v)=>i64::from(*v),NbtTag::Long(v)=>*v,NbtTag::Float(v)=>*v as i64,NbtTag::Double(v)=>*v as i64,_=>return None})}
fn integer(v:&NbtTag)->Option<i32>{Some(match v{NbtTag::Long(v)=>*v as i32,NbtTag::Float(v)=>*v as i32,NbtTag::Double(v)=>*v as i32,_=>long(v)? as i32})}
fn values(v:&NbtTag)->Option<Vec<NbtTag>>{Some(match v{NbtTag::List(v)=>v.clone(),NbtTag::ByteArray(v)=>v.iter().map(|v|NbtTag::Byte(*v)).collect(),NbtTag::IntArray(v)=>v.iter().map(|v|NbtTag::Int(*v)).collect(),NbtTag::LongArray(v)=>v.iter().map(|v|NbtTag::Long(*v)).collect(),_=>return None})}
fn uuid(v:&NbtTag)->Option<[i32;4]>{let v=values(v)?;let numbers:Vec<i32>=v.iter().map(integer).collect::<Option<_>>()?;numbers.try_into().ok()}
fn event(v:&NbtTag,known:&impl Fn(&str)->bool)->Option<Event>{
 let NbtTag::Compound(c)=v else{return None};let mut kind=c.get_string("game_event")?.to_owned();if !kind.contains(':'){kind="minecraft:".to_owned()+&kind;}if !known(&kind){return None;}
 let distance=double(c.get("distance")?)? as f32;if distance.total_cmp(&0.0).is_lt() || distance.total_cmp(&f32::MAX).is_gt(){return None;}
 let position:Vec<f64>=values(c.get("pos")?)?.iter().map(double).collect::<Option<_>>()?;let position=position.try_into().ok()?;
 Some(Event{kind,distance,position,source:c.get("source").and_then(uuid),owner:c.get("projectile_owner").and_then(uuid)})
}
impl Data {
 pub fn read(tag:&NbtTag,known:impl Fn(&str)->bool)->Option<Self>{
  let NbtTag::Compound(c)=tag else{return None};let selector=c.get_compound("selector")?;let tick=long(selector.get("tick")?)?;
  let delay=match c.get("event_delay"){None=>0,Some(v)=>integer(v)?};if delay<0{return None;}
  Some(Self{current:c.get("event").and_then(|v|event(v,&known)),pending:selector.get("event").and_then(|v|event(v,&known)).map(|e|(e,tick)),delay,reload:true})
 }
 pub fn write(&self)->NbtCompound{
  fn encode(e:&Event)->NbtCompound{let mut c=NbtCompound::new();c.put_string("game_event",e.kind.clone());c.put_float("distance",e.distance);c.put_list("pos",e.position.map(|v|NbtTag::Double(if v==0.0{0.0}else{v})).to_vec());if let Some(id)=e.source{c.put("source",NbtTag::IntArray(id.to_vec()));}if let Some(id)=e.owner{c.put("projectile_owner",NbtTag::IntArray(id.to_vec()));}c}
  let mut c=NbtCompound::new();if let Some(e)=&self.current{c.put_compound("event",encode(e));}let mut selector=NbtCompound::new();selector.put_long("tick",self.pending.as_ref().map_or(-1,|(_,t)|*t));if let Some((e,_))=&self.pending{selector.put_compound("event",encode(e));}c.put_compound("selector",selector);c.put_int("event_delay",self.delay);c
 }
}
