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

# 世界
出于精度/距离效应的考量，世界的坐标使用整数像素单位存储（PhysicalPosition）。

而无限缩放则使用数值嵌套来实现。

# 可能的优化

1. 减少 CPU 到 GPU 的数据传输：使用 compute shader 在 GPU 上直接绘制
2. 事件累积和批量处理：不要每次鼠标移动都立即更新
3. 双缓冲纹理：避免读写冲突
4. 分层绘制：分离实时绘制和最终合成
5. 事件过滤：基于时间和距离阈值过滤过多的鼠标事件

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

# 有关 Queue 实现
对，我把 Queue 也变成 Element 了。这个变化已经非常理所应当了（经过上面这些实践之后），所以现在我更想说说有关 Queue 的有效性问题。

是这样的，在 Bevy 里我们有个很相似的东西叫 Commands，也是命令队列，不过有很多潜在的 panic 问题，大部分是有关在删除实体后再进行操作导致的。

所以我们希望规避这个问题，但同时也保留一些灵活性。

我们会把 queue 的操作分为两部分：
- cell 的内部操作
- 外部用户操作

其中 cell 的内部操作在队列中会优先执行，这包括了由 `WorldCellElement` 实现的 `trigger` 和 `observe` 方法。无论如何，我们希望这几个直接在 cell 中实现的方法不会报错，即**既然 WorldCellElement 存在，那么元素一定存在**，不会报错。

但同时用户的代码我们也必须允许以保留灵活性，也就是保留一个任意修改世界的 `queue` 方法。此时如果用户删除了元素后仍访问对应句柄，那就是不规范使用 Handle 导致的，跟我们 cell 就没有关系了（预期的 panic）。

# 循环数位和相对的尺度
我们实现了无限画布——好吧其实不是无限的，毕竟受到 32 位整数限制。但是我们希望即使真的哪个神经病到达了地图边缘我们仍然能够愉快的处理这种现象，我的答案是——循环。

因为循环其实从计算机的角度来看相当合理嘛，是一种很自然的处理方式。不过目前的 rect 在处理溢出回绕时会导致负尺寸并且无法正常显示（实际上会导致三角形完全炸掉），所以为了处理循环顺便防止负尺寸，rect 我们使用左下角点的位置 + 相对的宽度高度尺寸来处理。

# 右键
右键菜单是取代目前使用 Fn 键来创建、管理组件的最佳人选。

因为我们想要跨平台跨到移动端去，所以我们就不创建新窗口来实现右键菜单了，而是在右键的瞬间在对应位置创建一个右键菜单 Element。

# 有关 Service
我还没有写对应的 RFC 我就写好了 Service……说真的写完之后发现好麻烦我不喜欢这个东西……

还是简单写一下 Service 是怎么用的吧，就是说，它允许将某个 Element 以任意形式读取。比如原本我们只能获取 `dyn Element` 类型或者原类型 `T`，有了 Service 之后我们就可以同时读取它注册好的类型 `dyn PositionElement` 或者其他类型 `U: ?Sized`

之后那个 `service` 和 `service_mut` 函数我们可能就直接集成到 `fetch` 家族里面去，这样 cell 模式下我们也能够使用，也可能更加简单方便。

那么 `contains_type` 方法就应该另有一个变体 `contains_type_raw` 来判断原始类型

# Commit 格式
我发现 commit 的分类我完全就是在乱写（

以后这么个格式：首先我还是用中文写（因为我是中！国！人！），然后模块开头，如果是优化或者修复在用点号分隔加上，然后再是标题。

比如：
- `image.feat: 图像模块`
- `label.remove`
- `button: 小修改`
- `world.fix: 某个 BUG`
- `mixed.clean: 清理代码`
- `proj.ROADMAP`
- `ver: v0.1.0-alpha4`

# observer 优化
现在我们推荐 observer 的正统用法。这意味着以下写法是不推荐的：
```rust
struct ElementUpdate(ElementHandle);
listener.observe::<ElementUpdate>(/* .. is that thing updated? */);
world.trigger(ElementUpdate(handle));
```

而以下是推荐的:
```rust
struct ElementUpdate;
let obv = that.observe::<ElementUpdate>(/* .. I need to send it back to the listener .. */)
(world.entry(obv).unwrap()).depend(listener);
that.trigger(ElementUpdate);
```

# 编写简化
1. `Handle` 类型和 `Fetchable`
我不是类型疯批，但是简单地划分 `Handle` 功能或许是不错的事情。
- trait: `Fetchable`, `Fetchable::Output`
- `StrongHandle<T = dyn Element>` -> `T` or panic!
- `Handle<T = dyn Element>` -> `Option<T>`

2. `Event<E>` 类型
对 observer 的 event 使用 Deref 进行一点封装，并实现：
- observer 能够获取自己的 handle

3. `Inserter`
```rust
impl Text {
    pub fn new(text: String) -> TextDescriptor {
        /* .. */
    }
}
impl Inserter for TextDescriptor {
    fn insert_with(self, world: &mut World) {

    }
}
```

