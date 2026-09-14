//! World adapters for the independently compared Java vibration state machine.
use pumpkin_world::chunk::io::Dirtiable;
use std::sync::{Arc, atomic::Ordering};
use pumpkin_data::{Block, BlockId, BlockStateId, entity::EntityType, tag::Taggable};
use pumpkin_util::math::{position::BlockPos, vector2::Vector2, vector3::Vector3};
use crate::{block::{entities::{BlockEntity, sculk_sensor::SculkSensorBlockEntity, calibrated_sculk_sensor::CalibratedSculkSensorBlockEntity}, blocks::redstone::sculk_sensor::SculkSensorBlock}, entity::EntityBase};
use super::{World, vibration::{Receiver, Source, events, vibration_listener::{self, Facts}, vibration_ticker::{Environment, Vibration}}};

fn receiver(be: &dyn BlockEntity) -> Option<(&Receiver, i32)> {
    if let Some(be) = be.as_any().downcast_ref::<SculkSensorBlockEntity>() { Some((&be.vibration, 8)) }
    else { be.as_any().downcast_ref::<CalibratedSculkSensorBlockEntity>().map(|be| (&be.vibration, 16)) }
}
impl World {
    pub(super) fn dispatch_vibration(&self, name: &str, position: Vector3<f64>, entity: Option<&dyn EntityBase>, affected: Option<BlockStateId>) {
        let Some(spec) = events::by_name(name) else { return; };
        let p = [position.x, position.y, position.z];
        let source_block = p.map(|v| v.floor() as i32);
        let dampens = entity.is_some_and(|entity| {
            entity.get_entity().entity_type == &EntityType::WARDEN || entity.get_item_entity().is_some_and(|item| {
                item.get_item_stack().lock().unwrap_or_else(std::sync::PoisonError::into_inner).item.is_tagged_with("minecraft:dampens_vibrations").unwrap_or(false)
            })
        });
        let source = Source { event: spec, entity: entity.map(|e| e.get_entity().entity_uuid), projectile_owner: entity.and_then(EntityBase::get_owner_id).and_then(|id| self.get_entity_by_id(id)).map(|e| e.get_entity().entity_uuid) };
        // Clone before callbacks: nested game events must never retain a DashMap shard lock.
        let mut listeners = Vec::new();
        for x in (source_block[0] - spec.radius) >> 4..=(source_block[0] + spec.radius) >> 4 {
            for z in (source_block[2] - spec.radius) >> 4..=(source_block[2] + spec.radius) >> 4 {
                if let Some(chunk) = self.block_entities.get(&Vector2::new(x, z)) {
                    listeners.extend(chunk.values().filter(|be| receiver(be.as_ref()).is_some()).cloned());
                }
            }
        }
        for be in listeners {
            let (receiver, radius) = receiver(be.as_ref()).unwrap();
            let pos = be.get_position();
            let d = [f64::from(pos.0.x) - f64::from(source_block[0]), f64::from(pos.0.y) - f64::from(source_block[1]), f64::from(pos.0.z) - f64::from(source_block[2])];
            if d[0]*d[0] + d[1]*d[1] + d[2]*d[2] > f64::from(radius*radius) { continue; }
            let destination = pos.to_centered_f64();
            let mut data = receiver.data.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            let distance = vibration_listener::admit(Facts {
                busy: data.current.is_some() || receiver.delivering.load(Ordering::Acquire), listenable: spec.listens[0], has_source: entity.is_some(),
                spectator: entity.is_some_and(EntityBase::is_spectator), sneaking: entity.is_some_and(|e| e.get_entity().is_sneaking()), ignore_sneaking: spec.ignore_sneaking,
                source_dampens: dampens, affected_dampens: affected.is_some_and(|state| Block::from_state_id(state).is_tagged_with("minecraft:dampens_vibrations").unwrap_or(false)),
                position: p, listener: Some([destination.x, destination.y, destination.z]),
            }, || SculkSensorBlock::can_receive(self, &pos, spec.name, spec.frequency, source_block),
            |p| self.get_block(&BlockPos(Vector3::new(p[0],p[1],p[2]))).is_tagged_with("minecraft:occludes_vibration_signals").unwrap_or(false), |_,_| {});
            if let Some(distance) = distance {
                data.add(Vibration { position:p, distance, frequency:spec.frequency, source:source.clone() }, self.get_world_age());
            }
        }
    }

