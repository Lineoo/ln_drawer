GPU-accelerated drawing app featured infinite canvas in Rust (winit + wgpu). Cargo workspace with two crates:

- `ln_drawer/` — the main app. Desktop (binary) entry: `src/bin/ln_drawer.rs` -> `lib.rs::desktop_main`; Android (C ABI library) entry: `lib.rs::android_main`.
- `ln_world/` — an in-house, single-file ECS (`src/lib.rs`), NOT a crates.io framework. The entire app is built on it (`World`, `Element`, `Handle`, `Observer`, `Descriptor`).

## Tasks

- If assigned to review in read-only mode, assume that users has run `cargo check` and `cargo fmt`. Skip cargo tool calls that'll need write-permission, except when significant type errors are spotted.
- Tests: `cargo test` — tests are all are pure-compute and run headless.
- If Vulkan backend not found (usually in sandbox env), use Chromium's SwiftShader. However, headless tests are preferred than running actually rendering.

## Critical gotchas

- `winit` is patched to a customized fork. This fork contains additional Android stylus support.
- `glam::I64Vec2` and `i64` may be used to represent `Q32.32` fixed-point types rather than integers, see `ln_drawer/src/measures/fixed.rs`

## Conventions

- No `foo/mod.rs`, please use new module style - `foo.rs` and `foo/*.rs`.
- Minimalistic comments or no comments at all. Don't leave any outdated, unrelated or redundant information.
- Panic is fine. As long as panic condition is clear, use `unwrap()` instead of silent return or messing up code structures. 
- Observer system design follows a three-part data-flow model. See `docs/observer.en.md` and `docs/observer.md` (Chinese).
- If you are adding new `Element`s, see `docs/world.md` (Chinese) first to see what happens.
- Render system architecture overview: `docs/widget.md` (Chinese).
- Drawing system architecture overview: `docs/draw.md` (Chinese).
- WGSL shaders are embedded via `include_str!` and macro-expanded with `shader_compile()` in `ln_drawer/src/widgets/shaders.rs`. Shared libs in `widgets/shaders/*.wgsl` are included via `#lib_*` during compiling.