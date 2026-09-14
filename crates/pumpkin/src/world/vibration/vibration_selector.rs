//! Candidate ordering for the vanilla vibration system.
#[derive(Clone,Copy)]
pub struct Candidate {pub id:i32,pub distance:f32,pub frequency:i32,pub tick:i64}
#[derive(Default)]
pub struct Selector {pending:Option<Candidate>}
impl Selector {
 pub fn add(&mut self,candidate:Candidate)->bool{
  let replace=match self.pending {None=>true,Some(old)=>candidate.tick==old.tick&&(candidate.distance<old.distance||(!(candidate.distance>old.distance)&&candidate.frequency>old.frequency))};
  if replace{self.pending=Some(candidate);} replace
 }
 pub fn chosen(&self,time:i64)->Option<i32>{self.pending.filter(|v|v.tick<time).map(|v|v.id)}
 pub fn pending_tick(&self)->Option<i64>{self.pending.map(|c|c.tick)}
 pub fn reset(&mut self){self.pending=None;}
}
