# Headless #

Headless rendering and frame export are controlled independently:

- **Headless** selects an offscreen render target instead of a window surface.
- **Export** copies the current render target to a PNG file.

## 1. Wayland Compositor

- `LNDRAWER_HEADLESS`: If present, do not create a `wgpu::Surface`; render into an offscreen texture.
- `LNDRAWER_WINDOW_SIZE`: If present, request the surface size in physical pixels, formatted as `WIDTHxHEIGHT`. This keeps exports independent of the compositor's scale factor and default window size.

A virtual Wayland compositor is enough. Weston's headless backend with the pixman renderer does not need a GPU or `/dev/dri`:

```bash
export XDG_RUNTIME_DIR=$PWD/target/headless/rt
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

weston \
    --backend=headless-backend.so \
    --renderer=pixman \
    --socket=wayland-headless \
    --width=1200 \
    --height=900 \
    --idle-time=0 &

export WAYLAND_DISPLAY=wayland-headless
unset DISPLAY
```

## 2. Software Rendering

Hardware rendering may not be available. `vulkan-swrast` is more recommended.

- `LNDRAWER_FORCE_FALLBACK_ADAPTER` enable wgpu `force_fallback_adapter` field to access Adapter

## 3. Save Permission

Use temporary save first, especially when you don't have write access to the save file location.

- `LNDRAWER_TMP_SAVE`: If present, use in-memory save & database.

## 4. Headless Export

Exporting needs headless environment.

- `LNDRAWER_EXPORT_PATH`: If present, export a frame to this path. Relative paths are resolved from the current directory. Parent directories are created.
- `LNDRAWER_EXPORT_FRAME`: Frame number to export, starting at `1`. Defaults to `1`.

Example:

```bash
LNDRAWER_HEADLESS=1 \
LNDRAWER_TMP_SAVE=1 \
LNDRAWER_FORCE_FALLBACK_ADAPTER=1 \
LNDRAWER_EXPORT_PATH=$PWD/target/headless/shot.png \
./target/debug/ln_drawer
```

The exporter requests redraws until the requested frame is rendered, then writes the PNG once and marks the export as done. The application logs the output path when the file has been written:

```text
exported frame to /path/to/shot.png
```

The process automatically exits after exporting.
