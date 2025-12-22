> NOTE: This page is written in Chinese.

# 待办事项 #
- [x] 右键菜单
    - [x] 菜单项目
    - [x] 菜单显示
        - [x] SDF 圆角矩形
        - [x] 浮点渲染
- [x] 交互操作改进
    - [x] 操作层级统一
        - [x] 使用 `when_insert` 和 `depend` 来注册操作元素
            - [x] 删除 `Service` 和 `Property` 并保持 `world` 模块整洁
            - [x] 删除各种 Entry, Other 写法
            - [x] (trait) ElementDescriptor 用于直接在世界新建 Element，自动获取 interface 等等的资源
        - [x] 无限大操作区域
    - [x] Alt 取色
        - [x] 更新调色盘
        - [x] 选定色彩和调色盘分离
    - [x] 右键元素并删除
- [x] 更新 ROADMAP.md
- **LnDrawer v0.1.1-alpha2**
- [x] 变换工具
    - [x] 变换接口元素
    - [x] 位置移动
    - [x] 矩形调整
- [x] 序列化保存 & 加载
- [x] 笔刷层的保存
- **LnDrawer v0.1.1-alpha3**
- [x] 统一跨端字体
- [x] 高精度相机位移、缩放
- [x] 相机缩放逻辑调整
- [x] 触摸、触控板的缩放、平移手势支持
- **LnDrawer v0.1.1**
- [x] 任意位置命令
- [x] 重写 Interface
    - [x] 使用世界元素来注册 Interface
- [x] 文字的高精度渲染
    - [x] 编写专用的文本渲染管线
- [x] 删除 ZOrder
- [x] Rectangle 使用 Size
- [x] 清理 viewport 逻辑
- [x] 支持透明窗口
- [ ] 把窗口事件转交给各个模块
    - [x] 分离指针交互
    - [x] 分离相机移动逻辑 `tool/camera.rs`
    - [x] ModifiersTool
- [ ] viewport 改成元素
- [ ] menu 创建跟随
- [ ] `trigger` 改为立即触发并返回观察者数量
- [ ] 删除 Delta
- [ ] 修复 Rectangle 的 clamp
- [ ] PointerEdgeCollider
- [ ] 修复首帧未定义的表面
- [ ] TextEdit 重新制作
    - [ ] Focus 由元素自己主动请求
- [ ] 修复"无限大矩形"精度问题
- [ ] 修复变换工具的一系列问题
- [ ] 选择工具优化
- **LnDrawer v0.1.2-alpha1**
- [ ] 可修改的圆角大小
- [ ] 圆角描边与阴影
- [ ] 菜单分割线
- [ ] 浮动信息框 Toast
- [ ] 动画系统
- **LnDrawer v0.1.2-alpha2**
- [ ] 设置界面
- [ ] 调试系统
    - [ ] 观察者与事件流显示
    - [ ] 依赖关系显示
    - [ ] 自定义属性显示
- [ ] 简单噪音播放器
    - [ ] 开关按键
    - [ ] 音频库
    - [ ] 噪声生成
- [ ] 音乐播放器
    - [ ] 用户界面
- **LnDrawer v0.1.2**
- [ ] 完整生命周期管理
- [ ] 基于生命周期的缓冲
    - [ ] 相交缓冲
    - [ ] 渲染缓冲
    - [ ] Aabb 与渲染剔除
- [ ] 曲率连续圆角
- [ ] 用于数位板/笔的代替控制按键
- [ ] 超级传送门
- [ ] 几个单位的分数进位 & 整数环绕
- [ ] Dependency 和 Observer 一样使用并返回 Handle
    - [ ] `coexist` 方法循环绑定组（并返回 `Handle<Coexist>` ）
    - [ ] 公开 Dependency 和 Observer 类型
- [ ] Observer & Dependency 清理
    - [ ] Element 的 `when_remove` 动作
