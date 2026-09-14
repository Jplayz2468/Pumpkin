//! Java vibration travel, reload and arrival state machine. World effects are explicit boundaries.
use super::vibration_selector::{Candidate,Selector};
#[derive(Clone)]
pub struct Vibration<T=()> {pub position:[f64;3],pub distance:f32,pub frequency:i32,pub source:T}
pub trait Environment<T> {
 fn position(&self)->Option<[f64;3]>;
 fn adjacent_required(&self)->bool;
 fn ticking(&mut self,x:i32,z:i32)->bool;
 fn loaded(&mut self,x:i32,z:i32)->bool;
 fn particle(&mut self,position:[f64;3],travel:i32)->bool;
 fn receive(&mut self,vibration:&Vibration<T>,source:[i32;3],distance:f32);
 fn changed(&mut self);
 fn travel_time(&self,distance:f32)->i32 {distance.floor() as i32}
}
pub struct Data<T=()> {
 pub current:Option<Vibration<T>>,pub travel:i32,pub reload:bool,
 selector:Selector,pending:Option<Vibration<T>>,
}
impl<T> Default for Data<T>{fn default()->Self{Self{current:None,travel:0,reload:false,selector:Selector::default(),pending:None}}}
impl<T> Data<T> {
 pub fn pending(&self)->Option<(&Vibration<T>,i64)>{self.pending.as_ref().zip(self.selector.pending_tick())}
 pub fn from_saved(current:Option<Vibration<T>>,pending:Option<(Vibration<T>,i64)>,travel:i32,reload:bool)->Self{
  let mut data=Self::default();data.current=current;data.travel=travel;data.reload=reload;
  if let Some((event,tick))=pending{data.add(event,tick);}data
 }

 pub fn add(&mut self,vibration:Vibration<T>,tick:i64){
  if self.selector.add(Candidate{id:0,distance:vibration.distance,frequency:vibration.frequency,tick}){self.pending=Some(vibration);}
 }
 pub fn tick(&mut self,time:i64,env:&mut impl Environment<T>){
  if self.current.is_none() && self.selector.chosen(time).is_some(){
   self.current=self.pending.take();let event=self.current.as_ref().unwrap();self.travel=env.travel_time(event.distance);env.particle(event.position,self.travel);env.changed();self.selector.reset();
  }
  let Some(event)=self.current.as_ref() else{return};
  let mut changed=self.travel>0;
  if self.reload {
   let destination=env.position().unwrap_or(event.position);let total=env.travel_time(event.distance);let fraction=1.0-f64::from(self.travel)/f64::from(total);
   let p=std::array::from_fn(|i|event.position[i]+fraction*(destination[i]-event.position[i]));
   if env.particle(p,self.travel){self.reload=false;}
  }
  self.travel=self.travel.wrapping_sub(1).max(0);
  if self.travel<=0 {
   let source=event.position.map(|v|v.floor() as i32);let destination=env.position().map(|p|p.map(|v|v.floor() as i32)).unwrap_or(source);
   let mut ready=true;
   if env.adjacent_required(){
    let cx=destination[0]>>4;let cz=destination[2]>>4;
    'chunks:for x in cx-1..=cx+1{for z in cz-1..=cz+1{if !env.ticking(x,z)||!env.loaded(x,z){ready=false;break 'chunks;}}}
   }
   changed=ready;
   if ready {
    let d:[f64;3]=std::array::from_fn(|i|f64::from(source[i])-f64::from(destination[i]));let distance=(d[0]*d[0]+d[1]*d[1]+d[2]*d[2]).sqrt() as f32;
    env.receive(event,source,distance);self.current=None;
   }
  }
  if changed{env.changed();}
 }
}
