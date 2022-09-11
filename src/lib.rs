use std::{future::Future, marker::PhantomData, sync::Mutex};

use bevy_app::{App, Plugin};
use bevy_ecs::{
    event::EventWriter,
    schedule::{ParallelSystemDescriptorCoercion, SystemLabel},
    system::{Command, Commands, ResMut},
    world::{Mut, World},
};
use bevy_tasks::AsyncComputeTaskPool;
use pollable::{PollableTask, SpawnPollableExt};

mod pollable;

/// Extension trait to add compute_in_background to Commands
pub trait ComputeInBackgroundCommandExt {
    fn compute_in_background<F, T>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + Sync + 'static;
}

/// The plugin to make a type computable in background
pub struct BackgroundComputePlugin<T>(PhantomData<fn() -> T>);

/// The label for the internal task checking system
#[derive(SystemLabel)]
#[system_label(ignore_fields)]
pub struct BackgroundComputeCheck<T>(PhantomData<T>);

// TODO remove Mutex when possible, for now required to make this Sync (for Command trait)
// https://github.com/bevyengine/bevy/pull/5871
/// Command struct for holding a future
struct ComputeInBackground<F, T>(Mutex<F>)
where
    F: Future<Output = T> + Send + 'static,
    T: Send + Sync + 'static;

/// Resource for holding background tasks to check for completion
struct BackgroundTasks<T> {
    tasks: Vec<PollableTask<T>>,
}

/// Event sent once a background compute completes
pub struct BackgroundComputeComplete<T>(pub T)
where
    T: Send + Sync + 'static;

impl<T> BackgroundComputeCheck<T> {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'w, 's> ComputeInBackgroundCommandExt for Commands<'w, 's> {
    fn compute_in_background<F, T>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + Sync + 'static,
    {
        self.add(ComputeInBackground(Mutex::new(future)))
    }
}

impl<T> Default for BackgroundComputePlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T> Plugin for BackgroundComputePlugin<T>
where
    T: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        app.add_event::<BackgroundComputeComplete<T>>()
            .insert_resource(BackgroundTasks::<T> { tasks: vec![] })
            .add_system(
                background_compute_check_system::<T>.label(BackgroundComputeCheck::<T>::new()),
            );
    }
}

impl<F, T> Command for ComputeInBackground<F, T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + Sync + 'static,
{
    fn write(self, world: &mut World) {
        world.resource_scope(|_, mut holder: Mut<BackgroundTasks<T>>| {
            let func = self
                .0
                .into_inner()
                .expect("Compute in background mutex error");
            holder
                .tasks
                .push(AsyncComputeTaskPool::get().spawn_pollable(func));
        });
    }
}

/// System responsible for checking tasks being computed in background and sending completion events
fn background_compute_check_system<T>(
    mut holder: ResMut<BackgroundTasks<T>>,
    mut event_writer: EventWriter<BackgroundComputeComplete<T>>,
) where
    T: Send + Sync + 'static,
{
    holder.tasks.retain(|pollable| {
        if let Some(value) = pollable.poll() {
            event_writer.send(BackgroundComputeComplete(value));
            false
        } else {
            true
        }
    })
}
