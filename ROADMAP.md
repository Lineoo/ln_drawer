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
    - 框选，相交检测 / 按钮 `Intersect`
    - 物理系统
    - 运行 Shell，动态链接/组件，OS 程序 `Executor`
    - 添加图像的入口，甚至是截图工具
    - 用于实现 stroke 的笔画管理器 `StrokeManager`
    - 文本编辑栏
- 提供业务逻辑支持 `world`
    - 与 interface *完全无关*！World 里的东西完全不需要渲染（甚至不需要 winit 和 wgpu）就应该可以使用。
    - Element 之间的更新业务逻辑
    - 一个简单的 RefCell 实现多可变访问
    - 带有一个事件注册系统，提供一个原生的 Observer / Trigger 系统
- 输入转接，导入，承接 IO 和用户输入，处理各类平台与输入转换为标准交互给 World 使用 `inbox`
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

# 修改器模式
使用 dyn + Trait

```rust
// impl World
// fn insert<T: Element>(&mut self, T) -> WorldEntry<'_, T>
let handle = world.insert(child_obj)
    .observe(|event: PositionChange, world: &World| {

    })
    .observe_with(ChildOf(parent))
    .observe_with(ClampPosition { up, down, left, right });

world.entry_dyn(handle) // WorldEntry<'_, T>
    .trigger(ChangePosition([10, 20]));

impl<T: PositionedElement> Modifier<T> for ChildOf {
    type Event = PositionChange

    fn attach(&mut self, target: WorldEntry<T>) {
        
    }
}
```

# 主交互逻辑
当我们按下键盘时，按键应该只发送给活跃工具元素并由他全权决定，还是发送给所有参与的元素而活跃工具只是其中一部分？

切换工具时，我们切换了个什么？
- 切换了我们的“期望”行为，我更倾向于上述后者的行为，我觉得独立处理总是好的，也就是说，我们只是显性地确保了只有一个活跃的工具元素（active 设为 true 的）。
- 这就像符号链接一样，我们会给一个特定的单例发送用户输入，再由它转发给这个符号链接（呃，或者说它本身是一个符号链接）

在按下键盘按键时会给所有注册了对应事件 KeyDown 的元素发送信号，这允许比如监听一个按键来实现快捷键功能。
- TODO 为了全局事件的性能，我们可能得改一下 Observer 的实现

其他交互事件也会发送给注册了专有监听的工具元素并由其再转发给世界其他元素。

采用全局广播的模式：键盘和鼠标时间全局广播，一般会有一些普通元素以及上面提到的*活跃工具*在监听这些事件，而主要就是那个活跃工具会负责比较重要的功能


# (未使用) Interface 模式
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