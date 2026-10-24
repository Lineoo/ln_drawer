# 渲染 #

## 1. 概述

渲染分为多个部分：

1. **总控 Render** - 包含大部分 GPU 资源，负责重绘
2. **相机 Camera** - 作为坐标系，同时包括了所有视图内的渲染控制，交互控制节点
3. **控制 RenderPrepareControl RenderDrawControl** - 闭包
4. **管线 ComponentPipeline** - 包含绑定组布局，管线布局，Shader 等，用于创建实例
5. **组件 Component ComponentInstance** - 包含绑定组和缓冲区

## 2. 相机控制

有两个不同职能的相机控制器 `Container` 和 `CameraUtils`

前者包括了堆叠功能，也就是能够自动转发交互，自动传播渲染指令，控制渲染裁切矩形

后者主用于控制相机相对移动缩放，功能丰富

*注意目前 `container.scroll` 也控制了一部分相机移动，以后会修改*

## 3. 重绘

重绘由 OS 发出的 `WindowEvent::RedrawRequest` 事件控制，且发出后 Render 将*不可逆*地开始重绘（跳跃绘制除外）。

- 若渲染实现需要**实时动画**，应在自己的 `RenderPrepareControl` 在 `prepare` 调用中返回积极重绘为 `true`。
    - 当然最好是使用 `animation` 模块打包好的功能
- 不应在上一帧的渲染过程中触发下一帧的重绘，这会导致渲染不受控制无限进行下去。

