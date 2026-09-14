//! Differential probe of the production queue, used by comparison/redstone/run_ticks.py.
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::{ScheduledTick, TickPriority, scheduler::ChunkTickScheduler};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Input {
    #[serde(default)]
    at: i32,
    delay: i32,
    priority: i32,
    x: i32,
    r#type: String,
}
#[derive(Deserialize)]
struct Case {
    id: String,
    saved: Vec<Input>,
    schedule: Vec<Input>,
    until: i32,
}
fn tick(input: &Input) -> ScheduledTick<&'static u8> {
    ScheduledTick {
        delay: input
            .delay
            .try_into()
            .expect("delay outside production range"),
        priority: TickPriority::try_from(input.priority).unwrap(),
        position: BlockPos::new(input.x, 64, 0),
        value: match input.r#type.as_str() {
            "wire" => &0,
            "repeater" => &1,
            _ => panic!("unknown type"),
        },
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let cases: Vec<Case> = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let mut results = Vec::new();
    for case in cases {
        let queue: ChunkTickScheduler<&u8> = case.saved.iter().map(tick).collect();
        let mut order = 0;
        let mut trace = Vec::new();
        for now in 0..=case.until {
            for input in case.schedule.iter().filter(|input| input.at == now) {
                queue.schedule_tick(&tick(input), order);
                order += 1;
            }
            let mut due = queue.step_tick();
            due.sort_unstable(); // Production level collector sorts by priority/sub-order.
            for event in due {
                trace.push(json!({"tick":now,"x":event.position.0.x,"type":if *event.value == 0 { "wire" } else { "repeater" },"priority":event.priority as i32,"order":event.sub_tick_order}));
            }
        }
        results.push(json!({"id":case.id,"trace":trace}));
    }
    std::fs::write(
        &args[2],
        serde_json::to_string_pretty(&results).unwrap() + "\n",
    )
    .unwrap();
}
