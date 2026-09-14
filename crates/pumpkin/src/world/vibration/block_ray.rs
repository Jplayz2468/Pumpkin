//! Java BlockGetter.traverseBlocks; checks voxel states in the same order.
pub fn traverse(from:[f64;3],to:[f64;3],mut hit:impl FnMut([i32;3])->bool)->bool {
 if from.map(f64::to_bits)==to.map(f64::to_bits){return false;}
 let end:[f64;3]=std::array::from_fn(|i|to[i]+(-1.0e-7)*(from[i]-to[i]));
 let start:[f64;3]=std::array::from_fn(|i|from[i]+(-1.0e-7)*(to[i]-from[i]));
 let mut p=start.map(|v|v.floor() as i32);
 if hit(p){return true;}
 let delta:[f64;3]=std::array::from_fn(|i|end[i]-start[i]);
 let sign=delta.map(|v|if v==0.0{0}else if v>0.0{1}else{-1});
 let interval:[f64;3]=std::array::from_fn(|i|if sign[i]==0{f64::MAX}else{f64::from(sign[i])/delta[i]});
 let mut next:[f64;3]=std::array::from_fn(|i|{let fraction=start[i]-start[i].floor();interval[i]*if sign[i]>0{1.0-fraction}else{fraction}});
 while next.iter().any(|v|*v<=1.0){
  let axis=if next[0]<next[1]{if next[0]<next[2]{0}else{2}}else if next[1]<next[2]{1}else{2};
  p[axis]=p[axis].wrapping_add(sign[axis]);next[axis]+=interval[axis];if hit(p){return true;}
 }
 false
}
