# 世界模块 #

世界模块提供**多元素响应**能力，这包括同时访问多个元素，用事件串联多个元素，以及多元素依赖处理。

功能比较类似 ECS 系统，对于类型组件的缓存访问性能也不错。

但不是 ECS，没有 Entity 以及其他能够联系 Component 成一组的能力，也没有 System 提供的高并行能力。

## 独立 Element 写法 ##

独立 Element 是指那些可以脱离 World 存在的组件，一般给渲染组件管线等功能丰富的结构体使用。

特点是包含了一些 GPU 等外部的复杂资源，使用独立方法创建或者使用 `Descriptor` 功能。

实例：`camera.rs` 

## 简单 Element 写法 ##

这个写法更常见一些，用于大部分业务代码、渲染组件等。

特点是 Element 所有字段都是公开的，全部都是公共 API，外部调用者可以**直接构造**这些结构并直接插入世界。

- 内部实现发生在 `when_insert` 阶段，一般路由回结构体的 `init` 方法。
- 内部实现的内容，如 GPU 资源、或者嵌套元素返回的句柄，不保存在 `Element` 上，而是直接利用 observer 闭包捕获。
- 闭包捕获虽然已经足够满足绝大多数场景了，但是有些时候某些资源需要在不同过程之间共享；这个时候可以创建一个私有结构体，命名一般为 `*Instance`，并在 observer 捕获句柄，在需要的时候 `fetch` 得到需要的数据。

高度依赖 world 的 observer 系统来保存运行变量，可以说是针对业务逻辑代码特化的。

- 优点是针对 observer 进行了特化，事件调用简单，响应代码写起来很规整。
- 但缺点也是针对 observer 进行了特化，内部字段被封闭在 observer 系统内，业务逻辑基本只能依附于 observer 系统存在，单单一个结构体什么都做不了。

实例：`quad.rs`, `oklab.rs`, `desktop_ui.rs`, `text.rs`

## queue 借用展平 ##

`fetch` / `fetch_mut` 会得到一个 RAII 风格的守卫 `Ref` / `RefMut` 用于运行时借用检查。

为了防止冲突，优先使用 `queue` 或 `insert` 来展平执行，而不是使用 `drop` 或者代码块。

`queue` 的闭包或者 `when_insert` 调用有以下保证：

- 所有 Element 都已经插入，你刚刚调用的 `insert` 一定可以成功 `fetch`
- 借用检查完全为空（`when_insert` 的话，只有 `self` 被可变借用）
- 深度优先展开，也就是这个调用一结束，立马调用你刚刚入队的命令（无交错）

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

## trigger 生命周期 ##

默认 `trigger` 调用是及时的

- 允许 zero-copy 和内部可变性等功能，但这种功能很少
- 缺点：生命周期管理不当可能会触发可变性检查导致 panic 或出现*循环访问*导致栈溢出

队列 `queue_trigger` 延迟调用

- 不必操心声明周期，一般用这个，除非有特殊需求
- 取点：需要事件 ownership 而不是 reference

## 约定顺序 ##

`observer` 或者 `when_insert` 推荐以下顺序进行书写：

1. **scope** 顶头声明位置，设置依赖
2. **fetch** 抓取必要组件
3. **insert** 插入数据，节点
4. **observer** 添加必要的事件控制

不是强制的，有复杂控制流的时候怎么方便怎么来。

## 元素视图 ##

元素视图主要是为了解决管理**可见性**，**权限管理**与**元素分层**等需求。目前用于多窗口支持。

world 世界模型会严格限制视图之间的可见性，以提供良好的并发安全和内存安全。

1. 所有节点都有一个视图节点
2. 在当前位置**可见**的节点来自：
    - 视图节点下的所有节点
    - 视图节点下的 `ElemRef` 节点指向的节点
        - `ElemRef -> T`
        - **(未实现)** `ElemRef -> ElemRef -> T` 单节点双跳
        - **(未实现)** `ElemRef -> ViewRef -> T` 视图跳跃
    - 视图节点下的 `ViewRef` 节点指向的视图节点下的所有节点
        - `ViewRef -> T`
        - `ViewRef -> ElemRef -> T` 视图节点双跳
        - `ViewRef -> .. -> ViewRef -> T` 嵌套跳跃
        - **(未实现)** `ViewRef -> ElemRef -> ElemRef -> T` 同上
        - **(未实现)** `ViewRef -> ElemRef -> ViewRef -> T` 同上
3. 输入非法句柄时绝对不应当被执行
    - fetch, foreach, single 家族运行良好
    - dependency(parent) 会返回 `ToxicDependency`
    - **(未实现)** 目前 enter 会导致进入一个虚空视图
    - **(未实现)** 目前 dependency(child), observer 均选择无报错返回
    - **(未实现)** 目前 trigger 做了初步拦截，会输出 log 并拒绝执行
4. 输入刚插入的节点时可部分执行
    - fetch trigger single foreach 不执行
        - fetch, trigger 返回 `JustInserted`
        - single 返回 `SingletonCorrupted` 指示单例初始化忙
        - foreach 会静默跳过
    - validate 优先返回 `Invisible` 而不是 `JustRemoved`
        -  **(未实现)**  `JustInserted` 仍优先返回，索引不可得，需要底层修改
        - 返回 `JustRemoved` 则说明句柄是可见的
    - **(未实现)** dependency, observer 允许执行
5. 不可见的节点会被直接视作非法句柄（句柄指向无效数据）处理，在任何细节上都与非法节点无差（除了报错信息）
    - enter 指令也遵循这一点，也就是说你只有看得见一个节点，你才能跳到这个节点上
    - fetch, foreach, single 家族一致性良好
    - 目前 enter, dependency(child), observer, trigger 和非法句柄处理一致（但行为不合理，见上）
    - **(未实现)** 目前 remove `ElemRef/ViewRef` 导致变为非法句柄时和 remove 原生节点后的行为不一致
    - **(未实现)** 目前 dependency(parent) 被 bypass，与非法句柄不一致
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
2. **(未实现)** 没有失去目标的无效 observer
3. 没有失去父节点的无效 dependency
4. 没有失去子节点的无效 dependency
5. **(未实现)** 没有目标被移除的无效 ElemRef/ViewRef
6. **(未实现)** 节点索引没有记录无效的 elemrefs/viewrefs 缓存
