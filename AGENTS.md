# AGENTS.md

GPU-accelerated drawing app featured infinite canvas in Rust (winit + wgpu). Cargo workspace with two crates:

- `ln_drawer/` - The main app. Desktop (binary) entry: `src/bin/ln_drawer.rs` -> `lib.rs::desktop_main`; Android (C ABI library) entry: `lib.rs::android_main`.
- `ln_world/` - An in-house, single-file ECS (`src/lib.rs`). The entire app is built on it (`World`, `Element`, `Handle`, `Observer`, `Descriptor`).
- `docs/` - Documents. Most are in Chinese, read some first.

## Tasks

- If assigned to review in read-only mode, assume that users has run `cargo check` and `cargo fmt`. Skip cargo tool calls that'll need write-permission, except when significant type errors are spotted.
- Headless rendering is only experimentally supported, and just tell user when you need it.
- Don't run `cargo check --target aarch64-linux-android`. Tell user to perform Android check or build.
- Don't use clippy. No need to check that (it returns warnings, but we just don't care about that). 
- Tests: `cargo test` — tests are all are pure-compute and run headless.

## Critical gotchas

- `winit` is patched to a customized fork. This fork contains additional Android stylus support.
- `glam::I64Vec2` and `i64` may be used to represent `Q32.32` fixed-point types rather than integers, see `ln_drawer/src/measures/fixed.rs`

## Conventions

- Comments should not exceed one line. Keep minimalistic. Don't leave any outdated, unrelated or redundant information.
- Never use `try_*` family functions in `ln_world`. `unwrap_or_*()` / `if let Ok(x) =  ...` / `match result { Ok(x) => ... }` is strictly forbidden unless users explicit allow it. **Panicking is better than skipping potential errors.**
- Never handle errors. Use `unwrap()` if `Result`s are returned. If you REALLY understand the error and REALLY need to handle it, ask user first and make sure they ARE fully aware of the error and intended to handle it. **Panicking is better than skipping unknown errors.**
- Do not overuse blocks or early drops. While `ln_world` uses runtime borrow validation, most system is welled designed to avoid most borrow collisions.
- WGSL shaders are embedded via `include_str!` and macro-expanded with `shader_compile()` in `ln_drawer/src/widgets/shaders.rs`. Shared libs in `widgets/shaders/*.wgsl` are included via `#lib_*` during compiling.

## Documents

Documents are in Chinese if not specially marked with language suffix. Sort by recommendation level.

- `docs/codebase.en.md` - The coding style instructions, list in format `### <tags,> - <title>`. Recommended.
- `docs/world.md` - about how to use `ln_world`. Recommended.
- `docs/composite.md` - Color-space notation for rendering and drawing. Important when treating color conventions.
- `docs/draw.md` - Draw call pipeline: batch-draw, stroke-submit, stroke-discard, undo/redo, failure paths, stream-thread synchronization.
- `docs/render.md` - Rendering architecture: render camera, controls, pipeline, instances, redraw rules.
- `docs/redb.md` - `redb` database tables and chunk binary formats.
- `docs/env.md` - Available environment variables.
- `docs/headless.en.md` - How to use experimental headless rendering.
- `docs/ndk.md` - Android `cargo ndk` usage and `cargo-ndk-env` fingerprint pitfalls. Not very useful for agents environment.
