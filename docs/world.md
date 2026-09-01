> NOTE: This page is written in Chinese.

# 世界模块 #

世界模块提供**多元素响应**能力，这包括同时访问多个元素，用事件串联多个元素，以及多元素依赖处理。

功能比较类似 ECS 系统，对于类型组件的缓存访问性能也不错，但总体上没有 ECS 的高并行能力。

## 世界外 Element ##

允许独立存在，独立运行的组件，参考 `LayerPipeline` (`layer.rs`)

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

最不推荐使用 `Option<T>` 的写法：

```rust
impl Baz {
    pub fn new(property: Property) -> Bar {
        /* .. */
    }
}
impl Element for Baz {
    fn when_inserted(&mut self, world: &World, handle: Handle<Self>) {
        // 过度包揽了自己不该干的活儿
        let interface = world.single_fetch_mut::<Interface>().unwrap();
        let inner = interface.create_painter(/* .. */);
        // self.inner 为 None 的状态只在初始化时存在，很别扭
        self.inner = Some(inner);
    }
}
```

Descriptor 模式本身**不暗示世界外使用**，真正暗示世界外使用的是 **Descriptor 不返回 `Handle<T>` 而直接返回 `T`**。

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

### 完全世界节点

这种类型只工作在世界内，就可以简化一些代码

实例来自 `quad.rs`

```rust
impl<M: QuadMaterial> QuadMesh<M> {
    pub fn init(&self, world: &World, this: Handle<Self>) {
        let render = world.single_fetch::<Render>().unwrap();
        let pipeline = world.single_fetch::<QuadMeshPipeline<M>>().unwrap();
        // Initialize code
    }
}
impl<M: QuadMaterial> Element for QuadMesh<M> {
    fn when_insert(&mut self, world: &World, this: Handle<Self>) {
        self.init(world, this);
    }
}
```

## insert & remove 生命周期 ##

我们遵循生命周期最小的原则：

- 在 insert 后无法 fetch
- 在 insert-flush 后正常 fetch
- 在 remove 后立刻无法 fetch
- 在 remove-flush 后当然无法 fetch

我们极力避免出现生命周期交叉！（图中没有画 queue 有关的，但是也应该尽力保证不交叉）

```text
---- insert
|    insert-flush ------
|    insert-event      |  ----  ===> where `when_insert` runs (after insert-flush)
|    ...               |     |
|    modify            |     |
|    modify-event      |     |  ===> where `when_modify` runs (after modify)
|    ...               |     |
|    bind-deps         |     |
|    ...               |     |
|    remove-event      |  ----  ===> where `when_remove` runs (before remove)
|    remove-deps       |   ^^^
|    remove       ------   element-trait hook lifecycle
---- remove-flush    ^^^
^^^                  fetch-available lifecycle
actual ownership lifecycle      ===> where `drop` runs
```

### 有关生命周期事件

之前是有一个 `Destroy` 事件的，但是被 `when_remove` 代替了。

完全取消生命周期事件的原因很简单——**减少隐式逻辑**。

- 提供一个统一的生命周期事件会导致任何元素都会能够读取同种生命周期事件而不加区分，往往会导致难以分离的循环调用问题
- 使用 hook 模式可以将事件触发下降到高级逻辑层面，这可以限定其作用范围，缩小调试范围并提供更稳定的代码

### `Ref` 与 `RefMut` 的 `.handle()` 函数

可以让 Rust 的 variable shadow 更好地发挥作用

```rust
let fetched = world.fetch(this).unwrap();
fetched.perform();
world.observer(this, /**/);
```

```rust
let this = world.fetch(this).unwrap();
this.perform();
world.observer(this.handle(), /**/);
```

## Observer & Trigger  ##

推荐 observer 的正统用法。这意味着绑定到监听节点是不推荐的：

```rust
struct ElementUpdate(ElementHandle);
let that, listener;
world.observer(listener, |ElementUpdate(rec), world| update(rec));
world.trigger(listener, ElementUpdate(that));
```

而推荐绑定到被监听节点:

```rust
struct ElementUpdate;
let that, listener;
world.observe(that, move |ElementUpdate, world| update(that));
that.trigger(that, ElementUpdate);
```

### trigger 即时性

默认 `trigger` 调用是及时的

- 这允许 zero-copy 和内部可变性等功能，也可减少命令队列负担
- 大部分请求不需要关照 `insert` 和 `remove` 的延迟执行的
- 缺点：生命周期管理不当可能会触发可变性检查导致 panic 或出现*循环访问*导致栈溢出

有对应的 `queue_trigger` 延迟调用

- queue 下属 shortcut
- 需要变量所有权而不是引用
- 一般来说更好用，代码库里用这个的也更多

## 元素视图 ##

元素视图主要是为了解决管理**可见性**，**权限管理**与**元素分层**等需求。

### 典型用法

