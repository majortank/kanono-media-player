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

`GaplessQueueManager` is the playback-control boundary for transitions. Preload the
current and successor with `preload_path` on a decode worker, then call `start`.
The manager refuses to start a non-final track unless its successor's metadata and
PCM are ready, and `promote_if_low_water` appends that successor at a two-second
per-channel low-water mark without ever running decoding in CPAL's callback.

`LibraryDatabase` uses a local SQLite WAL database for recursive MP3, FLAC, and WAV
indexing. `TrackQuery` composes text, artist, album, genre, and year filters for the
Playlist view. MP3 `TagUpdate` batches write ID3v2.4 tags and mirrors the changes to
the database transaction; FLAC and WAV remain read-only until their native tag writers
are added.

`ReplayGainWorker` performs EBU R128 / BS.1770 integrated-loudness and true-peak
analysis on a dedicated background thread. It writes `REPLAYGAIN_TRACK_GAIN` and
`REPLAYGAIN_TRACK_PEAK` ID3v2.4 `TXXX` tags for MP3 inputs, targeting -18 LUFS.

The player exposes `org.mpris.MediaPlayer2.kanono` on the user session bus for
desktop media controls. `LatencyProfile::FixedFrames(n)` requests a CPAL buffer size
that is supported by the selected device; for PipeWire, configure the server before
launching Kanono, e.g. `PIPEWIRE_LATENCY=128/48000 cargo run -p kanono-player`.
For JACK, select the desired server period/buffer before starting the app.

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