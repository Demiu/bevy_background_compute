use std::time::{Duration, Instant};

use bevy::{prelude::*, MinimalPlugins};
use bevy_app::{App, AppExit};
use bevy_background_compute::{
    BackgroundComputeCheck, BackgroundComputeComplete, BackgroundComputePlugin,
    ComputeInBackgroundCommandExt,
};
use bevy_ecs::schedule::IntoSystemConfigs;

// A newtype for holding funny numbers only
#[derive(Debug)]
struct FunnyNumber(i32);

fn main() {
    App::new()
        // Minimal plugins are required for creation of taskpools
        .add_plugins(MinimalPlugins)
        // Register the compute result type
        .add_plugins(BackgroundComputePlugin::<FunnyNumber>::default())
        // Producer system
        .add_systems(Startup, start_background_compute)
        // Add a consumer
        .add_systems(
            Update,
            background_compute_callback
                // Optional: Schedule the system receiving the callback event to
                // run after the check system to prevent single-update delays
                .after(BackgroundComputeCheck::<FunnyNumber>::new()),
        )
        .run();
}

fn start_background_compute(mut commands: Commands) {
    println!("Starting background compute");
    commands.compute_in_background(async move {
        let start_time = Instant::now();
        let duration = Duration::from_secs(5);
        while start_time.elapsed() < duration {
            // Simulating doing some very haard work!
        }
        // We have done it!
        FunnyNumber(69)
    });
    println!("Returning from starting compute");
}

fn background_compute_callback(
    mut events: EventReader<BackgroundComputeComplete<FunnyNumber>>,
    mut exit: EventWriter<AppExit>,
) {
    if events.len() > 0 {
        println!(
            "Funny number found: {:?}",
            events.iter().next().unwrap().0 .0
        );
        // Compute complete, schedule an exit
        exit.send(AppExit);
    }
}
