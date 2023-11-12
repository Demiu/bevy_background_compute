//! A bevy plugin streamlining task handling.

use std::{future::Future, marker::PhantomData};

use bevy_app::{App, Plugin};
use bevy_ecs::{
    event::EventWriter,
    schedule::{SystemSet, IntoSystemConfig},
    system::{Command, Commands, Resource, ResMut},
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

/// The set for the internal task checking system
#[derive(SystemSet)]
#[system_set(base)]
pub struct BackgroundComputeCheck<T>(PhantomData<T>);

/// Command struct for holding a future
struct ComputeInBackground<F, T>(F)
where
    F: Future<Output = T> + Send + 'static,
    T: Send + Sync + 'static;

/// Resource for holding background tasks to check for completion
#[derive(Resource)]
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
        self.add(ComputeInBackground(future))
    }
}

impl<T> Default for BackgroundComputePlugin<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

// These impls have to be written manually for BackgroundComputeCheck<T>
// instead of being derived because the #derive[] doesn't ignore the 
// T bounds in a PhantomData<T> member
// TODO see: https://github.com/rust-lang/rust/issues/26925 wait 4 fix
mod impls {
    use std::fmt::Debug;
    use std::hash::Hash;
    use super::BackgroundComputeCheck;

    // TODO this derive is undocumented as a requirement in the migration guide
    // try to remove it, make an issue
    impl<T> Clone for BackgroundComputeCheck<T> {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    impl<T> Debug for BackgroundComputeCheck<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "BackgroundComputeCheck<{:?}>", self.0)
        }
    }

    impl<T> PartialEq for BackgroundComputeCheck<T> {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }

    impl<T> Eq for BackgroundComputeCheck<T> { }

    impl<T> Hash for BackgroundComputeCheck<T> {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.0.hash(state);
        }
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
                background_compute_check_system::<T>.in_base_set(BackgroundComputeCheck::<T>::new()),
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
            let func = self.0;
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
