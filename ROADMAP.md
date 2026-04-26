> NOTE: This page is written in Chinese.

# Commit 格式 #

用中文写，模块开头，优化、修复等用点号分隔，最后标题。

比如：
- `image.feat: 图像模块`
- `label.remove`
- `button: 小修改`
- `world.fix: 某个 BUG`
- `mixed.clean: 清理代码`
- `proj.ROADMAP`
- `ver: v0.1.0-alpha4`

# 世界模块 #

世界模块提供**多元素响应**能力，这包括同时访问多个元素，用事件串联多个元素，以及多元素依赖处理。

## Element & Descriptor ##

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
        let interface = world.single_fetch_mut::<Interface>().unwrap();
        let inner = interface.create_painter(/* .. */);
        // self.inner 为 None 的状态只在初始化时存在，很别扭
        self.inner = Some(inner);
    }
}
```

Descriptor 模式**不暗示世界外使用**，真正暗示世界外使用的是 **Descriptor 不返回 `Handle<T>` 而直接返回 `T`**。

如果有对应的描述器，也推荐如下写法：
```rust
struct BazDescriptor {
    property: Property
}
impl ElementDescriptor for BazDescriptor {
    type Target = Baz;
    fn prepare(self, world: &WorldCell) -> Self::Target {
        // 描述器专门用于从世界中提取数据进行构建
        let interface = world.single_fetch_mut::<Interface>().unwrap();
        let inner = interface.create_painter(/* .. */);
        // 没有非法状态
        Baz { inner }
    }
}
```

## 延迟执行 insert 和 remove 的生命周期 ##

我们遵循生命周期最小的原则：
- 在 insert 后无法 fetch
- 在 insert-flush 后正常 fetch
- 在 remove 后立刻无法 fetch
- 在 remove-flush 后当然无法 fetch

我们极力避免出现生命周期交叉！（图中没有画 queue 有关的，但是也应该尽力保证不交叉）
```text
/--- insert
|    insert-flush ----=\
|    insert-event      |  >==\  ===> where `when_insert` runs (after insert-flush)
|    ...               |     |
|    modify            |     |
|    modify-event      |     |  ===> where `when_modify` runs (after modify)
|    ...               |     |
|    bind-deps         |     |
|    ...               |     |
|    remove-event      |  >==/  ===> where `when_remove` runs (before remove)
|    remove-deps       |   ^^^
|    remove       ----=/   element-trait hook lifecycle
\--- remove-flush    ^^^
^^^                  fetch-available lifecycle
actual ownership lifecycle      ===> where `drop` runs
```

### 生命周期事件

之前是有一个 `Destroy` 事件的，但是被 `when_remove` 代替了。

完全取消生命周期事件的原因很简单——**减少隐式逻辑**。
- 提供一个统一的生命周期事件会导致任何元素都会能够读取同种生命周期事件而不加区分，往往会导致难以分离的循环调用问题
- 使用 hook 模式可以将事件触发下降到高级逻辑层面，这可以限定其作用范围，缩小调试范围并提供更稳定的代码

### `Ref` 与 `RefMut` 的 `.handle()` 函数

觉得样子有点奇怪，不过实现上不困难。

```rust
let fetched = world.fetch(this).unwrap();
fetched.perform();
world.observer(this, |/* */| { /* .. */ });
```

```rust
let this = world.fetch(this).unwrap();
this.perform();
world.observer(this.handle(), |/* */| { /* .. */ });
```

## Observer & Trigger  ##

我们推荐 observer 的正统用法。这意味着以下写法是不推荐的：
```rust
struct ElementUpdate(ElementHandle);
world.observer(listener, |ElementUpdate| /* .. is that thing updated? */);
world.trigger(listener, ElementUpdate(that));
```

而以下是推荐的:
```rust
struct ElementUpdate;
world.observe(that, |ElementUpdate| /* .. send it back to the listener .. */)
that.trigger(that, ElementUpdate);
```

### 为什么 trigger 要是即时的？

减少队列的侵占，因为其设计**不符合人体工程学**。同时，大部分请求是不需要关照 `insert` 和 `remove` 的延迟执行的。
> 若确实需要*处理延迟执行*或*避免循环访问*，请使用：
> ` world.queue(|world| { world.trigger(event); }); `

## 元素编组 ##

元素编组主要是为了解决管理多元素交互，可见性，权限管理与元素分层等需求。

比如单选框就是这个功能非常典型的应用：只需简单地将同组的其他单选框取消选择，即可实现一定范围内的单选。

API 部分
- World 接口: subview(), enter()
- 行为：insert 会将元素插入到顶端元素，fetch 等命令会取并集筛选
- 结构体: WorldView, Entrance
- Entrance: 配置目标视角组，是否丢弃当前活跃的 view，是否进入 结构只读/完全只读模式
- 通过 arc 实现多重引用，可以在不同的地方呈现不同的 view
- World 和 WorldInner, flush 因为需要 arc 回收所以只能在 inner 触发

每个元素都有唯一的 WorldView

### 典型用法
- 通过 section 来将世界分为多个交叠部分（世界坐标与相机坐标）
    - Render, Tool 统一通过 SectionBridge 来跨越不同的 View
        - 开启结构只读以防止意外的插入移除操作
```
|-----------------------------------------------|
|           Render, PointerTool (Basic)         |
|                                               |
|   SectionBridge           SectionBridge       |
|---vvvvvvvvvvvvv-------|---vvvvvvvvvvvvv-------|
|   Camera (Paint)      |   Camera (UI)         |
|                       |                       |
|   RenderControl       |   RenderControl       |
|   RenderControl       |   ToolCollider        |
|   ToolCollider        |                       |
|                       |                       |
|-----------------------|-----------------------|
```
- 通过不同的交叠组来实现全异步逻辑
    - 由 World 内部保证不同交叠组不会重叠
    - 需要多线程安全，内部通过 arc 实现多重引用（为了多个地方不同的 view）
    - 异步 async_enter() API


## 任意位置命令 Commander ##

允许获取世界的命令队列，然后**从任何地方直接发送命令**到世界。

可以简化组合元素的更新与清理。

## 可变性 & 生命周期 ##

如果 `RefMut<T>` 开始追踪并发送事件？

**Pros:**
- 减少调用端的检查与保证（删除 `upload` 等保证一致性的方法）
- 隐藏内部逻辑

**Cons:**
- 存在隐式逻辑与隐藏的事件触发，很容易不小心造成链式调用并引发 panic
- 显著扩大了 observer 池，使控制逻辑进一步不可控

- `when_modify` 和 `when_remove` 可以考虑一下
    - 仅限于自己内部追踪，对于 `PointerEdgeCollider` 这个案例确实是够用了
    - 相较而言逻辑也比较清晰，有这个的话 `world` 层面可以不实现生命周期事件，实在需要的话由调用方自己实现
    - 但是这样就要使用 Descriptor 模式了（需要内部追踪原 Collider 的句柄）
        - 不过 `fetch_mut` 的隐式逻辑会造成严重困惑
        - ~~Descriptor 模式*暗示了允许世界外的使用*~~
        - 但在世界里就会自动同步，不在世界里无法自动同步
    - 也可以尝试 Attach 模式，但是这样就不如使用 Observer 了

## ~~事件系统的无状态性~~ ##

> *废弃*：这个提案在生命周期规范后不再被需要了

事件系统只能在进入/离开的那一瞬间触发，而无法记录是否已经进入/离开。无论如何，为了解决这个问题，我们需要提供**外在状态性**。

所有涉及到**插入元素与触发事件同时发生**的地方：
- PointerCollider / Interact 有关鼠标进入/离开

### 从源头获取

最根本的，事件系统是有源头的，所以从源头获取状态性肯定是可行的。但会带来额外的代码，而且因为事件系统本身是一个高度抽象层，想要绕过去**获取原状态非常困难**。

### 状态性传递

在获得事件后同时进行记录，提供一个状态性。最明显的问题就是带来了**侵入性更改**。

### Hang

`hang` 是一个用来**让事件系统状态化**的功能。核心思想是 hang 住的事件无论如何，一定会被 observer，无论原来就有的还是新来的，都能够被读取到一次。
- 只会在 `observer` 触发的时候运行。和 `trigger` 差不多，关键还要额外管理 hang 自己的生命周期。
- 虽然解决了状态性问题，但是自由度受到明显限制（能且只能在事件系统里被使用）

## 状态性与描述器 ##

现在大部分元素都使用了 Descriptor 这种模式，非常清晰、直观，这也是它最大的优点。

但它的缺点也很明显——每添加一种元素就会带有一个描述器和一个元素实例。

这里的一个特例是 `PointerCollider`
- 它的附属元素 `PointerEdgeCollider` 是保存了状态了的（指针锁定的控制边 `ColliderEdgeLock`）
- 但是没有 Descriptor 参与 感觉写起来真是更简单了
- 技巧在于，它使用了 Observer 系统的**闭包**保存了实例句柄 `Handle<ColliderEdgeLock>`（其实现了 Copy 便于传递）

简单来说就是通过**一个数据元素和一个实例元素**代替了**一个描述器和一个元素**的模式。
- 优点是简单。数据元素就是单纯的数据元素。
- 缺点就是无法读出。特别是这个实例句柄无法在三个生命周期事件中传递，导致必须再额外设计一个事件并附着一个观察者来处理可变事件

## Attach 模式 ##

现在想要推进我们的 DEAT 模式 —— Descriptor, Element, Attach, Trigger（什

Attach 是为了解决单个 Element 的组合模式过于复杂而产生的，
- 目的是替代一部分目前通过*松散、无类型*的 Observer 模式实现的逻辑
- 以及替代单个 Element 内部过多 Handle 的问题

Attach 的特点：
- 强类型：相比 Observer，被调用者的类型在一开始就被确定好，无需内部使用 dyn Any 转换
- 状态化：相比 Observer，这个状态可以被简单地通过索引找到，并且有完整的生命周期和钩子

Attach 可以通过统一的世界 View 模型，借助 single 和 foreach 等相同的 API 进行查找

# 高性能与异步并发 #

## 线程安全 ##

我们希望 world 可以实现 Sync。

## 基于类型的占用表 ##

因为绝大部分占用查询负荷来自类型遍历，我们希望互斥锁是**类型独立**的。

也就是 `Handle<dyn Any>` 能够对应到 `TypeId`
再由 `TypeId` 找到 `Box<dyn Any>` 并映射到 `HashMap<usize, T>`

# 单位系统 #

我们实现了无限画布——好吧其实不是无限的，毕竟受到 32 位整数限制。但是我们希望即使真的哪个神经病到达了地图边缘我们仍然能够愉快的处理这种现象，我的答案是——循环。

因为循环其实从计算机的角度来看相当合理嘛，是一种很自然的处理方式。不过目前的 rect 在处理溢出回绕时会导致负尺寸并且无法正常显示（实际上会导致三角形完全炸掉），所以为了处理循环顺便防止负尺寸，rect 我们使用左下角点的位置 + 相对的宽度高度尺寸来处理。

# 渲染 & 用户界面 #

## 核心渲染系统 ##

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

管线相关初始化此处省略。此章重点规范**渲染组件的初始化**。

```rust
struct Panel {
    rectangle: Handle<RoundedRectangle>,
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
2. 注册对应的 **GPU 指针数据**，如需要的 Buffer 等
3. 在世界中生成**渲染控制节点**并完成注册
4. 在世界中完成**生命周期追踪**，主要是观察者和对象依赖

将插入了三种元素：
- 核心元素 `Panel`
- 渲染元素 `RoundedRectangle`
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

在渲染组件修改后，我们需要：
1. 上传对应数据到 GPU
2. 应用 RenderControl 对应更改
3. 通知重绘

修改由 `Element::when_modify` 控制。

### 5. 重绘问题

重绘由 OS 发出的 `WindowEvent::RedrawRequest` 事件控制，且发出后 Render 将不可逆地开始重绘（跳跃绘制除外）。

- 若组件希望**主动请求重绘**，应显式**触发 RenderControl 更改** 通过 `when_modify` 钩子来通知，而 `Render` 会在评估 control 的渲染剔除等属性后，将重绘请求发送给 `lnwindow` 并等待 OS 分配重绘。

- 若渲染实现需要**应用实时动画**，应在自己的 `RenderControl` 中将 `active` 改为 `true`，`Render` 会在该组件可见的情况下积极触发重绘。

有时**循环重绘**会发生，即**上一帧的渲染过程中触发了下一帧的重绘**。这会导致渲染*不受控*地无限进行下去。为了避免这种情况的产生，`Render` 会直接忽略重绘阶段产生的重绘指令，并打印一个 `WARNING` 告知潜在的循环重绘被跳过。
> NOTE: 使用 queue 模式来绕过检测是可行的，但是**绝对不推荐**！请改用 `active` 属性来进行积极重绘！

### 6. Viewport & Camera

1. wgpu 负责绘制的那个 viewport，是作用在**窗口**上的
2. 我们内部给 `render` 模块用的那个 camera，用来平移用的

两者的确是对应的（一般来说会保持两个的 width 和 height 相同），但逻辑上来讲，
- 一个是**世界空间**（或者说可以用 `Size` 来表示）
- 一个是**屏幕空间**（只能用 `[u32; 2]` 来表示）

## 无头组件 ##

**无头组件 Headless Widget** 是解耦很重要的一步。而 `ln_drawer` 原生的事件系统让这变得相当自然。

### 1. "无头组件" 概述

无头组件库定义了：
- 触发的**事件**
- 拥有的**属性**
- 捕获对应输入事件并**转换**成对应组件事件

无头组件库*不提供*：
- 如何**渲染**组件
- 如何**响应**事件（不过会提供默认响应）

### 2. 有哪些无头组件？

**第一类**包括简单逻辑组件。
1. 按钮 `Button`
    - 点击 `Click` 
2. 复选框 `CheckButton`
    - 点击 `Click`
    - 切换 `Switch`
3. 滑条 `Slider`
    - 滑动 `Slide`
    - 跃动 `Leap`
4. 宽滑条 `WideSlider`
    - 滑动 `Slide`
5. 面板 `Panel`
    - 尺寸改变 `Resize`

**第二类**包括多种逻辑组件：
1. 数字选择器 `NumberScroll`
    - 增加/减少 `Scroll`
2. 颜色选择器 `ColorPicker`
    - 点击 `Click`
    - 选择 `Pick<Color>`
3. 枚举选择器 `VariantPicker<T>`
    - 点击 `Click`
    - 选择 `Pick<T>`
4. 文本编辑 `TextEditor`
    - 鼠标输入 `CursorEdit`
    - 触摸输入 `TouchEdit`
    - 键盘输入 `KeyboardEdit`

**第三类**包括了*可嵌套*、*复合用例*的组件。
1. 单选框 `RadioButton`
2. 表格 `Table`
3. 日历 `Calendar`
4. 列表 `List`

### 3. 渲染实现

**渲染实现**主要关注每个无头组件生成的渲染类实现。

- `WidgetRender` 事件用于追踪用户交互（制作动画反馈等）
- `WidgetRender::PropertyChange` 事件时跟踪无头组件的属性并更新渲染

使用 `Attach<T>` 将具体的组件绑定到对应的渲染实现。

### 4. 最终实现

创建一个完成功能的界面元素需要：
1. 创建无头组件
2. 为无头组件绑定渲染实现
3. 为无头组件绑定响应逻辑

```rust
// 默认主题 Theme 会自动转发
let theme = world.single::<Theme>().unwrap();

// 指定主题
let theme = world.single::<Luni>().unwrap();

let widget = world.build(CheckButtonDescriptor::default());
world.trigger(theme, Attach(widget));
world.observer(widget, |Switch, world, widget| {
    let mut widget = world.fetch_mut(widget).unwrap();
    widget.enabled = !widget.enabled;

    foo(widget.enabled);
});
```

## 动画系统 ##

动画系统旨在给**渲染实现**提供一个可用的动画工具。其核心类为 `Animation<T>`。

1. 使用 `.target` 来设定动画目标
2. 使用 observer & trigger 来通知外部逻辑

### 循环重绘

考虑到动画剔除的优化问题，我们不使用直接 `request_redraw`（实际上在如何避免这个的问题上有点头疼）。

`Animation<T>` **自带 `RenderControl` 管理能力**，能够自动生成 `RenderControl`。

同时会在动画进行过程中通过 `when_modify` **自动管理** `RenderControl`。

## 布局器 ##

这个是之前 Transform 系统的进化版。负责自动控制无头组件的位置排列与分布。

### 1. 概述

布局器提供各种**更新逻辑**，会在合适的时候触发**布局事件**，并由无头组件以及子布局器读取。

### 2. 布局器

1. `Transform` 从 `Layout::Rectangle` 生成 `Layout::Rectangle`，允许锚点，对齐等高级排版工具
    - `Transform` 类似于一个 **Observer 包装**，通过指定源和目标来传递事件，而本身不接受/触发布局事件。
2. `Resizable` 从 `PointerColliderEdge` 生成 `Layout::Rectangle` 来处理尺寸
3. `Translatable` 从 `PointerColliderEdge` 生成 `Layout::Rectangle` 来处理尺寸

### 3. 布局事件

1. `Layout::Rectangle` 一个矩形
2. `Layout::Alpha` 透明度指示

## i18n 国际化 ##

首先我们使用的文本库 `cosmic_text` 完整支持国际化字符串，那么接下来我的任务：
- 让各种**布局器**支持 RTL 排版
- 支持**字符串本地化**模式

## "展开" ##

**展开**是 LnDrawer 的特色风格！借由 LnDrawer 无限画布的核心逻辑，我们不想使用传统的*列表排列遮罩 + 滚轮*的选择逻辑，转而使用更有趣的*直接铺满*的方法，来实现列表、网格、菜单、表单等的实现。

### 1. 应用场景和对应的功能需求

- 音乐播放器：选择音乐
    - 需要**支持复合元素**（文字、播放时长、专辑封面），元素大小一致
    - 需要**选择元素**并收起
    - 收起后选择的元素会和**别的控件协作**（音乐播放器还有进度条，音量条等）
    - 音乐播放器这一侧可以实现：
        - 音频管理（显然的）
        - 决定展开（播放器自己生成按钮）
        - 复合元素有哪些（由播放器生成并绑定至网格）
        - 决定收起以及通知选择了哪个（由内层元素的逻辑决定）
        - 收起到哪里
    - 音乐播放器不想实现：
        - 展开到哪里
        - 展开、收起动画（通过 `Animation` 将已注册的元素插值通过 `Layout` 移动到对应位置）
- 日历：
    - 需要**二维**平铺
    - 需要内部元素可变
    - 可能会插入很多日程，需要能够**按需收缩伸张**
- 平铺纹理绘制：
    - 需要对外提供动态**生成**的功能

### 2. 概述

展开是**一种设计风格**，它是基于 LnDrawer 的核心逻辑自然形成的风格，而非某一个特定的组件。

但是围绕着展开这一个主题有多个可以辅助实现的工具/组件：
- 动画布局器：允许简单地实现组件动画
- 平铺布局器：允许按照**固定大小**按照索引分配对应的位置

## 菜单 ##

菜单是一个**多元素组件**，这意味着它包含了**不定数量**的子组件。这会带来几个需要额外考虑的问题。

### 1. 逻辑绑定

#### 1.1. 统一绑定

```rust
let menu = world.build(MenuDescriptor {
    position: Position::default(),
    entries: &[
        MenuEntryDescriptor {
            value: 100.0,
        },
        MenuEntryDescriptor {
            value: 120.0,
        },
    ]
});

world.observer(menu, |ClickEntry(idx), world, _| {
    foo(match idx {
        0 => 100.0,
        1 => 120.0,
        _ => unreachable!(),
    });
});
```

#### 1.2. 分别绑定

```rust
let entry0 = world.build(MenuEntryDescriptor {
    value: 100.0,
});

world.observer(entry0, |Click, world, _| {
    foo(100.0);
});

let entry1 = world.build(MenuEntryDescriptor {
    value: 120.0,
});

world.observer(entry1, |Click, world, _| {
    foo(120.0);
});

let menu = world.build(MenuDescriptor {
    position: Position::default(),
    entries: &[entry0, entry1],
});
```

考虑：如果用户没有把 `entry0` 和 `entry1` 真正 build 出来会发生什么？
- 创建 observer 需要句柄，也就是说 entry 一定作为元素被插入了世界（并获取到了句柄）。
- 由于没有 menu 提供具体初始化位置 `position`，条目根本无法创建指针碰撞体等具体逻辑，同时也无法使用主题（显然）进行渲染。
- 也就是说，这会留下一些**垃圾元素**——没有具体逻辑，不可见，永远无法调用，最终造成**内存泄漏**。

简单的解决解决方法——不要忘记构建菜单。

复杂的解决方法：

#### 1.3. 注册并绑定

```rust
let menu = world.build(MenuDescriptor {
    position: Position::default(),
});

let entry0 = world.build(MenuEntryDescriptor {
    value: 100.0,
    menu,
});

world.observer(entry0, |Click, world, _| {
    foo(100.0);
});

let entry1 = world.build(MenuEntryDescriptor {
    value: 120.0,
    menu,
});

world.observer(entry1, |Click, world, _| {
    foo(120.0);
});
```

### 2. 渲染系统

Theme 系统直接监听子节点，初始的节点列表直接读取列表并挂上监听。

#### 2.1 选择

选择了某个元素会在主节点上使用 `Interact::Select(Rectangle)` 事件进行通知。

#### 2.2. 子组件

子组件不提供任何自带的渲染，theme 无需对子组件提供支持，菜单也不会在子节点发送任何 `Interact` 事件。

相应的文字渲染请直接使用对应的渲染组件。

### 3. 布局行为

列表主元素子节点不包含数据（需要由菜单的数据计算得出），但列表在列表元素增删的时，会在**子节点**触发合适的布局事件。

# 移动端支持 (Android) #

## 音频 ##

使用 `rotio` 提供的 `AAudio` 实现。
- 这要求默认使用 **SDK API Level 26**
- 限制了最低安卓版本要求 **Android 8.0+**

# 简单特征 #

## Widget ##
1. 绑定主题 `Luni`
2. 绑定控制器 `PointerTool`
3. 输出 `Widget*`
4. 响应 `Layout*`

## Layout ##
1. 有一个或多个 `Widget` 目标
2. 输出目标 `Layout*`
3. 响应目标 `Widget*`

## Tool ##
1. 响应 Lnwindow 的 `WindowEvent`
2. 输出 特定事件
3. 响应 `Layout*`