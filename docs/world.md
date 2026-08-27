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
|-----------------------------------------------|
|                  INITELEM                     |
|---------------vvvvvvvvvvvvvvv-----------------|
|               Lnwindow (Main)                 |
|              Render, PointerTool              |
|---vvvvvvvvvvvvv-------|---vvvvvvvvvvvvv-------|
|   Camera (Paint)      |   Camera (UI)         |
|                       |                       |
|   RenderControl       |   RenderControl       |
|   RenderControl       |   ToolCollider        |
|   ToolCollider        |                       |
|                       |                       |
|-----------------------|-----------------------|
```

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
