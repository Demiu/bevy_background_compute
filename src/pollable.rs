/// Yoinked from https://github.com/bevyengine/bevy/pull/4102
// TODO replace this with bevy's PollableTask once it's merge in
use std::future::Future;

use async_channel::{bounded, Receiver, TryRecvError};
use bevy_tasks::{Task, TaskPool};

/// Extension trait to add spawn_pollable to TaskPool
pub(crate) trait SpawnPollableExt {
    fn spawn_pollable<F, T>(&self, future: F) -> PollableTask<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static;
}

/// Wrapper around a Task that allow for polling
/// Yoinked from https://github.com/bevyengine/bevy/pull/4102, TODO replace with bevy's version once this gets merged
pub(crate) struct PollableTask<T> {
    receiver: Receiver<T>,
    _task: Task<()>,
}

impl<T> PollableTask<T> {
    pub(crate) fn poll(&self) -> Option<T> {
        match self.receiver.try_recv() {
            Ok(value) => Some(value),
            Err(error) => match error {
                TryRecvError::Empty => None,
                TryRecvError::Closed => panic!("PoolableTask couldn't receive"),
            },
        }
    }
}

impl SpawnPollableExt for TaskPool {
    fn spawn_pollable<F, T>(&self, future: F) -> PollableTask<T>
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        let (sender, receiver) = bounded(1);
        let task = self.spawn(async move {
            let result = future.await;
            match sender.send(result).await {
                Ok(_) => {}
                Err(error) => panic! {"Sending result of a task failed: {error}"},
            }
        });
        PollableTask {
            receiver,
            _task: task,
        }
    }
}