```
|                  INITELEM                     |
|-----------------------------------------------|
|            LnAndroid (on Mobile)              |
|               Lnwindow (Main)                 |
|---------------vvvvvvvvvvvvvvv-----------------|
|              Render, PointerTool              |
|                                               |
|   Camera (Paint)          Camera (UI)         |
|---vvvvvvvvvvvvv----|------vvvvvvvvvvvvv-------|
|   RenderPhase      |      RenderPhase         |
|                    |                          |
|   RenderControl    |      RenderControl       |
|   RenderControl    |      ToolCollider        |
|   ToolCollider     |--------------------------|
|                    | RenderPhase |RenderPhase |
|                    | SubUI       |SubUI       |
|--------------------|--------------------------|
```

## 管理保证 ##

world 世界模型会严格限制视图之间的可见性，以提供良好的并发安全和内存安全。

1. 所有节点都有一个视图节点
2. 在当前位置**可见**的节点来自：
    - 视图节点下的所有节点
    - 视图节点下的 `ElemRef` 节点指向的节点
        - `ElemRef -> T`
        - **未实现** `ElemRef -> ElemRef -> T` 单节点双跳
        - **未实现** `ElemRef -> ViewRef -> T` 视图跳跃
    - 视图节点下的 `ViewRef` 节点指向的视图节点下的所有节点
        - `ViewRef -> T`
        - `ViewRef -> ElemRef -> T` 视图节点双跳
        - `ViewRef -> .. -> ViewRef -> T` 嵌套跳跃
        - **未实现** `ViewRef -> ElemRef -> ElemRef -> T` 同上
        - **未实现** `ViewRef -> ElemRef -> ViewRef -> T` 同上
3. 输入非法句柄时绝对不应当被执行
    - fetch, foreach, single 家族运行良好
    - dependency(parent) 会返回 `ToxicDependency`
    - **未实现** 目前 enter 会导致进入一个虚空视图
    - **未实现** 目前 dependency(child), observer 均选择无报错返回
    - **未实现** 目前 trigger 做了初步拦截，会输出 log 并拒绝执行
4. 输入刚插入的节点时可部分执行
    - fetch trigger single foreach 不执行
        - fetch, trigger 返回 `JustInserted`
        - single 返回 `SingletonCorrupted` 指示单例初始化忙
        - foreach 会静默跳过
    - **未实现** dependency, observer 允许执行
    - **未实现** validate 优先返回 `Invisible` 而不是 `JustInserted`
        - 返回 `JustInserted` 则说明句柄是可见的
5. 不可见的节点会被直接视作非法句柄（句柄指向无效数据）处理，在任何细节上都与非法节点无差（除了报错信息）
    - enter 指令也遵循这一点，也就是说你只有看得见一个节点，你才能跳到这个节点上
    - fetch, foreach, single 家族一致性良好
    - 目前 enter, dependency(child), observer, trigger 和非法句柄处理一致（但行为不合理，见上）
    - **未实现** 目前 remove `ElemRef/ViewRef` 导致变为非法句柄时和 remove 原生节点后的行为不一致
    - **未实现** 目前 dependency(parent) 被 bypass，与非法句柄不一致
6. INITELEM 也和不可见节点一样，总是被当作非法句柄
    - fetch, foreach, single 家族一致性良好
    - 目前 enter, dependency, observer, trigger 和非法句柄处理一致（但行为不合理，见上）
7. 任意节点内容同一时刻只能被一个线程看见
    - 目前是单线程，当然满足（笑）
    - 随着后续调整可能会允许多线程不可变地占用同一个节点
    - 这个保证允许了所有调用都无锁，只做一次运行时可变检查
8. 不调用 enter 操作视图绝对不变
    - observer, queue 包含闭包，其运行时视图节点仍然是上下文节点，不会变成别的视图节点
9. 任何时候 dependency 绝对不失效
    - 跨视图 dependency 仍然会正常执行
    - 在 when_remove 的时候所有依赖依旧保证可以访问
10. 缓存绝对有效，只有丢失缓存，没有错误缓存
    - 不会看见多余的元素
    - 不会看见错误的元素

尽可能解决世界模型内部的内存泄漏问题：

1. 没有失去视图节点导致不可访问的元素
2. **未实现** 没有失去目标的无效 observer
3. 没有失去父节点的无效 dependency
4. 没有失去子节点的无效 dependency
5. **未实现** 没有目标被移除的无效 ElemRef/ViewRef
6. **未实现** 节点索引没有记录无效的 elemrefs/viewrefs 缓存

## 任意位置命令 Commander ##

允许获取世界的命令队列，然后**从任何地方直接发送命令**到世界。

可以简化组合元素的更新与清理。

## 高性能与异步并发（TODO） ##

### 线程安全

我们希望 world 可以实现 Sync。

### 基于类型的占用表 

因为绝大部分占用查询负荷来自类型遍历，我们希望互斥锁是**类型独立**的。

也就是 `Handle<dyn Any>` 能够对应到 `TypeId`
再由 `TypeId` 找到 `Box<dyn Any>` 并映射到 `HashMap<usize, T>`
