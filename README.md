<p><img src="ln_drawer/res/banner.svg" alt="LnDrawer Banner" align="center"/></p>

**English** | [中文](./README.zh_CN.md)

----------

<img src="ln_drawer/res/icon_hicolor_lime.svg" alt="LnDrawer Icon" align="right"/>

**LnDrawer** is a simple, light-weight, GPU-accelerated drawing application written in Rust, driven by `winit` and `wgpu`.


## Build

Manually build through `cargo`:

```
$ git clone --depth 1 https://github.com/Lineoo/ln_drawer
$ cd ln_drawer
$ cargo build --package=ln_drawer --bin ln_drawer
```

Android platform needs `cargo-ndk`, Android SDK 34, Build Tools 34.0.0, NDK 29.0.14206865 and JDK 17 or 21.

```
$ cargo install cargo-ndk
$ cd ln_drawer/build/android
$ ./gradlew assembleDebug
```

The `cargoNdk` Gradle task builds the Rust library into `ln_drawer/build/android/app/src/main/jniLibs`.
`ANDROID_HOME` and `ANDROID_NDK_HOME` must be set.

## License

The entire repository is licensed under **GNU Affero General Public License v3.0 or later** (LICENSE or https://opensource.org/license/AGPL-3.0).

This workspace contains multiple crates with different licenses:

- Crate **ln_drawer** is licensed under **AGPL-3.0-or-later**.
- Crate **ln_world** is dual-licensed under **MIT OR Apache-2.0**.

See the crate-level README file or license field for details.

Some icons in this project are from [Lucide Icons](https://lucide.dev/).