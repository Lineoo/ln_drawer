# Headless #

Headless rendering and frame export are controlled independently:

- **Headless** selects an offscreen render target instead of a window surface.
- **Export** copies the current render target to a PNG file.

Either can be enabled without the other.

## 1. Environment variables

| Variable | Meaning |
|---|---|
| `LNDRAWER_HEADLESS` | If present, do not create a `wgpu::Surface`; render into an offscreen texture. Any value enables it, including `0`. |
| `LNDRAWER_EXPORT_PATH` | If present, export a frame to this path. Relative paths are resolved from the current directory. Parent directories are created. |
| `LNDRAWER_EXPORT_FRAME` | Frame number to export, starting at `1`. Defaults to `1`. |
| `LNDRAWER_FORCE_FALLBACK_ADAPTER` | If present, request a fallback (CPU) adapter. Requires a CPU Vulkan ICD such as lavapipe. |

## 2. Wayland Compositor

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

## 3. Vulkan Driver

Hardware rendering may not be available. `vulkan-swrast` is more recommended.

## 4. Headless Export

Exporting needs headless environment.

Headless export:

```bash
LNDRAWER_HEADLESS=1 \
LNDRAWER_FORCE_FALLBACK_ADAPTER=1 \
LNDRAWER_EXPORT_PATH=$PWD/target/headless/shot.png \
./target/debug/ln_drawer
```

The exporter requests redraws until the requested frame is rendered, then writes the PNG once and marks the export as done. The application logs the output path when the file has been written:

```text
exported frame to /path/to/shot.png
```

The process exits after exporting.
