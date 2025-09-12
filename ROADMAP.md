> NOTE: This page is written in Chinese.

# 主要愿景
- 绘制
- 图片参考
- 备忘录
- 可直接执行的脚本文本
- 节点/自动化
- 全统一操作逻辑
    - 程序设置等和导入的图像等是同一层级（只是不可删除）

# 具体目标
- 渲染框架
    - 页面元素
        - 图片
        - 文字
        - 按钮
    - 元素编组
    - 多页面
    - 渲染偏移与精度修正
- 交互
    - 移动
    - 缩放/拉伸
    - 文本输入
    - 右键菜单
- 全屏覆盖层
    - 需要能够传递鼠标，键盘事件
- 功能
    - 一个文本框，输入 script 可以直接运行 shell
        - 还可以把 stdout 和 stderr 输出出来（用连线）
    - 可以调节的笔刷
        - 流量 粗细 颜色

# 架构
- 界面基础渲染，与 wgpu 直接交互，使用屏幕空间 `interface`
    - 通用的按钮，标签等组件
    - 用于拖曳组件的线框，手柄
    - 直接纹理渲染与网格渲染
    - 笔画的细分网格 StrokeSection
    - 节点之间连接的箭头 Line
    - 右键菜单
    - 空间变化
    - interface 的位置等由自己存储
- 更加高级的逻辑组件，不涉及 wgpu 与屏幕空间 `elements`
    - 笔画的渲染图层 `StrokeLayer`
    - 添加的图像 `Image`
    - 节点连接 `NodeLink`
    - 交互按钮 `Button`
    - 文本编辑栏
- 总体的各种功能支持，绘制、工具与导入，承接 IO 和用户输入 `world`
    - 框选
    - 相交检测 / 按钮
    - 用于实现 stroke 的笔画管理器 `StrokeManager`
    - 添加图像的入口，甚至是截图工具
- 输入转接，处理各类平台与输入转换为标准交互给 World 使用 `inbox`
    - 移动平台：触摸，各类传感器，虚拟输入法
    - PC 平台：笔输入，触摸，鼠标，键盘
- 窗口管理，与 winit 直接交互 `lnwin`
    - 全屏覆盖层
    - 输入处理
- 主程序 `main`
    - 事件循环

# 控制流
> 需求：按钮需要在处理请求时能够获取对应调色板的位置

~~两者都在 World 中，只能通过预先输入/使用Arc多所有权来获取~~

~~预先注册参数可能会好一点，问题是：~~
~~- 一个预先注册的引用有严重的生命周期问题~~

使用后处理 update_within() 函数。

# 世界
处于精度/距离效应的考量，世界的坐标使用整数像素单位存储（PhysicalPosition）。

而无限缩放则使用数值嵌套来实现。

# 可能的优化

1. 减少 CPU 到 GPU 的数据传输：使用 compute shader 在 GPU 上直接绘制
2. 事件累积和批量处理：不要每次鼠标移动都立即更新
3. 双缓冲纹理：避免读写冲突
4. 分层绘制：分离实时绘制和最终合成
5. 事件过滤：基于时间和距离阈值过滤过多的鼠标事件

# Interface 模式
类 descriptor
```rust
struct InterfaceHandle<T: InterfaceBuffer> {
    tx: Sender<(usize, T::Descriptor)>
    comp_idx: usize,
}

let mut text: InterfaceHandle<TextBuffer> = interface.create_text(TextDescriptor {
    text: Some("hi"),
    size: Some(24.0),
    color: None, // Use default value
    rect: None, // default value??
    ..Default::default()
});
text.set_value(TextDescriptor {
    text: Some("hi"),
    size: Some(24.0),
    color: None, // Keep unchanged
    ..Default::default()
})
``` 

Factory
```rust
let mut text: Text = interface.create_text();
text.set_text("hello");
text.set_z_order(1); // 渲染支持由 interface 提供，数据由组件自己负责
text.flush();
```

# 史记
现在是九月，更新放缓