    fn vibration_particle(&self, position: [f64; 3], target: BlockPos, travel: i32) -> bool {
        use pumpkin_protocol::java::client::play::CParticle;
        let mut extra = vec![0]; // PositionSourceType.BLOCK, followed by packed BlockPos and VarInt ticks.
        let packed = ((i64::from(target.0.x) & 0x3ffffff) << 38) | ((i64::from(target.0.z) & 0x3ffffff) << 12) | (i64::from(target.0.y) & 0xfff);
        extra.extend_from_slice(&packed.to_be_bytes());
        let mut ticks = travel as u32;
        loop { let byte = (ticks & 127) as u8; ticks >>= 7; extra.push(byte | if ticks != 0 {128} else {0}); if ticks == 0 {break;} }
        let position = Vector3::new(position[0],position[1],position[2]);
        let packet = CParticle::new(false, false, position, Vector3::new(0.0,0.0,0.0), 0.0, 1, (pumpkin_data::particle::Particle::Vibration.to_id() as i32).into(), &extra);
        let players = self.players.load();
        let nearby: Vec<_> = players.iter().filter(|player| {
            let p = player.get_entity().pos.load();
            let d = [p.x-position.x, p.y-position.y, p.z-position.z];
            d[0]*d[0]+d[1]*d[1]+d[2]*d[2] < 32.0*32.0
        }).collect();
        let delivered = !nearby.is_empty();
        let groups = Self::collect_java_recipients_by_version(nearby.into_iter());
        Self::broadcast_java_grouped(&packet, groups);
        delivered
    }
}

struct SensorEnvironment<'a> { world: &'a Arc<World>, pos: BlockPos, delivery: Option<(Vibration<Source>, f32)>, dirty: bool }
impl Environment<Source> for SensorEnvironment<'_> {
    fn position(&self) -> Option<[f64;3]> { let p=self.pos.to_centered_f64();Some([p.x,p.y,p.z]) }
    fn adjacent_required(&self) -> bool {true}
    fn ticking(&mut self,x:i32,z:i32)->bool {self.world.active_chunks.read().unwrap_or_else(std::sync::PoisonError::into_inner).contains(&Vector2::new(x,z))}
    fn loaded(&mut self,x:i32,z:i32)->bool {self.world.level.is_chunk_loaded(&Vector2::new(x,z))}
    fn particle(&mut self,p:[f64;3],travel:i32)->bool {self.world.vibration_particle(p,self.pos,travel)}
    fn receive(&mut self,v:&Vibration<Source>,_source:[i32;3],distance:f32){self.delivery=Some((v.clone(),distance));}
    fn changed(&mut self){self.dirty=true;}
}
impl Receiver {
    pub(crate) fn tick_sensor(&self, world:&Arc<World>, pos:BlockPos) {
        if !matches!(world.get_block(&pos).id, BlockId::SCULK_SENSOR | BlockId::CALIBRATED_SCULK_SENSOR) {return;}
        let mut environment=SensorEnvironment{world,pos,delivery:None,dirty:false};
        {
            let mut data=self.data.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            data.tick(world.get_world_age(),&mut environment);
            // Keep the listener busy while arrival callbacks recursively emit resonance events.
            if environment.delivery.is_some(){self.delivering.store(true,Ordering::Release);}
        }
        if let Some((vibration,distance))=environment.delivery {
            let source=vibration.source.entity.and_then(|id|world.get_entity_by_uuid(id).or_else(||world.players.load().iter().find(|p|p.get_entity().entity_uuid==id).map(|p|p.clone() as Arc<dyn EntityBase>)));
            SculkSensorBlock::trigger(world,&pos,vibration.frequency,distance,source.as_deref());
            self.delivering.store(false,Ordering::Release);
        }
        if environment.dirty {world.level.read_chunk_sync(&pos.chunk_position(),|chunk|{chunk.mark_dirty(true);});}
    }
}
