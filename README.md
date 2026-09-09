# Kanono Media Player

Kanono is a modular Arch Linux audio player foundation inspired by Foobar2000.

## Workspace layout

```text
apps/kanono-player/          iced desktop application and dynamic plugin host
crates/audio-engine/         Symphonia decoding and CPAL f32 output pipeline
crates/plugin-api/           shared plugin trait and exported ABI signatures
plugins/example-component/   example `.so` component (`cdylib`)
```

The output callback consumes one interleaved f32 queue. Decode the successor and
call `enqueue_track` before the current track drains: it appends rather than clears
the queue, which preserves sample order across a track boundary.

`playback_state_channel` is a bounded crossbeam bridge from decode/audio workers to
the iced thread. CPAL publishes position with `try_send`; decoded PCM is copied into
fixed-size `Arc<[f32]>` visualizer windows off the real-time thread. The UI polls at
30 Hz and drains stale updates, so rendering never blocks audio output.

Build the application with `cargo run -p kanono-player`. Build the example shared
object with `cargo build -p kanono-example-component --release`; it is emitted under
`target/release/` as `libkanono_example_component.so`.

## Plugin contract

Plugins export `kanono_plugin_api_version`, `kanono_create_plugin`, and
`kanono_destroy_plugin`. The host loads them with `libloading`. These symbols use
the Rust ABI because the shared `Plugin` trait crosses the library boundary, so
plugins and host must use the same Rust toolchain and `kanono-plugin-api` version.
A future independently distributed plugin SDK should use a C-compatible data ABI
(for example `abi_stable`) instead.

At startup, the player scans `./components` for `.so` files (or the directory in
`KANONO_COMPONENTS_DIR`), checks the API version, activates each unique component,
and retains the loaded library for the component's lifetime. A bad component is
isolated so other components can still load.