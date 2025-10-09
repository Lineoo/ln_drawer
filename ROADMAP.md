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
出于精度/距离效应的考量，世界的坐标使用整数像素单位存储（PhysicalPosition）。

而无限缩放则使用数值嵌套来实现。

# 可能的优化

1. 减少 CPU 到 GPU 的数据传输：使用 compute shader 在 GPU 上直接绘制
2. 事件累积和批量处理：不要每次鼠标移动都立即更新
3. 双缓冲纹理：避免读写冲突
4. 分层绘制：分离实时绘制和最终合成
5. 事件过滤：基于时间和距离阈值过滤过多的鼠标事件

# （未使用）修改器模式
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

# （已解决）是否 panic ？
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

# （已完成）observer 优化
使用 `HashMap<(ElementHandle, TypeId), SmallVec<Observer, 1>>`

# IntersectElement
既然有了 PositionedElement 的服务……这个也很正常对吧？使用 Observer 就可以很轻松的实现它。

# 有关 Service
我还没有写对应的 RFC 我就写好了 Service……说真的写完之后发现好麻烦我不喜欢这个东西……

还是简单写一下 Service 是怎么用的吧，就是说，它允许将某个 Element 以任意形式读取。比如原本我们只能获取 `dyn Element` 类型或者原类型 `T`，有了 Service 之后我们就可以同时读取它注册好的类型 `dyn PositionElement` 或者其他类型 `U: ?Sized`

之后那个 `service` 和 `service_mut` 函数我们可能就直接集成到 `fetch` 家族里面去，这样 cell 模式下我们也能够使用，也可能更加简单方便。

那么 `contains_type` 方法就应该另有一个变体 `contains_type_raw` 来判断原始类型

# 父子依赖关系
在句柄模式下尤为重要，需要保证对方永远处于有效状态。

- ~~父对象 A，子对象 B~~
- ~~子对象在初始化时 `depend` 生成了两个 Observer~~
    - ~~第一个 E，监听 A，在 A 被删除时删除 B，清理时会删除 E 和 F~~
    - ~~第二个 F，监听 B，在 B 被删除时删除 E，清理时会删除 F~~
- ~~当 Observer 被删除时会自动清理索引~~

~~depend 互依赖也是可行的，删除任意一个都会删除对方~~

~~如果单独删除 E 或者 F 则会导致世界无效状态，所以 E 和 F 的句柄需要保持内部。~~

我完全把目前系统的 Observer 想错了……因为删除的时候的那个 ElementRemoved 是*全局广播*的！
我完全只需要把 Observer 挂在自己下面即可！

嗯……没错 Observer 对自己监听的对象算是有一个自带的 depend。这是用 Observer 实现依赖的关键。不过如果你重复添加了一个 depend 的话倒也不会有很大问题。

# 拖曳拖曳拖曳手柄
我！要！——把！所有！东西！——全部！！变成 Element ！！！啊！啊！啊！！！！

目前 Intersect 的拖拽使用 Service 转化 `dyn PositionElement` 和 `Intersection` 来进行，但是使用 Service 没有办法做出多碰撞箱（Service 唯一性），同时 Intersection 的位置同步更新也是有一点 `unwrap` 灾难的。

所以，我觉得首先可以把 Intersection 也变成 Element 保存，再使用 Service 系统更改 `dyn PositionElement`，并在更改后时候发出信号 `PositionChanged` 并由下面 Intersection 的 Observers 来检测进行位置更新

# cell 下的即时更新
目前 cell 不支持结构性更改。但是就像 Bevy 使用 `Entities` 堆来提前返回 Entity 指针一样，我们在 cell 模式下也可以专门维护一个 handle 状态，这样就可以支持 insert 的同时完成即时返回，别的工作放到 queue 里完成。而且就像之前说的，remove 的实现也可以同样。

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

# World 继续升级
- remove 功能
- 在 remove 的基础上添加原生的 depend 指令
- cell 模式的 insert 和 remove 等结构性功能
- entry 模式下的 destroy 指令
- observe 指令支持返回对应的 handle
- cell 模式的自定义 queue 命令
- cell 模式的 contains 查询

# intersect 功能划分
Intersect 包括了：
- 相交检测/优化
- Intersection 碰撞体注册

不包括：
- 选择/移动物体 pointer
- 右键菜单 Menu / Tooltip
- 聚焦物体与输入 Focus / TextEdit

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

其实本身第二种写法是挺好的，比第一种效率要高，也是 observer 的正统用法，但是问题其有效性……需要一个额外的 observer 的 depend 来做有效性保证。看上去太丑了！

而且有关 `ElementInserted` 最难搞的其实是我不知道它应该挂载在哪里（

所以就有了这次更新最破坏性的更改：`World::trigger` 不再是遍历触发了，实际上 World 的 observer 相关代码现在就只是挂载到了 Element#0 （即 Observers 内部组件）上了。

~~但是看上去仍然很丑。所以我让 observer 能够找到自己（并且可以摧毁自己）~~因为破坏性太大而取消了计划。

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

也可以匿名：
```rust
impl Text {
    pub fn new(text: String) -> impl Inserter {
        move |world| {
            let interface = world.single_mut::<Interface>().unwrap();
            interface.create_painter(/* .. */);
            /* .. */
        }
    }
}
```

