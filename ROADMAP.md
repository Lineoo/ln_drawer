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

# 是否 panic ？
这个问题针对 WorldCell 的 fetch* 家族，目前所有 fetch* 会在已借用的情况下 panic 而有一个专门的 try_fetch* 家族不 panic （但没有专门的 Error ）

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

# 为什么 interface 成为了一个 Element 而 lnwin 没有成为 Element？
这个主要是由于——事件循环。虽然窗口和事件循环并不是一一对应的，但是就对于 ln_drawer 而言，目前窗口和事件循环还是绑定在一块儿的。

但是 interface 渲染不一样，它不负责事件循环，但他需要和新生成的元素交互（创建渲染组件），这部分而言其实和我们未来要做的相交检测等功能是并列的，即元素之间的互动、更新，将 interface 也作为一个 Element 使用可以更方便我们使用。

比如说，现在 interface 保存在 lnwin 里面，我们只能在 lnwin 里面直接创建 (`self.interface.create_*`) 但是对于以后的功能，我们可能希望从 World 中的一个普通元素进行生成（想象一下按下一个按钮生成一个组件），这就要求我们能够从 World 里访问到目前的 interface 实例。且不说用 singleton 写这个有多方便，如果我们不把 interface 作为 element 实现，那我们就只能使用一个命令队列 element 把操作发送给 lnwin ，到头来还是要加一个 Element，代码逻辑也不减反增，这就完全没必要了。

而且虽然现在窗口仍然保留为一个直隶于事件循环的成员，到时候或许我们也会把它作为一个元素。刚好其实我们实际上也区分了 Lnwin（外层控制） 和 Lnwindow（真窗口），改起来就更方便了不是嘛。所以可能以后还是会把 Lnwindow 变成 Element ，留下那个 Lnwin 负责事件循环。

interface 现在在 lnwin 的逻辑（需要分离）：
1. 相机移动
    - 这个是非常好处理的，lnwin 转发了所有窗口事件所以这部分的分离很简单。
2. 屏幕空间变换
    - 问题是，world 是不负责屏幕空间的，而 lnwin 想要把用户输入转换成世界空间需要 interface 里面的 viewport。
    - 这就会破坏 world 的整洁（屏幕空间入侵到了 World 内）
    - 所以我可能希望 viewport 独立到 lnwin 这里，再被发送到 interface，这样同时也不需要迁移相机移动的代码
3. 渲染重绘
    - 没关系，一个单例解决。

# Intersect 实现
采用对应的元素直接生成一个绑定自己的 Intersection 元素（带有监听器，在主元素删除时会跟着删除）

调用 IntersectManager 的 intersect 方法时，会遍历所有的 Intersection 元素并找到 z-order 最大的那个返回。

- TODO 需要更高效率的单一类型遍历，可以带 Singleton 一起
- TODO 以后会做四叉树优化。

# 为什么 Observers 需要成为一个 Element
首先单例优化已经完成了，没有性能上的考量。

Observer 系统是 world 非常基础、底层的一个功能——但是，还是不够底层。这个不够底层说的是相对于元素插入删除、元素内部可变性而言不够底层。

如果 Observer 系统和 Element 同一级存储，那么：
- `World` 将会负责 Observer 和 Element
- `WorldCell` 不能负责 Observer，有内部可变性

那么，为什么没有一个 `WorldDeref` 的对象不负责 Observer 没有内部可变性？（实际上在作业过程中我发现我非常需要这样一个对象，但无奈再多一个结构体实在太让人抓狂了）

非常不整齐。所以这是我想要的：
- `World` 只负责 Element
- `WorldCell` 也只负责 Element，只是多了一个内部可变性

那么 Observer 放在哪里呢？
1. 我们可以新建一个 `ObservedWorld` 来存放 World 和 Observers
2. 或者把 Observers 作为一个特殊单例放进 World

综合考虑下来，我觉得还是把 Observers 作为一个特殊单例放进 World 里面。不过这应该是一个纯内部的修改。

# 有关 Queue 实现
对，我把 Queue 也变成 Element 了。这个变化已经非常理所应当了（经过上面这些实践之后），所以现在我更想说说有关 Queue 的有效性问题。

是这样的，在 Bevy 里我们有个很相似的东西叫 Commands，也是命令队列，不过有很多潜在的 panic 问题，大部分是有关在删除实体后再进行操作导致的。

所以我们希望规避这个问题，但同时也保留一些灵活性。

我们会把 queue 的操作分为两部分：
- cell 的内部操作
- 外部用户操作

其中 cell 的内部操作在队列中会优先执行，这包括了由 `WorldCellElement` 实现的 `trigger` 和 `observe` 方法。无论如何，我们希望这几个直接在 cell 中实现的方法不会报错，即**既然 WorldCellElement 存在，那么元素一定存在**，不会报错。

但同时用户的代码我们也必须允许以保留灵活性，也就是保留一个任意修改世界的 `queue` 方法。此时如果用户删除了元素后仍访问对应句柄，那就是不规范使用 Handle 导致的，跟我们 cell 就没有关系了（预期的 panic）。

至于后面可能还会实现一个 `remove` 方法，以及对于 entry 模式特定的 `destroy` 方法，这样也能保持一致性。无论是 `WorldElement` 还是 `WorldCellElement` 调用 `destroy` 方法取走所有权后都不会再有后顾之忧（不过 cell 模式下可能要标记已删除以确保不会二次获取）。

# 循环数位和相对的尺度
我们实现了无限画布——好吧其实不是无限的，毕竟受到 32 位整数限制。但是我们希望即使真的哪个神经病到达了地图边缘我们仍然能够愉快的处理这种现象，我的答案是——循环。

因为循环其实从计算机的角度来看相当合理嘛，是一种很自然的处理方式。不过目前的 rect 在处理溢出回绕时会导致负尺寸并且无法正常显示（实际上会导致三角形完全炸掉），所以为了处理循环顺便防止负尺寸，rect 我们使用左下角点的位置 + 相对的宽度高度尺寸来处理。

# 右键
右键菜单是取代目前使用 Fn 键来创建、管理组件的最佳人选。

因为我们想要跨平台跨到移动端去，所以我们就不创建新窗口来实现右键菜单了，而是在右键的瞬间在对应位置创建一个右键菜单 Element。