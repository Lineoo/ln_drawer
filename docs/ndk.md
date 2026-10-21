# Android NDK 编译问题 #

Android 的 `check` / `build` 一律用 `cargo ndk`：

```bash
cargo ndk check -t aarch64-linux-android
cargo ndk build -t aarch64-linux-android
```

配置 rust-analyzer 使用：

```bash
cargo ndk-env -t aarch64-linux-android --json
```

## cargo-ndk 的问题

cargo-ndk 的 `ndk-env` 命令有几个会导致 fingerprint 冲突的问题。这会导致 `zstd-sys` 以及其他依赖 cc 的库频繁触发重建。

除了配置 ra，其他位置绝对不应使用 `cargo ndk-env`

### 1. 变量名输出不一致

cargo-ndk 4.1.2 的两条路径变量名不一致：

- `cargo ndk build`：`CC_aarch64-linux-android`、`CFLAGS_aarch64-linux-android`（连字符）
- `cargo ndk-env`：`CC_aarch64_linux_android`、`CFLAGS_aarch64_linux_android`（下划线）

shell 版输出会把 key 改成下划线，直接用于构建会和 `cargo ndk` 的 fingerprint 冲突。需要把变量名改回与 `cargo ndk` 一致的连字符形式才能正常使用。

JSON 输出保留连字符 key，和 `cargo ndk` 一致，所以 ra 配置不用改。

### 2. FLAGS 重复追加

`cargo ndk-env` 还会在已有 `CFLAGS` 前重复追加 `--target=`，反复 source 会累积。需要开新 shell 或者清除环境变量。

### 3. CARGO 链接缺失

`cargo ndk-env` 过滤了 linker wrapper 需要的 `_CARGO_NDK_LINK_*`，所以此时 `cargo build` 的链接阶段总是不可靠的。