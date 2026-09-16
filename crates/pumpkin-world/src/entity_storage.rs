//! Snapshot entity records before asynchronous I/O, and persist submissions in order.
use crate::{
    chunk::{ChunkEntityData, io::FileIO},
    level::{EntitySaver, LevelFolder, SyncEntityChunk},
};
use pumpkin_util::math::vector2::Vector2;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, atomic::AtomicBool},
};
use tokio::sync::oneshot;
use tokio_util::task::TaskTracker;

type Job = (
    Vec<(Vector2<i32>, SyncEntityChunk)>,
    oneshot::Sender<Result<(), String>>,
);
#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    running: bool,
}
#[derive(Default)]
pub(crate) struct EntityWrites(Mutex<Queue>);

impl EntityWrites {
    pub fn enqueue(
        self: &Arc<Self>,
        chunks: Vec<(Vector2<i32>, SyncEntityChunk)>,
        saver: Arc<EntitySaver>,
        folder: Arc<LevelFolder>,
        tasks: &TaskTracker,
    ) -> oneshot::Receiver<Result<(), String>> {
        let (tx, rx) = oneshot::channel();
        let mut queue = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Capture under the submission lock: a later job cannot precede an older
        // snapshot, even if their caller tasks are scheduled in a different order.
        let snapshots = chunks
            .into_iter()
            .map(|(pos, chunk)| {
                let data = chunk
                    .data
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                (
                    pos,
                    Arc::new(ChunkEntityData {
                        x: pos.x,
                        z: pos.y,
                        data: Mutex::new(data),
                        dirty: AtomicBool::new(true),
                    }),
                )
            })
            .collect();
        queue.jobs.push_back((snapshots, tx));
        if !queue.running {
            queue.running = true;
            let state = self.clone();
            tasks.spawn(async move {
                loop {
                    let job = {
                        let mut queue = state
                            .0
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        match queue.jobs.pop_front() {
                            Some(job) => job,
                            None => {
                                queue.running = false;
                                break;
                            }
                        }
                    };
                    let result = saver.save_chunks(&folder, job.0).await;
                    if let Err(error) = &result {
                        tracing::error!("Failed writing entity snapshot: {error}");
                    }
                    let _ = job.1.send(result.map_err(|error| error.to_string()));
                }
            });
        }
        rx
    }
}
