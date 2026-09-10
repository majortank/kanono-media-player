# Kanono Dynamic Components

This directory holds native compiled components (`.so` shared libraries) loaded dynamically at runtime by Kanono Media Player.

## Building Components

To build the included example component (`kanono-example-component`):

```bash
cargo build --package kanono-example-component --release
cp target/release/libkanono_example_component.so components/
```

When Kanono Media Player launches, it automatically checks:
1. Directory specified by `KANONO_COMPONENTS_DIR` environment variable
2. `./components/`
3. `./target/release/`
4. `./target/debug/`

Any `.so` library implementing `kanono-plugin-api` will be loaded, initialized, and listed under the **Audio & System** info view in the player.