- [ ] Any 观察者
- [ ] 使用 `Attaches` 简化元素对应
- [ ] Palette & TextEdit 纹理更新时占用太大 - 缓冲鼠标输入
- [ ] Palette 单独着色
- [ ] 拖放图片
- [ ] 有限-无限工具包
    - [ ] 转换器
    - [ ] 可视时生成器
- [ ] 备忘录
- [ ] 日历
- [ ] 元素编组
- **LnDrawer v0.1.3**
- [ ] 圈选工具
    - [ ] 移动
    - [ ] 绘制限制
- [ ] 画布工具
    - [ ] 临时瓦片平铺（或者其他格式显示）
    - [ ] 选择保存为 .png
    - [ ] 简单网格化
    - [ ] 动画帧
- [ ] 吸色实时显示颜色
- [ ] 分型画板
- [ ] 更多画板工具
    - [ ] 按钮列
        - [ ] 按钮图片显示
        - [ ] 单选组
    - [ ] 画板工具状态机
        - [ ] 画笔按钮
    - [ ] 全 Alpha 区块垃圾清理
- [ ] 世界锁模式 Panic, Block, Invisible
- [ ] async 异步支持
- **LnDrawer v0.2.0**

# 技术细节 #

## 为什么呢 ##

### 为什么 interface 成为了一个 Element 而 lnwin 没有成为 Element？

这个主要是由于——事件循环。虽然窗口和事件循环并不是一一对应的，但是就对于 ln_drawer 而言，目前窗口和事件循环还是绑定在一块儿的。

但是 interface 渲染不一样，它不负责事件循环，但他需要和新生成的元素交互（创建渲染组件），这部分而言其实和我们未来要做的相交检测等功能是并列的，即元素之间的互动、更新，将 interface 也作为一个 Element 使用可以更方便我们使用。

比如说，现在 interface 保存在 lnwin 里面，我们只能在 lnwin 里面直接创建 (`self.interface.create_*`) 但是对于以后的功能，我们可能希望从 World 中的一个普通元素进行生成（想象一下按下一个按钮生成一个组件），这就要求我们能够从 World 里访问到目前的 interface 实例。且不说用 singleton 写这个有多方便，如果我们不把 interface 作为 element 实现，那我们就只能使用一个命令队列 element 把操作发送给 lnwin ，到头来还是要加一个 Element，代码逻辑也不减反增，这就完全没必要了。

而且虽然现在窗口仍然保留为一个直隶于事件循环的成员，到时候或许我们也会把它作为一个元素。刚好其实我们实际上也区分了 Lnwin（外层控制） 和 Lnwindow（真窗口），改起来就更方便了不是嘛。所以可能以后还是会把 Lnwindow 变成 Element ，留下那个 Lnwin 负责事件循环。

### 为什么 trigger 要改成即时的？

不占用队列。大部分请求是不需要关照 `insert` 和 `remove` 的延迟执行的。

同时若真的有需要，也只需要 `world.queue(|world| { world.trigger(event); });` 即可。

## 循环数位和相对的尺度 ##

我们实现了无限画布——好吧其实不是无限的，毕竟受到 32 位整数限制。但是我们希望即使真的哪个神经病到达了地图边缘我们仍然能够愉快的处理这种现象，我的答案是——循环。

因为循环其实从计算机的角度来看相当合理嘛，是一种很自然的处理方式。不过目前的 rect 在处理溢出回绕时会导致负尺寸并且无法正常显示（实际上会导致三角形完全炸掉），所以为了处理循环顺便防止负尺寸，rect 我们使用左下角点的位置 + 相对的宽度高度尺寸来处理。

## Commit 格式 ##

用中文写，模块开头，优化、修复等用点号分隔，最后标题。

比如：
- `image.feat: 图像模块`
- `label.remove`
- `button: 小修改`
- `world.fix: 某个 BUG`
- `mixed.clean: 清理代码`
- `proj.ROADMAP`
- `ver: v0.1.0-alpha4`

## Observer & Trigger 系统综述 ##

我们推荐 observer 的正统用法。这意味着以下写法是不推荐的：
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

## Element, non-Element 和 Descriptor 模式 ##

