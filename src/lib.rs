//! A bevy plugin streamlining task handling.

use std::{fmt::Debug, future::Future, hash::Hash, marker::PhantomData, sync::Mutex};

use bevy_app::{App, Plugin, Update};
use bevy_ecs::{
    event::{Event, EventWriter},
    schedule::{IntoSystemConfigs, SystemSet},
    system::{Command, Commands, ResMut, Resource},
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
#[derive(SystemSet)]
pub struct BackgroundComputeCheck<T>(PhantomData<T>);

impl<T> Debug for BackgroundComputeCheck<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("BackgroundComputeCheck")
            .field(&self.0)
            .finish()
    }
}

impl<T> Clone for BackgroundComputeCheck<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> PartialEq for BackgroundComputeCheck<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for BackgroundComputeCheck<T> {}

impl<T> Hash for BackgroundComputeCheck<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

// TODO remove Mutex when possible, for now required to make this Sync (for Command trait)
// https://github.com/bevyengine/bevy/pull/5871
/// Command struct for holding a future
struct ComputeInBackground<F, T>(Mutex<F>)
where
    F: Future<Output = T> + Send + 'static,
    T: Send + Sync + 'static;

/// Resource for holding background tasks to check for completion
#[derive(Resource)]
struct BackgroundTasks<T> {
    tasks: Vec<PollableTask<T>>,
}

/// Event sent once a background compute completes
#[derive(Event)]
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
            .add_systems(
                Update,
                background_compute_check_system::<T>.in_set(BackgroundComputeCheck::<T>::new()),
            );
    }
}

impl<F, T> Command for ComputeInBackground<F, T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + Sync + 'static,
{
    fn apply(self, world: &mut World) {
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
