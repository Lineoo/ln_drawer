<p><img src="ln_drawer/res/banner.svg" alt="LnDrawer 横幅" align="center"/></p>

[English](./README.md) | **中文**

----------

<img src="ln_drawer/res/icon_hicolor_lime.svg" alt="LnDrawer 图标" align="right"/>

**LnDrawer** 是一款简单、轻量、GPU 加速的绘图应用，使用 Rust 编写，由 `winit` 和 `wgpu` 驱动。


## 从源码构建

通过 `cargo` 手动构建：

```
$ git clone --depth 1 https://github.com/Lineoo/ln_drawer
$ cd ln_drawer
$ cargo build --package=ln_drawer --bin ln_drawer
```

Android 平台需要 `cargo-ndk`、Android SDK 34、Build Tools 34.0.0、NDK 29.0.14206865 和 JDK 17 或 21。

```
$ cargo install cargo-ndk
$ cd android
$ ./gradlew assembleDebug
```

Gradle 的 `cargoNdk` 任务会把 Rust 库编译到 `android/app/src/main/jniLibs`。
需要设置 `ANDROID_HOME` 和 `ANDROID_NDK_HOME`。

## 许可证

整个仓库基于 **GNU Affero 通用公共许可证 v3.0 或更高版本**（LICENSE 或 https://opensource.org/license/AGPL-3.0）授权。

此工作区包含多个具有不同许可证的 crate：

- **ln_drawer** crate 基于 **AGPL-3.0-or-later** 授权。
- **ln_world** crate 基于 **MIT OR Apache-2.0** 双许可证授权。

详情请参阅 crate 级别的 README 文件或 license 字段。

本项目中的部分图标来自 [Lucide Icons](https://lucide.dev/)。