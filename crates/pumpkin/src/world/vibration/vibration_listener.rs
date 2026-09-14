//! Java vibration admission and six-ray block-state occlusion.
use super::block_ray;
pub struct Facts {
 pub busy:bool,pub listenable:bool,pub has_source:bool,pub spectator:bool,pub sneaking:bool,pub ignore_sneaking:bool,pub source_dampens:bool,pub affected_dampens:bool,
 pub position:[f64;3],pub listener:Option<[f64;3]>,
}
pub fn admit(f:Facts,mut can_receive:impl FnMut()->bool,mut occludes:impl FnMut([i32;3])->bool,mut ray:impl FnMut([f64;3],[f64;3]))->Option<f32>{
 if f.busy || !f.listenable{return None;}
 if f.has_source && (f.spectator || (f.sneaking && f.ignore_sneaking) || f.source_dampens){return None;}
 if f.affected_dampens{return None;}
 let listener=f.listener?;
 if !can_receive(){return None;}
 let from=f.position.map(|v|v.floor()+0.5);let to=listener.map(|v|v.floor()+0.5);
 let mut blocked=true;
 for direction in [[0,-1,0],[0,1,0],[0,0,-1],[0,0,1],[-1,0,0],[1,0,0]]{
  let start=std::array::from_fn(|i|from[i]+f64::from(direction[i])*f64::from(1.0e-5_f32));
  ray(start,to);
  if !block_ray::traverse(start,to,&mut occludes){blocked=false;break;}
 }
 if blocked{return None;}
 let d:[f64;3]=std::array::from_fn(|i|f.position[i]-listener[i]);Some((d[0]*d[0]+d[1]*d[1]+d[2]*d[2]).sqrt() as f32)
}