推荐简单、解耦的写法：
```rust
impl Foo {
    pub fn new(property: Property, interface: &mut Interface) -> Foo {
        /* .. */
    }
}
```

不推荐直接获取整个世界的写法：
```rust
impl Bar {
    pub fn new(property: Property, world: &World) -> Bar {
        /* .. 明明你只需要 Interface! .. */
    }
}
```

同样不推荐使用 `Option<T>` 的写法：
```rust
impl Baz {
    pub fn new(property: Property) -> Bar {
        /* .. */
    }
}
impl Element for Baz {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        // 过度包揽了自己不该干的活儿
        let interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_painter(/* .. */);
        // self.inner 为 None 的状态只在初始化时存在，很别扭
        self.inner = Some(inner);
    }
}
```

如果有对应的描述器，也推荐如下写法：
```rust
struct BazDescriptor {
    property: Property
}
impl ElementDescriptor for BazDescriptor {
    type Target = Baz;
    fn prepare(self, world: &WorldCell) -> Self::Target {
        // 描述器专门用于从世界中提取数据进行构建
        let interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_painter(/* .. */);
        // 没有非法状态
        Baz { inner }
    }
}
```

## Element 重构 ##

既然 interface 也是 Element，是不是应该把 interface 也放进 elements 里面？

啊当然不是。其实如果 interface 也放在里面的话，我甚至觉得整个程序放在 elements 里也没问题（

所以我们会慢慢地把 elements 里面的东西，相反的，挪*出来*。

- interface: 所有跟 wgpu 有关的代码
- text: 所有跟 cosmic-text 有关
- tools: 有关用户输入处理
- widgets: 预设的用户组件

## 元素编组 ##

元素编组主要是为了解决管理多元素交互，可见性，权限管理与元素分层等需求。

比如单选框就是这个功能非常典型的应用：只需简单地将同组的其他单选框取消选择，即可实现一定范围内的单选。

### 方案一：使用状态切换

使用 `Group` 和 `group()` 函数来指定使用的编组：

```rust
// 这是 World 默认使用的编组
world.group_switch(Group::default());
```

更多时候会可能会希望进行临时切换：

```rust
// 会在离开时触发 Drop
let _guard: GroupGuard<'_> = world.group(Group::default());
```

接下来的 `single` 和 `foreach` 指令都会使用该 group。

### 方案二：使用编组元素 + 默认编组 shortcut

优点是使用独立编组元素，相比起方案一，侵入性更小，也更加统一。

缺点是没有原生实现，进而引入自指涉/初始化问题，而 `single` 和 `foreach` 等函数会变成单纯的 shortcut。

```rust
let group = world.insert(Group::default());
let group = world.group(group);
group.single_fetch::<Foo>();
```

### 方案三：完全独立仅元素

类似方案二，但是完全独立，不对 World 原有逻辑作任何修改。

没有自指涉，没有初始化，一切都很简单。

更加精简，而世界层面的指令就类似 root 一样，一定可以看到所有东西。

而且多亏独立与 World 的访问器以及无需 shortcut 的统一层级，我们可以获得额外的 `fetch` 控制。

缺点是元素的启用/禁用等功能要求手动进入默认组，不然就是默认看得到所有东西，这会导致代码量增多。

```rust
let group = world.insert(Group::default());
let view: GroupView<'_> = world.view(group);
view.fetch(foo);
```

## 新的渲染流程 ##

目前的渲染流程控制关系复杂且难以拓展。

### 1. 概述

渲染分为多个部分：
1. **渲染控制** - 控制渲染组件的排序、剔除、可见性，负责 Surface，Viewport 等
2. **渲染管线** - 负责记录绑定组，管线布局，Shader 等
3. **渲染组件** - 负责实际的绘制

对于渲染组件，可用的生命周期：
1. **初始化**
2. **渲染绘制**
3. **修改同步**
4. **移除**

我们会采用**任意位置命令**进行**全生命周期的渲染组件跟踪**。

### 2. 初始化

