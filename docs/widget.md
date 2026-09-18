> NOTE: This page is written in Chinese.

# 渲染 & 用户界面 #

## 1. 概述

渲染分为多个部分：

1. **渲染总控 Render RenderPhase** - 包含所有渲染资源，负责重绘所有渲染控制逻辑
2. **渲染控制 RenderControl** - 控制渲染组件的排序、剔除、可见性，包含用于重绘闭包
3. **渲染管线 ComponentPipeline** - 包含绑定组布局，管线布局，Shader 等，用于创建实例
4. **渲染实例 Component** - 包含绑定组和缓冲区

## 2. 渲染实例

优先参见 `world.md` 的 `简单 Element 写法` 一节。

重点规范渲染实例的**结构**和**初始化**。

```rust
struct Panel {
    pub rect: Rectangle,
    pub visible: bool,
    pub shadow: bool,
}

world.insert(world.insert(Panel {
    rect: Rectangle::default(),
    visible: true,
    shadow: false,
}));
```

渲染组件首选**描述构建模式**来初始化渲染实例。过程：

1. 初始化并获取**对应的渲染管线**
2. 注册对应的 **GPU 指针数据**，如需要的 Buffer 等
3. 在世界中生成**渲染控制节点**并完成注册
4. 在世界中完成**生命周期追踪**，主要是观察者和对象依赖

将插入了三种元素：

- 核心元素 `Panel`
- 渲染元素 `RRect`
- 渲染控制节点 `RenderControl`

## 3. 渲染绘制

渲染命令从事件循环出发，并由 `lnwin` 转移给**渲染总控**。

1. 解析所有**渲染控制**节点，进行排序、剔除
2. 按序遍历所有节点并调用 `prepare` call
3. 按序遍历所有节点并触发 `redraw` call

## 4. 修改同步 & 删除

在渲染组件修改后，我们需要：

1. 上传对应数据到 GPU
2. 应用 RenderControl 对应更改
3. 通知重绘

修改由 observer 系统控制。参见 `docs/observer.md`

## 5. 重绘问题

重绘由 OS 发出的 `WindowEvent::RedrawRequest` 事件控制，且发出后 Render 将*不可逆*地开始重绘（跳跃绘制除外）。

- 若渲染实现需要**实时动画**，应在自己的 `RenderControl` 在 `prepare` 调用中返回积极重绘为 `true`。
- 不应在上一帧的渲染过程中触发下一帧的重绘，这会导致渲染不受控制无限进行下去。

## 6. 分组绘制

- 一个 Render 只持有一个主 Render Pass
- 一个 RenerPhase 对应一组 Draw Calls

这个是为了方便设置矩形裁切等进行分组绘制而设置的。

- Redraw 直接发生在 Lnwindow 窗口层
- 被自定义节点分发到下属节点

对应的世界架构在 `world.md` 可供参考

控制流：

1. winit 负责处理 OS 重绘事件
2. 由 Lnwindow 转发到 Render
3. Render 开始处理 prepare 遍历自己视图下的所有 Camera
    - 调用 RenderControl 的 prepare
    - RenderControl 逐级下发 prepare 指示
    - 为 RenderControl 排序
4. 开始重绘，由 Render 创建 RenderPass 和 RenderExtra
    - 逐级下发

## 7. 内嵌容器

容器是一块**内嵌子界面**，拥有自己的视图、局部坐标系与独立相机。内部元素的渲染与交互
都基于该相机，与外层界面无关。

- 内容按本地坐标布局；平移与滚动只改变子相机，不触发子元素重排或重新上传。
- 内容挂在容器视图内的子 `RenderPhase` 中，由外层的一个渲染控制负责：先按外层相机设置
  裁剪，再进入容器视图绘制，最后恢复。
- 子相机对外层不可见，不会被顶层相机遍历收集，也不会被重复绘制。

相机与视图语义参见 `world.md` 的「元素视图」。

