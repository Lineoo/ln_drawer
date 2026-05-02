# LnDrawer
<img src="ln_drawer/res/icon_hicolor_lime.svg" alt="LnDrawer Icon" align="right"/>
A simple, light-weight, GPU-accelerated drawing application written in Rust, driven by `winit` and `wgpu`.


## Build

Building requires ALSA Library, in Ubuntu you need:

```
apt-get install libasound2-dev
``` 

Manually build through `cargo`:

```
$ git clone --depth 1 https://github.com/Lineoo/ln_drawer
$ cd ln_drawer
$ cargo build --package=ln_drawer --bin ln_drawer
```

Android platform needs `cargo-apk` and Android SDK & NDK. Recommanded SDK platform is 34 and NDK version is 27.3.13750724.

```
$ cargo install cargo-apk
$ cargo apk build --package=ln_drawer --lib
```

## License

The entire repository is licensed under **GNU Lesser General Public License v3.0 or later** (LICENSE or https://opensource.org/license/LGPL-3.0).

This workspace contains multiple crates with different licenses:

- Crate **ln_drawer** is licensed under **LGPL-3.0-or-later**.
- Crate **ln_world** is dual-licensed under **MIT OR Apache-2.0**.

See the crate-level README file or license field for details.