管线相关初始化此处省略。此章重点规范**独立渲染组件的初始化**。
> 独立渲染组件是**不直接被插入世界的渲染组件**，它们依赖*描述构建模式*来注册进渲染控制。

```rust
struct Panel {
    rectangle: RoundedRectangle,
}

world.insert(Panel {
    rectangle：world.build(RoundedRectangleDescriptor {
        position: Position::new(1, 2),
        ..Default::default()
    }),
});
```

渲染组件首选**描述构建模式**来初始化渲染实例。过程：
1. 初始化并获取**对应的渲染管线**
2. 注册对应的 **GPU 实例节点**，如需要的 Buffer 等
3. 在世界中生成**渲染控制节点**并完成注册
4. 在世界中完成**生命周期追踪**，主要是观察者和对象依赖

也就是说，插入了三种元素：
- 核心元素 `Panel`
- GPU 实例节点 `RoundedRectangleBuffer`
- 渲染控制节点 `RenderControl`

### 3. 渲染绘制

渲染命令从事件循环出发，并由 `lnwin` 转移给**渲染控制系统**。

1. 解析所有渲染控制节点，进行排序、剔除
2. 按序遍历所有节点并触发 `RedrawPrepare` 事件
3. 按序遍历所有节点并触发 `Redraw` 事件

渲染控制节点是一种**附属节点**，给渲染控制系统提供控制信息和一个事件触发点。

```rust
let control = world.insert(RenderControl {
    clip: None,
    z_order: ZOrder::new(50),
    ..Default::default()
});

world.observer(control, move |Redraw(rpass), world| {
    let instance = world.fetch_mut(instance).unwrap();
    instance.redraw(rpass);
});
```

### 4. 修改同步 & 删除

在核心元素修改后，我们需要同步将其上传到 GPU；删除元素后，也需要及时通知。
> **不能依赖核心元素**的生命周期事件，因为两者的生命周期不一定相同。（比如使用了 `Option<T>` 时）

用下文的**任意位置命令**可以利用 `mpsc` 来隐藏对应逻辑。

```rust
panel.rectangle.location = Rectangle::new(0, 0, 100, 100);
panel.rectangle.upload();
```

### 5. 模式选择理由

为什么使用**任意位置命令**并把组件内嵌而不是直接使用 `Handle<T>` + 生命周期事件？
- 一般情况下，元节点和渲染组件这两者不是互为*关联*，而是前者*拥有*后者，持有其所有权

### 6. Viewport?

Viewport 在这个语境下其实是两个东西：
1. wgpu 负责绘制的那个 viewport，是作用在**窗口**上的
2. 我们内部给 `render` 模块用的那个 viewport，用来平移用的

两者的确是对应的（一般来说会保持两个的 width 和 height 相同），但逻辑上来讲，
- 一个是**世界空间**（或者说可以用 `Size` 来表示）
- 一个是**屏幕空间**（只能用 `[u32; 2]` 来表示）

## 任意位置命令 ##

允许获取世界的命令队列，然后**从任何地方直接发送命令**到世界。

可以简化组合元素的更新与清理。

## 无头组件 ##

**无头组件 Headless Widget** 是解耦很重要的一步。而 `ln_drawer` 原生的事件系统让这变得相当自然。

## 原则 & 规范 ##

### 1. 平移一致性

在不同的位置，所有组件应当表现出一致的行为。

特指渲染处理。

### 2. 行为完整性

任意元素的任意功能都应当能够**独立于世界环境存在**，即能够被简单地通过 newtype 模式进行功能拓展。

**标记元素**只能在世界中起效，但实际与该原则不冲突。标记元素的功能实现**必须由其对应工具类和观察者模式实现**。
- 比如 `Transform` 其实是**不附带功能**的
- 相反，所有的功能实现是由 `TransformTool` 和创建 `Transform` 的元组件监听 `TransformUpdate` 事件来实现的
- 优势在于这种**类即时模式**避免了生命周期处理，但是劣势是带来了**性能下降**（显然的）
