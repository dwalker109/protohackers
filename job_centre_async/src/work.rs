use std::{
    collections::{BinaryHeap, HashMap},
    sync::atomic::AtomicUsize,
};

use serde_json::Value;
use tokio::sync::{broadcast, oneshot};

#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Job {
    id: usize,
    queue: String,
    job: Value,
    pri: usize,
}

impl From<Job> for crate::res::Response {
    fn from(job: Job) -> Self {
        crate::res::Response::Get {
            status: crate::res::ResponseStatus::Ok,
            id: job.id,
            queue: job.queue,
            pri: job.pri,
            job: job.job,
        }
    }
}

impl Ord for Job {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.pri.cmp(&other.pri)
    }
}

impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

pub struct JobQueues {
    queues: HashMap<String, BinaryHeap<Job>>,
    next_id: AtomicUsize,
    broadcaster: broadcast::Sender<String>,
}

impl JobQueues {
    pub fn new() -> Self {
        Self {
            queues: HashMap::new(),
            next_id: AtomicUsize::new(0),
            broadcaster: broadcast::channel::<String>(32).0,
        }
    }

    pub fn subscriber(&self) -> broadcast::Receiver<String> {
        self.broadcaster.subscribe()
    }

    pub fn add(&mut self, queue: String, job: Value, pri: usize) -> usize {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        self.queues
            .entry(queue.clone())
            .or_insert(BinaryHeap::new())
            .push(Job {
                id,
                queue: queue.clone(),
                job,
                pri,
            });

        self.broadcaster.send(queue).ok();

        id
    }

    fn restore(&mut self, job: Job) {
        let queue = job.queue.clone();

        self.queues
            .entry(queue.clone())
            .or_insert(BinaryHeap::new())
            .push(job);

        self.broadcaster.send(queue).ok();
    }

    pub fn next_best(&mut self, candidates: &[String]) -> Option<Job> {
        self.queues
            .iter_mut()
            .filter(|(name, queue)| candidates.contains(name) && !queue.is_empty())
            .max_by(|l, r| l.1.peek().unwrap().pri.cmp(&r.1.peek().unwrap().pri))
            .and_then(|(_, queue)| queue.pop())
    }

    pub fn del(&mut self, id: usize) -> Option<()> {
        let target_queue = self.queues.iter().find_map(|(name, queue)| {
            queue
                .iter()
                .any(|job| job.id == id)
                .then_some(name.to_owned())
        });

        match target_queue {
            Some(name) => {
                self.queues
                    .get_mut(&name)
                    .unwrap()
                    .retain(|job| job.id != id);
                Some(())
            }
            _ => None,
        }
    }
}

struct InFlight {
    job: Job,
    client_id: usize,
    del_sender: oneshot::Sender<()>,
}

pub struct InFlightQueue(HashMap<usize, InFlight>);

impl InFlightQueue {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn add(&mut self, job: Job, client_id: usize) -> oneshot::Receiver<()> {
        let (del_sender, del_receiver) = oneshot::channel::<()>();
        self.0.insert(
            job.id,
            InFlight {
                job,
                del_sender,
                client_id,
            },
        );

        del_receiver
    }

    pub fn del(&mut self, id: usize) -> Option<()> {
        self.0.remove(&id).map(|in_flight| {
            in_flight.del_sender.send(()).ok();
        })
    }

    pub fn abort(
        &mut self,
        queues: &mut JobQueues,
        job_id: usize,
        client_id: usize,
    ) -> Result<Option<()>, ()> {
        if self
            .0
            .get(&job_id)
            .is_some_and(|in_flight| in_flight.client_id != client_id)
        {
            return Err(());
        }

        if let Some(in_flight) = self.0.remove(&job_id) {
            queues.restore(in_flight.job);

            return Ok(Some(()));
        }

        Ok(None)
    }

    pub fn cleanup(&mut self, queues: &mut JobQueues, client_id: usize) {
        let job_ids = self
            .0
            .iter()
            .filter_map(|(id, in_flight)| (in_flight.client_id == client_id).then_some(*id))
            .collect::<Vec<_>>();

        for job_id in job_ids.iter() {
            self.abort(queues, *job_id, client_id).ok();
        }
    }
}
