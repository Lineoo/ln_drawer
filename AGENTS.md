# AGENTS.md

GPU-accelerated drawing app in Rust (winit + wgpu). Cargo workspace with two crates:

- `ln_drawer/` — the app (LGPL-3.0-or-later). Desktop entry: `src/bin/ln_drawer.rs` → `lib.rs::desktop_main`; Android entry: `lib.rs::android_main` (`crate-type = ["lib","cdylib"]`).
- `ln_world/` — an in-house, single-file ECS (`src/lib.rs`, ~1170 lines), NOT a crates.io framework. The entire app is built on it (`World`, `Element`, `Handle`, `Observer`, `Descriptor`). `ROADMAP.md` (Chinese) is the design reference for ECS semantics, but parts of it are stale (esp. the Observer & Trigger section) — the data-flow norms below and the code take precedence.

## Build & run

- ALSA is required: `sudo apt-get install libasound2-dev` (rodio pulls it in).
- `cargo build --package=ln_drawer --bin ln_drawer`; `cargo run -p ln_drawer --bin ln_drawer`.
- Tests: `cargo test` — tests live in `ln_world/src/lib.rs`, `ln_drawer/src/layer/stream.rs`, and `ln_drawer/src/widgets/shaders.rs`; all are pure-compute and run headless. `cargo test <name>` works per-test as usual.
- Format gate: `cargo fmt --all --check` (enforced by CI and the `scripts/hooks/pre-push` hook). rustfmt config sets `group_imports = "StdExternalCrate"` and `imports_granularity = "Crate"` — imports must be organized as grouped crate imports, never `use foo::{a, b}` split lines.

## Critical gotchas

- `winit` is patched to a fork: `[patch.crates-io.winit] git = "https://github.com/Lineoo/winit/" branch = "ln_drawer"` (root `Cargo.toml`). The code uses a non-standard `window.surface_size()` method that only exists in this fork. Don't remove the patch or bump winit versions.
- `LNDRAWER_RELEASE=1` at compile time (`option_env!` in `save.rs`) switches the save dir between `LnDrawerDev` and `LnDrawer`. It is set by CI/release builds and `build/release.sh`; leave it unset in dev.
- `ln_drawer/build/release.sh` rewrites `ln_drawer/Cargo.toml` **in place** (strips `-dev` from version, rewrites android package id `dev.linn.lndrawer` → `org.linn.lndrawer`, label, and symlinks the mipmap icon). It is for the release workflow only — do not run it locally and leave Cargo.toml uncommitted-modified.
- Android: requires `cargo-apk`, SDK platform 34, NDK 27.3.13750724. Build with `cargo apk build --package=ln_drawer --lib`. Android-only deps are `cfg(target_os = "android")`.

## Data flow architecture (observer pattern)

All inter-element communication follows a three-part data-flow model (the whole project's observer pattern is built on it). ROADMAP's observer/trigger guidance is outdated — follow this and the code.

1. **发者 Sender** — the element that *releases* data. e.g. `Slider` emits `SliderValue(f32)` while dragged; `color_picker.rs` observes it and writes into `LayerWrapper`'s brush settings. Emission is internal to the sender's implementation.
2. **收者 Receiver** — the element that *accepts* data. e.g. `Slider` listens for `SetSliderValue(f32)` to apply an external value. Acceptance is internal to the receiver's implementation.
3. **数据流 Data flow** — the *transfer* between them, wired externally (by whoever composes the two). e.g. `color_picker.rs` observes a `SliderValue` event and `world.queue_trigger`s a `SetSliderValue` at the target slider.

Naming: a paired event type is a *command* with a `Set` prefix (observed by the receiver, e.g. `SetSliderValue`) and a *notification* with the bare name (emitted by the sender, e.g. `SliderValue`). `Echo` (`widgets/echo.rs`) exists to re-emit a `Set*` command as its matching notification on the same node.

Rules:

- A node can be both sender and receiver: e.g. `Tabs` accepts `SetWidgetRectangle` and re-emits the sub-panel `WidgetRectangle`.
- The sender does not need to own state for the data it releases. e.g. `Button` emits `WidgetHover` as a sender; the hover state it keeps is only for its own internal rendering, which also consumes `WidgetHover` as a receiver — that self-loop is an implementation detail and is ignored when wiring.
- A flow need not have exactly two nodes. e.g. `Transform` names an explicit `source`/`target` pair and is itself part of the data flow (it transforms `WidgetRectangle` from the source into `SetWidgetRectangle` at the target) — three logical nodes. Internally observers are also nodes, so real flows always have more than two.
- A receiver is not necessarily command-event driven. Legacy code uses `when_modify` (e.g. `ToolCollider`, `RoundedRect`) and some paths require manual calls; conceptually they are still receivers, just without a decoupling event, and their internal behavior can leak into the data-flow control. Prefer command events for new code.
- Accepting data is not mandatory: `Slider`/`Button` support receiving `Theme` data, but usually display is driven only by init-time values, which is fine.

## Rendering conventions

- wgpu 30. WGSL shaders are embedded via `include_str!` and macro-expanded with `shader_compile()` (in `ln_drawer/src/widgets/shaders.rs`): `#lib_camera`, `#lib_colorspace`, `#lib_constant`, `#lib_rectangle` map to the shared libs in `widgets/shaders/*.wgsl`. When adding a shader, reuse those libs rather than duplicating code.