# 有关 Text 的移动
主要是因为 Text 的*交互*问题。因为鼠标交互是只存在于 element 层面的，如果为了 hit 支持而编写一大堆 glue code 那才是真得要命。再加上其实原先 text 也没有和 wgpu 有多么多么深厚的交互（基本还是使用 painter 完成的），所以干脆就不要了。

然后就是有关其他地方对 text 的引用了，虽然 text 现在确实是元素了，但是这并不意味着（也从不意味着）你一定得把它插入到 World 中才能使用。

# Element 的创建
我们鼓励 non-Element 场景！推荐的写法：
```rust
impl Foo {
    pub fn new(property: Property, interface: &mut Interface) -> Foo {
        /* .. */
    }
}
```

不推荐的写法：
```rust
impl Bar {
    pub fn new(property: Property, world: &WorldCell) -> Bar {
        /* .. 使用 &mut World 也是同理 .. */
    }
}
```

```rust
impl Baz {
    pub fn new(property: Property) {
    }
}
impl Element for Baz {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        // 过度包揽了自己不该干的活儿
        let interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_painter(/* .. */);
        // 不应该存在 inner 为 None 的非法状态！（除非有特定使用场景）
        self.inner = Some(inner);
    }
}
```

# Element 清理
既然 interface 也是 Element，是不是应该把 interface 也放进 elements 里面？

啊当然不是。其实如果 interface 也放在里面的话，我甚至觉得整个程序放在 elements 里也没问题（

所以我们会慢慢地把 elements 里面的东西，相反的，挪*出来*。

- interface: 所有跟 wgpu 有关的代码
- text: 所有跟 cosmic-text 有关
- tools: 有关用户输入处理
- widgets: 预设的用户组件

# 有关完全所有权下的世界更改
有时候我们只能获得元素的 `&mut self` 字段而无法获得 `World` 的权限。这种时候如果我们想要改变世界内容（添加 Observer 等）怎么办？

`mpsc` 是一个很简单、直观的解决方案：
```rust
pub fn new(queue: &mut WorldQueue) -> Foo {
    Foo { tx: queue.sender() }
}
```

但是怎么让世界能够读取这个队列是一个很重要的事情。

# 依赖更改
Group 写法：
```rust
let parent: Group<'_, World> = world.entry_with(Parent).group(); // 隐式添加一个对 ElementInsert 的 Observer
parent.insert(Child); // 使用 Deref
// drop 的时候移除 Observer
```

```rust
let parent = world.insert(Parent);
let parent: Group<'_, World> = world.entry(parent).group();
parent.insert(Child);
```

Span 写法：
```rust
let span: Span<'_, World> = world.span(move |new_element: WorldEntry| new_element.depend(parent));
```

有关 Observer 自删除后的清理，我们应让 Observer 保存自己的侦测数据以便索引。

# 数据持久化
extension: ln-save

Windows: %AppData%/Roaming/LnDrawer/world.ln-save
Linux: $XDG_DATA_HOME/LnDrawer/world.ln-save

# 输入输出与数据格式
怎么说，我很喜欢 MC 模组的各种输入输出（

我们把一个交互界面分为 6 种
- 主动（输入）
- 主动（输出）
- 被动（输入）
- 被动（输出）
- 被动（双工）
- 禁用

输入输出的东西也是元素 Element。

接口即需要实现 Port 这个 Service

# wgpu 集成深度
目前对于 wgpu 我们采用了最小集成，即整个世界只有 Interface 这么一个元素进行交互。我觉得挺好。

# World for Element
世界是一个 Element 显然也是合理的吧？！子世界嘛！

# Property & Modify
依赖 Service 提供类型化服务

独立的更改结构：
```rust
// 'static
let mut modify: Modify<Position> = entry.modify::<Position>().unwrap();
let position = modify.get();
modify.set(position + Delta::splat(1));

// 使用 DerefMut
modify += Delta::splat(-1);

// 生效并触发 ModifiedProperty<Position> 事件
modify.flush(world);
```

配置：
```rust
entry.property::<Position>(|this|, setter);
```

# 类型化的 entry
依赖 Service 和 Insert 重构

```rust
impl InsertWorld: Element {
    fn before(world: &mut World) {

    }

    fn after(entry: WorldEntry<Self>) {

    }
}
```

# 更直观地写
```rust
world.observe(|event, world: &WorldCell| {
    // ... //
})
element.observe(|event, element: WorldCellEntry| {
    // ... Why not? ... //
})
```

深思熟虑。
```rust
fn when_inserted(&mut self, entry: WorldCellEntry) {
    let handle: ElementHandle = entry.handle();
    let world: &WorldCell = entry.world();
}
```

# v0.2 的内容
- crate 架构 - lib 独立
- lnwin 的窗口实现为 Element 
    - 同时把 world 的 observe 删掉
- Observers 和 Services 作为 Element 泛型单例而不是使用 TypeId Map
    - 不再 Internal Element
- InsertWorld
- 类型化 entry