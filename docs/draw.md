# 绘制调用简述

## 通用过程

过程分组，并标记了优化过的步骤。

### batch-draw: draw + cache

draw:

1. scratch 归化为 Some
2. draw 到 scratch 上

cache:

1. raw 去 associate
2. *(特化预留)* cache 0 生成 -> 带 scratch 生成 cache 0
3. cache 后续生成
4. *(跳过)* raw 语义回收 -> 不必回收：scratch 存在一定不回收 + 后面 batch 还会频繁调用 + 两个 submit 阶段总会至少回收一次

### stroke-submit: merge + cleanup + cache

merge:

1. *(跳过)* raw 去 associate -> 反正 batch 的时候执行过了，再执行一遍也行
2. *(特化预留)* scratch 合并到 raw -> 还没有别的覆盖物，这边直接设置成 associated 丢掉旧区块即可

cleanup:

1. scratch 回收

cache:

1. *(跳过)* raw 去 associate -> 不生成 cache 无必要
2. *(跳过)* cache 0 生成 -> cache 已准备好
3. *(跳过)* cache 后续生成 -> cache 已准备好
4. *(特化预留)* raw 语义回收 -> 还没有别的覆盖物，一定是 associated

### stroke-discard: cleanup + cache

cleanup:

1. scratch 回收

cache:

1. raw 去 associate
2. *(特化预留)* cache 0 生成 -> 不带 scratch 生成 cache 0
3. cache 后续生成
4. *(特化预留)* raw 语义回收 -> 还没有别的覆盖物，一定是 associated

### redo/undo: place + cache

place:

1. raw 去 associate
2. 把内容拷贝到 raw

cache:

1. *(跳过)* raw 去 associate -> place 做了
2. *(特化预留)* cache 0 生成 -> 不带 scratch 生成 cache 0
3. cache 后续生成
4. raw 语义回收

## 失败路径

其他场景主线程都可以补救，但 raw/cache 未加载时主线程是一点办法也没有的（除非阻塞等待流线程），所以需要额外的处理。

### batch 时 raw/cache 没有加载好

1. 不调用绘制，这一批会被丢弃，但避免了区块边界出现“被切开的”笔迹
2. 不调用 `confirm_batch`，stroke 会正确滞留在上一笔，用户会感觉笔刷卡在了区块边界，但问题不算特别严重，只需等待加载即可
3. 主动请求区块加载

### stroke-submit 时 cache 没有加载好

没啥关系，cache 已经是完成的状态。啥都不用做。

### stroke-submit 时 raw 没有加载好

非常糟糕的场景；不过很少见，一般来说，前面调用 batch 后会把相关的区块全部加载进来；但是如果cache确实是没有加载……

- 方案一：尝试调用 stroke-discard 丢弃笔刷，刚才画好的全部丢失，算是比较严重的问题（简单，但问题还在）
- 方案二：允许 submit 失败，Stroke 进入一个“悬挂”的状态，下一个 Stroke 之前每次都会先尝试把这次笔刷提交，提交上了才能创建下一个（复杂，但比较美观）

### stroke-discard 时 raw 没有加载好

raw 无法进行语义回收，不过再画一笔即可正常回收，问题不是特别严重。

### stroke-discard 时 cache 没有加载好

非常非常糟糕的场景；特别是这次 stroke-discard 如果是 stroke-submit 的回退调用，那么意味着主线程啥也没有，数据库很有可能处于完全非法的状态。

- 方案一：和 stroke-submit 的方案二一样允许重试
- Plan B: 阻塞等待！！！

## 未被记录的优化

1. replace 模式启用时 cache 生成可以不依赖 raw，也就不需要去 associate

## 中途保存（Bug：数据丢失）

如果在 stroke 过程中保存，因为 scratch 没有办法保存下来，数据库会记录下来一个 cache 和 raw 不同步的快照，需要考虑解决方案

## 流线程和主线程的同步

流线程的任务是加载/保存，主线程则负责修改里面的内容。这意味着：

1. 流线程对区块加载有绝对权威，流线程说没加载那就是没加载，主线程无论如何不能假设区块里面是什么内容。
2. 主线程负责区块内容，比如 GPU 写入或者替换成 associate/transparent 变种，都是可以的。

来考虑一些同步时的边界情况。

### 1. SwapChunk

SwapChunk 是一个特殊的“仅写入”操作，会覆盖所有以前的内容，所以不存在“假设区块里面内容”的问题，反正全被替换掉了，是**合法的**操作。

注意即使流线程已经决定卸载区块也没有关系，因为那个卸载掉的区块我们不需要了。

### 2. 未保存的操作（Bug：数据丢失）

会存在这样的情况，流线程决定卸载某个区块，读取了内容并交给了数据库。这**之后**才会通知主线程卸载区块，此时主线程可能会操作区块，导致区块又更新了，但没来得及同步到数据库。

解决方案：在保存之前就通知主线程卸载，主线程发来消息确认卸载后我们再进行内容读取并卸载。

需要额外考虑：如果主线程又主动请求了区块，或者流线程决定加载区块，我们还得考虑“完全卸载”，“部分卸载”两种情况

### 3. 区块回收

在最后丢区块的那一侧进行区块回收。

## swap 设备兼容

调试：debug build 下设置 `LNDRAWER_NO_READ_WRITE=1` 可以在支持原地写入的硬件上强制走 swap 路径。
