use std::{marker::PhantomData, sync::Mutex, future::Future};

use bevy_app::{Plugin, App};
use bevy_ecs::{system::{Commands, Command, ResMut}, world::{World, Mut}, event::EventWriter};
use bevy_tasks::AsyncComputeTaskPool;
use pollable::{PollableTask, SpawnPollableExt};

mod pollable;

/// Extension trait to add compute_in_background to Commands
trait ComputeInBackgroundCommandExt {
    fn compute_in_background<F, T>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static;
}

struct BackgroundComputePlugin<T>(PhantomData<fn()->T>);

/// Command struct for holding a future
// TODO remove Mutex when possible, for now required to make this Sync (for Command trait)
// https://github.com/bevyengine/bevy/pull/5871
struct ComputeInBackground<F, T> (Mutex<F>)
where
    F: Future<Output = T>,// + Send + 'static,
//    T: Send + 'static;
;

/// Resource for holding background tasks to check for completion
struct BgComputeTaskHolder<T> {
    tasks: Vec<PollableTask<T>>,
}

/// Event sent once a background compute completes
pub struct BackgroundComputeComplete<T>(T)
where
    T: Send + Sync + 'static;

impl<'w, 's> ComputeInBackgroundCommandExt for Commands<'w, 's> {
    fn compute_in_background<F, T>(&mut self, future: F)
    where
        F: Future<Output = T> + Send + 'static,
        T: Send + 'static,
    {
        self.add(ComputeInBackground(Mutex::new(future)))
    }
}

impl<T> Plugin for BackgroundComputePlugin<T>
where 
    T: Send + Sync + 'static,
{
    fn build(&self, app: &mut App) {
        app
            .add_event::<BackgroundComputeComplete<T>>()
            .insert_resource(BgComputeTaskHolder::<T>{tasks:vec![]})
            .add_system(background_compute_check::<T>);
    }
}

impl<F, T> Command for ComputeInBackground<F, T>
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    fn write(self, world: &mut World) {
        world.resource_scope(|world, pool: Mut<AsyncComputeTaskPool>| {
            world.resource_scope(|_, mut holder: Mut<BgComputeTaskHolder<T>>| {
                let func = self.0.into_inner().expect("Compute in background mutex error");
                holder.tasks.push(pool.spawn_pollable(func));
            });
        });
    }
}

/// System responsible for checking tasks being computed in background and sending completion events
fn background_compute_check<T>(mut holder: ResMut<BgComputeTaskHolder<T>>, mut event_writer: EventWriter<BackgroundComputeComplete<T>>)
where
    T: Send + Sync + 'static
{
    holder.tasks.retain(|pollable| {
        if let Some(value) = pollable.poll() {
            event_writer.send(BackgroundComputeComplete(value));
            false
        } else { true }
    })
}