问题是这种写法会积极阻止 non-Element 场景的应用，也就是 text 无法被单独直接管理，所以还有待商榷吧。

# 有关 Text 的移动
主要是因为 Text 的*交互*问题。因为鼠标交互是只存在于 element 层面的，如果为了 hit 支持而编写一大堆 glue code 那才是真得要命。再加上其实原先 text 也没有和 wgpu 有多么多么深厚的交互（基本还是使用 painter 完成的），所以干脆就不要了。

然后就是有关其他地方对 text 的引用了，虽然 text 现在确实是元素了，但是这并不意味着（也从不意味着）你一定得把它插入到 World 中才能使用。

# Element 的创建
上面提到了 non-Element 场景。我们鼓励这种场景下的使用！所以推荐的写法：
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

# （废）依赖更改
目前依赖是实现了的（通过 `depend` 方法），但是问题是——太丑了！entry 模式绝对还可以添加一点东西。

不好看：
```rust
let parent = world.insert(Parent);
let child = world.insert(Child);
world.entry(child).unwrap().depend(parent);
```

好看：
```rust
let parent = world.entry_with(Parent);
parent.insert(Child); // Implicit dependence
```

对于 observer 也适用：
```rust
let parent = world.entry_with(Parent);
parent.observe(move |ElementUpdate, world| {
    // Observer 会被挂载到 parent 下而非 Element#0 下
    // 其实就是正常的 entry 操作
    parent.update();
});
parent.entry(other).unwrap().observe(move |ElementUpdate, world| {
    // Observer 会被挂载到 other 下，但是会在 parent 被删除时同步删除
    parent.sync_with(other);
});

// 这种写法也是有用的，但是我看着是真的很别扭
// 正常来说应该是像 parent.entry(ElementHandle(0)).unwrap().observe(..)
// 但是这个操作显而易见的危险，所以我也不知道怎么办
parent.observe_root::<WindowEvent>(move |event, world| {
    // Observer 会被挂载到 Element#0 下但依赖于 parent
    // 然后我觉得 World 的那个方法也应该改成对应的 observe_root 然后没有 observe
    // trigger 改成 trigger_root
    match event {}
});
```

最后可以返回 handle:
```rust
let parent = world.entry_with(Parent);
parent.insert(Child);
let parent: ElementHandle = parent.finish();
```

还可以链式调用（注意依赖只有单层）：
```rust
let grandparent = world.entry_with(GrandParent);
let parent = grandparent.entry_with(Parent);

let boy = parent.entry_with(Boy);
boy.insert(Toys);
boy.finish();

let girl = parent.entry_with(Girl);
girl.insert(Toys);
girl.finish();
```

# 依赖更改 II
上面的写法其实问题很大：
- A 的 entry 下允许再次调用 entry 索引 B，B 显然并不依赖于 A，但生成的 observer 又依赖于 A
- 其实那个 observer 写法是歧义的，根本不可能实现
- 对 entry 有很大的破坏性更改

除了 `entry_with` 的写法完全可以保留，其他的都有以上的问题。

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

```rust
let span: Span<'_, World> = world.span(move |new_element: WorldEntry| new_element.depend(parent));
```

同时，依赖应该不再依靠 Observer 实现，因为 Observer 本身需要依赖，故极易造成递归死循环。我们全部换成独立的实现，这样也可以使 Observer 更加自由。

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

接口 port

# 高级 Service 逻辑与附生元素
想要让 String 成为一个 Element 的想法是非常自然的不是吗？我可以简单地添加一个 String 到世界里，然后就可以显示它，非常直观而简单。

```rust
world.observe(|&ElementInserted(handle), world| {
    let text = world.fetch::<Text>(handle); // 在别人看来就是一个文本组件
});
world.insert(String::from("Hello, world!"));
```

可以实现的：
```rust
impl Element for String {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        let mut this = world.entry(handle).unwrap();
        let text = world.single_mut::<Interface>().unwrap().create_text(/* .. */);
        this.register::<Text>(move |this| {
            // ..
        });
    }
}
```

# wgpu 集成深度
目前对于 wgpu 我们采用了最小集成，即整个世界只有 Interface 这么一个元素进行交互。我觉得挺好。

# World for Element
世界是一个 Element 显然也是合理的吧？！子世界嘛！

# 高级数据更改 
我希望我改完 Position 之后不用我提醒，反正你知道我改了，更新就完事了。

你只要：
```rust
this.register::<Position>(|this| &this.rect.position);
```

他只要：
```rust
this.observe::<Modified<Position>>();
```

我只要：
```rust
let position = this.modify::<Position>(handle);
position += Delta::new(10, 10);
```

就都能获得更新。

# Service 的 getter 和 setter

使用事件系统进行更改
```rust
this.trigger(Modifier::new(|item: Position| item + 1));
this.trigger(Modifier::new(|_| Position::new(1, 2)));
```

内源读取
```rust
this.observe::<Modifier<Position>>(move |modifier, world| {
    let this = world.fetch_mut(handle).unwrap();
    this.origin = modifier.invoke(this.origin + Delta::splat(1)) - Delta::splat(1);
})
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
