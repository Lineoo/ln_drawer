# 可用的环境变量 #

## 编译时

- `LNDRAWER_RELEASE` 使用发布模式（影响存档路径）

## 运行时

- `LNDRAWER_NO_READ_WRITE` 禁用设备 `STORAGE_READ_WRITE` 能力
- `LNDRAWER_DESKTOP_UI` 使用实验性桌面模式
- `LNDRAWER_SCALE_FACTOR` 覆盖 winit 报告的 HiDPI 缩放因子
- `LNDRAWER_WINDOW_SIZE` 覆盖窗口表面大小，格式为物理像素 `宽x高`
- `LNDRAWER_HEADLESS` 启用无头渲染
- `LNDRAWER_EXPORT_PATH` 无头渲染导出图片到路径
- `LNDRAWER_EXPORT_FRAME` 无头渲染捕获第几帧
- `LNDRAWER_FORCE_FALLBACK_ADAPTER` 启用 wgpu 的 force_fallback_adapter 来获取 Adapter
- `LNDRAWER_TMP_SAVE` 使用内存数据库，并跳过场景文件的读写