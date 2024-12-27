# Bevy background compute

This [bevy](https://bevyengine.org) plugin provides improvements to handling of background tasks. It adds a `Commands` extension to more easily place a `Future` onto bevy's `TaskPool` and upon completion it will send a callback in the form of an event containing the result.

## Full usage example

Check out the basic_usage example with

`cargo run --example basic_usage`

## Registering a type as background computable

In order to keep track of running tasks and to produce completion events every type returned by your Futures has to be registered using the `BackgroundComputePlugin<T>`.

```rust ignore
app.add_plugins(BackgroundComputePlugin::<MyType>::default())
```

## Computing a Future in background

```rust ignore
commands.compute_in_background(async {
    // Your code here
});
```

## Getting the result

```rust ignore
// Create a system consuming BackgroundComputeComplete<T> events
fn my_callback_sys(
    mut events: EventReader<BackgroundComputeComplete<MyType>>
) {
    // Handle like any other bevy event
}
```

## Version compatibility

Plugin | Bevy
--- | ---
0.1 | 0.8
0.2 | 0.9
0.3 | 0.10
0.4 | 0.11
0.5 | 0.12
0.6 | 0.13
