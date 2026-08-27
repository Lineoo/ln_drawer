# AGENTS.md

GPU-accelerated drawing app in Rust (winit + wgpu). Cargo workspace with two crates:

- `ln_drawer/` — the app (LGPL-3.0-or-later). Desktop entry: `src/bin/ln_drawer.rs` → `lib.rs::desktop_main`; Android entry: `lib.rs::android_main` (`crate-type = ["lib","cdylib"]`).
- `ln_world/` — an in-house, single-file ECS (`src/lib.rs`, ~1170 lines), NOT a crates.io framework. The entire app is built on it (`World`, `Element`, `Handle`, `Observer`, `Descriptor`). `docs/world.md` (Chinese) is the design reference for ECS semantics.

## Build & run

- ALSA is required: `sudo apt-get install libasound2-dev` (rodio pulls it in).
- `cargo build --package=ln_drawer --bin ln_drawer`; `cargo run -p ln_drawer --bin ln_drawer`.
- Tests: `cargo test` — tests live in `ln_world/src/lib.rs`, `ln_drawer/src/layer/stream.rs`, and `ln_drawer/src/widgets/shaders.rs`; all are pure-compute and run headless. `cargo test <name>` works per-test as usual.
- Format gate: `cargo fmt --all --check` (enforced by CI and the `scripts/hooks/pre-push` hook). rustfmt config sets `group_imports = "StdExternalCrate"` and `imports_granularity = "Crate"` — imports must be organized as grouped crate imports, never `use foo::{a, b}` split lines.

## Critical gotchas

- `winit` is patched to a fork: `[patch.crates-io.winit] git = "https://github.com/Lineoo/winit/" branch = "ln_drawer"` (root `Cargo.toml`). This fork contains additional Android stylus support. Don't remove the patch or bump winit versions.
- `LNDRAWER_RELEASE=1` at compile time (`option_env!` in `save.rs`) switches the save dir between `LnDrawerDev` and `LnDrawer`. It is set by CI/release builds and `build/release.sh`; leave it unset in dev.
- `ln_drawer/build/release.sh` rewrites `ln_drawer/Cargo.toml` **in place** (strips `-dev` from version, rewrites android package id `dev.linn.lndrawer` → `org.linn.lndrawer`, label, and symlinks the mipmap icon). It is for the release workflow only — do not run it locally and leave Cargo.toml uncommitted-modified.
- Android: requires `cargo-apk`, SDK platform 34, NDK 27.3.13750724. Build with `cargo apk build --package=ln_drawer --lib`. Android-only deps are `cfg(target_os = "android")`.

## Conventions

- All inter-element communication follows a three-part data-flow model (发者 Sender / 收者 Receiver / 数据流 Data flow). See `docs/observer.md` for the full model and naming conventions (`Set*` command vs bare notification).

## Rendering conventions

- wgpu 30. WGSL shaders are embedded via `include_str!` and macro-expanded with `shader_compile()` (in `ln_drawer/src/widgets/shaders.rs`): `#lib_camera`, `#lib_colorspace`, `#lib_constant`, `#lib_rectangle` map to the shared libs in `widgets/shaders/*.wgsl`. When adding a shader, reuse those libs rather than duplicating code.
- Render system architecture overview: `docs/widget.md` (Chinese).
