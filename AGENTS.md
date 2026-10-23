# AGENTS.md

GPU-accelerated drawing app featured infinite canvas in Rust (winit + wgpu). Cargo workspace with two crates:

- `ln_drawer/` - The main app. Desktop (binary) entry: `src/bin/ln_drawer.rs` -> `lib.rs::desktop_main`; Android (C ABI library) entry: `lib.rs::android_main`.
- `ln_world/` - An in-house, single-file ECS (`src/lib.rs`). The entire app is built on it (`World`, `Element`, `Handle`, `Observer`, `Descriptor`).
- `docs/` - Documents. Most are in Chinese, read some first.

## Tasks

- If assigned to review in read-only mode, assume that users has run `cargo check` and `cargo fmt`. Skip cargo tool calls that'll need write-permission, except when significant type errors are spotted.
- Don't run actual rendering tests, headless rendering is not supported, and just tell user when you need it.
- Don't run `cargo check --target aarch64-linux-android`. Tell user to perform Android check or build.
- Don't use clippy. No need to check that (it returns warnings, but we just don't care about that). 
- Tests: `cargo test` — tests are all are pure-compute and run headless.

## Critical gotchas

- `winit` is patched to a customized fork. This fork contains additional Android stylus support.
- `glam::I64Vec2` and `i64` may be used to represent `Q32.32` fixed-point types rather than integers, see `ln_drawer/src/measures/fixed.rs`

## Conventions

- Never leave comments. Keep minimalistic. Don't leave any outdated, unrelated or redundant information.
- Never use `try_*` family functions in `ln_world`, and `unwrap_or_*()` / `if let Ok(x) =  ...` / `match result { Ok(x) => ... }` is strictly forbidden unless users explicit allow it. **Panicking is better than skipping potential errors.**
- Never handle errors. Use `unwrap()` if `Result`s are returned. If you REALLY understand the error and REALLY need to handle it, ask user first and make sure they ARE fully aware of the error and intended to handle it. **Panicking is better than skipping unknown errors.**
- Keep it in mind. Let it be part of you. And please repeat it. **Panicking is better than skipping errors.**
- WGSL shaders are embedded via `include_str!` and macro-expanded with `shader_compile()` in `ln_drawer/src/widgets/shaders.rs`. Shared libs in `widgets/shaders/*.wgsl` are included via `#lib_*` during compiling.
