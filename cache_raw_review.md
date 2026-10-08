# 当前缺陷报告（截至 `2946aac`）

基线：`a2ec5f1 cache+raw layer system` → `6c0d782` → `ceb24b7` → `b1b8f82` → `2946aac`。
本文件只记录**仍未解决**的问题；已完成项在 `cache_raw_review.md` 的历史版本中，已清理。

---

# 一、对 `b1b8f82` + `2946aac` 的审阅

## 1.1 `b1b8f82` undo/redo 同步：方向正确，但引入了一个热点

新增的 `Stroke::raw_merge`（`brush.rs:96-141`）思路是对的：撤销只改 raw 纹理，所以需要一个"不带 scratch、纯 raw→cache"的合并入口。六种 (cache, raw) 组合的搬运逻辑与 `submit_cache` 对齐，且 `(Filled, Transparent)` 会顺手把 cache 置为 `Transparent` —— 这个方向（拿到"删除"信号）是对的。

两个问题：

- **`draw_active` / `draw_erase` 现在每个绘制批都多了一次 `submit_cache` 提交**（`wrapper.rs:341-360`、`401-420`）。详见 §3 性能分析。
- **删除内容时上层 LOD 不会失效**（§二 R3）。

`2946aac` 把 `pick_color`、`Traveler::stock/undo/redo` 都接上了 `Associated` → cache level 0 的回退，这三处修得对。剩下 `Transparent` 分支被显式留成 `log::error!("todo: transparent chunk")`，正是你说的已知问题 2。

## 1.2 关于你说的三个已知问题

**确认成立，但比"三个"要多几层，其中第 3 个的后果最严重：**

| 你的描述 | 复核结论 |
|---|---|
| 1. 旧存档迁移没做 | ✅ 成立。另注意"上一版（`2b71d34`）写出的库"也读不回来，且会崩进程，见 R11/R12。 |
| 2. traveler 历史记录系统的透明区块没支持 | ✅ 成立，**且后果比"不支持"更重**：见 R1 —— 撤销到"本来空白"的区域时快照会整个丢失，并且 `undo()` 静默失败不给任何视觉反馈。另外相关的一个独立缺陷是 R4（快照陈旧）。 |
| 3. 主动区块请求没做 | ✅ 成立，但它**不是性能问题而是可用性问题**：raw 图层没有任何磁盘加载路径（`queue.raw` 无 `insert`、`ChunkKeyKind::Raw` 不可达），而 `prepare_scratch` 对缺失 raw 走 `continue` → **重启后一笔都画不上**。见 R2。 |

---

# 二、剩余缺陷清单

严重度：**P0** = 功能不可用 / 会崩；**P1** = 功能错误但可绕过；**P2** = 资源与性能；**P3** = 清理。

## R1. {P0} 撤销到"空白区域"时快照整段丢失，`undo()` 静默失败

`traveler.rs` —— `stock`/`undo`/`redo` 三处在遇到 `RawLayerChunk::Transparent` 或 `CacheLayerChunk::Transparent` 时是：

```rust
Some(RawLayerChunk::Transparent) => { log::error!("todo: transparent chunk"); continue; }
```

- **stock 侧**：`continue` 发生在 `self.alloc(...)` **之后**，该 section 既不进 `sections`、也不归还 atlas 分配（顺带的内存泄漏）。于是在"空白画布上画第一笔"时，快照的 `sections` 为空。之后 `undo()` 里 `self.undos.pop_back()` 拿到的是这个空快照，什么也不恢复。
- **undo/redo 侧**：`continue` 只是跳过该 section，但**函数仍然返回 `Some(Stroke)`**，`wrapper.rs:585/609` 的 `.unwrap()` 不会失败 —— 用户按 Ctrl+Z 完全没有任何反馈（既不报错也不变化）。
- **语义缺口**：`SnapshotSection` 目前只能表达"把某块旧内容拷回去"，无法表达"这块当时是空的，请清空"。所以透明支持需要新增一种 section 状态（例如 `Option<atlas 区域>`，`None` 表示当时为空），并在还原时用 `clear_chunk` 清空 cache level 0 —— 这比"多一个 match 分支"要多一点数据结构改动。
- **期望**：空白区域也能入栈；撤销到空白时清空对应 cache 区块；恢复失败时 `undo()` 返回 `None` 让上层能拒绝这次操作。

## R2. {P0} raw 图层仍然没有磁盘加载路径 → 重启后无法绘制

- `stream.rs` 里 `queue.raw` 只有 `clear` / `sort` / `get_index` / `contains`，**没有任何 `insert`**；`restock` 只填 `queue.cache`。因此 `generate_batch` 的 raw 分支（`stream.rs:619-628`）永远取不到 key，`ChunkKeyKind::Raw` 不可达，`load_raw` / `RawInsert` / `RawRemove` 整条链是死代码。
- raw 块唯一来源是主线程 `RawLayer::swap_chunk` → `SwapRaw`，而 `swap_chunk` 只在 `submit_cache` 内部对**已存在**的 raw 块调用 → 干净启动时 `raw.chunks` 恒空。
- `DrawsBatch::prepare_scratch`（`brush.rs:363-372`）对 `None => continue`：**每一批绘制都被静默丢弃**（`draw_active` 只画 scratch，而 scratch 里没有新块）。
- 关联：`pick_color` 已能回退到 cache，但新画笔依然无效；`Traveler` 也一样拿不到 raw。
- **期望**：raw 与 cache 对称地由相机队列加载（level 0 键），并补上"绘制前请求 + 未就绪则放弃该批"的通道（即你的已知问题 3）。

## R3. {P1} 内容被删除后，上层 LOD 区块不失效，缩小视图仍显示旧笔迹

`raw_merge` / `submit_cache` 在把 cache level 0 变成立即 `Transparent`（`brush.rs:125-127`）或把 raw 置为 `Transparent` 时，**只动了 level 0**；而 `lp.cache_mipmaps` 的循环条件是"上下层都必须 `Filled`"：

```rust
let Some(CacheLayerChunk::Filled(lower_chunk)) = cache.chunks.get(&lower_key) else { continue };
```

level 0 变成 `Transparent` 后，上层 level 1..7 的 `Filled` 区块**永远不会被更新或清除**，继续保持删除前的内容。用户在缩小的视图里仍能看到已经撤销掉的笔画。

- **期望**：上层区块要么在 level 0 变透明时递归失效（置 `Transparent` 并通知流线程），要么让 `cache_mipmaps` 支持"下层透明 → 上层清空"。
- 触发路径：撤销（`raw_merge`）与"擦掉全部内容"（`submit_cache` 的 `(Filled, Transparent)` 分支）。

## R4. {P1} 撤销快照依赖"当前内存中恰好有 cache level 0 区块"，缺失时静默放弃

`traveler.rs` 的 `undo`/`redo` 在 raw 是 `Associated` 时要求 `cache.chunks.get(&(x, y, 0))` 命中 `Filled`，否则 `continue`：

- 区块被流线程卸载（`CacheRemove`）后，`Associated` 的 raw 块暂时没有任何内容可恢复，撤销会**静默跳过这一节**，而不是拒绝这次操作或等待区块加载。
- `undo_available` / `redo_available` 只检查 `raw.chunks.contains_key`，查不出这种"raw 在但内容不在"的情况，所以上层也无法提前拒绝（见 R1 的同一类问题：状态检查与执行结果不一致）。
- **期望**：`*_available` 与执行路径用同一套判据；或让撤销路径具备"必要时取回区块"的能力。

## R5. {P1} 擦除路径的降级/落盘时序被改动

`draw_erase` 从 `submit_raw_cache` 改成 `submit_cache`（`b1b8f82`）后：

- **不再降级为 `Associated`**：擦掉内容后 raw 可能仍是 `Filled`（旧内容），cache 才是新的（被擦除的）内容。二者处于"内容不同"的状态，只是恰好能自洽 —— 直到 R3/R4 的路径踩进来。
- **不再写入 raw 的 `Transparent`**：在原本空白的区域擦除，会走 `submit_cache` 的 `(Transparent, Transparent)` 分支，把 scratch（透明的擦除结果）写进新建的 cache 区块，而 raw 保持 `Transparent`。数据集里因此多出一个"无意义但持久化"的 cache 区块（关联 R3/R9）。
- 另外 `draw_erase` 每个绘制批都会 `submit_cache` → 触发流线程的回读与 zstd 落盘（`SwapCache` 即标脏），长按擦除时卡顿明显。
- **期望**：明确"擦除的结果归 raw 还是归 cache"；若与普通笔画对称，就应在 `submit()` 时统一提交。

## R6. {P1} 非 `STORAGE_READ_WRITE` 设备上绘制会静默失效

编译器确认以下三项全部未被使用（`cargo check` 警告）：

- `field support_read_write is never read`
- `methods draw_chunk_swap and merge_chunk_over_read_write are never used`
- `layer.rs:455` 还留着 `// TODO pipeline should have read_write/swap varieties`

当前只实现了 `draw_chunk_read_write` 一条路径，而 `support_read_write == false` 时 `chunk_layout.read_write` 退化成只读布局 —— 在无 read-write storage texture 支持的设备/后端上，绘制与合并都会走错布局或失效。旧实现有 swap（双缓冲）分支兜底。

- **期望**：恢复 `draw_chunk_swap` + `submit` 的 swap 分支，或显式检测并给出可诊断的错误。

## R7. {P2} 每个绘制批都做一次 `submit_cache` + `cache_mipmaps`（性能，详见 §三）

`draw_active`（`wrapper.rs:341-360`）与 `draw_erase`（`401-420`）在每批绘制后都额外提交一个 encoder + compute pass，其中对**整笔画累积的 `stroke.dirty`** 区域重做"scratch → cache 拷贝/合并"与"逐级 LOD 重建"。

- 替换模式笔刷（pixel / tint / smudge / blur）在旧实现里每批只有 1 个 encoder，现在是 2 个。
- 非替换模式笔刷（round）批数相同，但旧实现是**增量**的 `cache_merge_scratch`（只处理 `stroke.dirty` 命中的 chunk + 增量分配），现在是**每批全量重拷累积矩形**。
- `cache_mipmaps` 在放大状态（只存在 level 0）时是空转（`(i32,i32,u8)` 的 `upper_chunk_of` 命中不了上层），代价主要是 CPU 侧的 `rect_chunks_stack_iter` 7 层 × N 块枚举；缩小状态有上层区块时才是真实负载。
- **期望**：见 §三 的建议（最少合并成一个 encoder，并把 dispatch 从 `stroke.dirty` 换成 `batch.dirty`）。

## R8. {P2} 重复的 dispatch 上传：同一批数据被写 3 次

`draw_active` 的调用链：

1. `batch.upload(&self.lp, &mut self.bridge)` → `upload_dispatch`（写 `work_rect` / `draws_rect` / `draws_length` / `draws_array`）
2. `stroke.upload_dispatch(&self.lp)` → 再写一次 `work_rect`
3. `submit_cache` 内部通过 `copy_chunk` / `merge_chunk_over_cross` 使用 `work_dispatch_group`

其中 1 与 2 的内容是同一批的同一份数据（`work_rect` 都写 `batch.dirty`），是纯粹的重复 host→device 写。每批 4 次 `queue.write_buffer`，可以省一半以上。

- **期望**：`upload_dispatch` 拆出"只写 work_rect"的版本，或在 `DrawsBatch::upload` 里一并写好后不再重复。

## R9. {P2} 透明块不再回收，数据库只增不减

`persist_raw` / `persist_cache` 现在无条件写入，而 `ChunkSave::is_transparent`（`chunk.rs:153`）是死代码（编译器警告 `method is_transparent is never used`），旧的"全透明则从表中删除"逻辑没有接上。

- 后果：被擦掉的内容在库里留下全零纹理；`RawLayerChunk::Transparent` / `CacheLayerChunk::Transparent` 也会被写成 `associated` / 空行。长期使用后数据库体积只增不减，与 issue 中"评估是否有必要保留原始数据"的意图相反。
- **期望**：落盘前用 `is_transparent` 判定，透明则 `save_raw_transparent` / `save_cache_transparent`。

## R10. {P2} `ChunkPool` 只进不出（内存无界增长）

编译器警告 `method pop_chunk_stack is never used`。`cleanup()` 每笔把 scratch 区块 `push` 进 `self.pool.list`，而 `prepare_scratch` 用 `lp.create_empty_chunk(...)` 直接新建 512² 纹理，**从不 pop**。

- 后果：池子（以及它持有的 GPU 纹理）随绘制量无界累积。
- **期望**：`prepare_scratch` 改走 `pop_chunk_stack`（它已经会重写 rect 并复用纹理），或干脆删掉池子。
- 附带：`function lower_chunk_of is never used`，说明 LOD 的"下层枚举"已无消费者。

## R11. {P0} 损坏区块会让整个流线程 panic，进而崩掉进程

`ChunkLoad::load_raw/load_cache` 对无法解析的 4 字节 version 或 zstd 解压失败返回 `Err`，经 `StreamThread::load()?` → `run()?` → `wrapper.rs:141-145` 的 `.unwrap()` 直接 panic。旧实现是 `if let Ok(...)`，损坏区块只当作未加载。

- 期望：跳过坏块并 `log::error!`，不要终止线程。

## R12. {P0} 存档兼容链断裂

- **旧表（`stroke_chunk` + `stroke_chunk_meta`）完全没有迁移代码**：`save/chunk.rs` 的 `legacy_classic` 模块只有两个未使用常量，`chunk_migration` 整个函数体被注释掉（编译器对此报 4 条 never used）。
- **`2b71d34` 写出的库也读不回来**：那时写的是"裸 zstd、无 4 字节头部、表名 `stroke_chunk`"，新代码只读 `layer_chunk` / `layer_chunk_raw` 且要求 `version` 头。
- `save.rs` 的文档注释仍描述旧表结构；`DATABASE_VERSION` 仍为 0，`migrate_format` 的循环体是 `unimplemented!`；`Scene` 里没有"是否已迁移"的状态位。
- `CHUNK_VERSION = 0` 与 `docs/save.md` 的 "version 版本 v1" 不一致；`docs/save.md` 缺尾换行。
- **期望**：读旧表 → 逐块 format 迁移 → 写 `layer_chunk` + `layer_chunk_raw`（伴生）→ 删旧表，由版本号或显式标记驱动一次性迁移。

## R13. {P2} `unload_overflow` 在候选都在队列中时空转

`stream.rs:411-427`：`continue` 分支不递减 `frnt`，若 `cache_overflow()` 持续为真会反复扫描同一批条目。应当记录"本次有效卸载数"，无进展即提前退出。

## R14. {P2} `restock` 的 `debug_assert` 语义错位

`debug_assert!(self.cache.len() < CHUNK_CACHE_REAL_CAPS - 1)` 统计的是**队列长度**（含所有 LOD 层级的可见区块），不是真实纹理数；靠近 level 0 的视锥很容易超过 511 条而在 debug 构建 panic。

## R15. {P2} 快照分配失败/跳过时的 atlas 泄漏

`traveler.rs:99-102`（`alloc` 失败）、`116-137`（各种 `continue`）都在**已经分配了 atlas 区域之后**放弃，既不写 section 也不 `deallocate`，这些区域永久泄漏。虽然被 `UNDO_SNAPSHOTS_LIMIT` 限制在 64 份快照内，但每次撤销栈滚动都只归还"已入栈"的 section。

## R16. {P3} 死代码与文档

- `layer.rs` 的 `Layer` 结构体、`ChunkPool.chunk_size`、`save/chunk.rs` 的 `ChunkLoadResult.unsaved`、`brush.rs` 的 `submit_standalone`、`stream.rs` 的 `MarkRawUnsaved` / `MarkCacheUnsaved` 变体、`legacy_classic` 的 `ChunkKey` 别名 —— 共 15 条编译警告，其中 `support_read_write` 与 `pop_chunk_stack` 两条带功能含义（见 R6 / R10）。
- `widgets/brush_preview.rs:37-41` 仍称使用 `DrawPipeline::draw_standalone`；`wrapper.rs:55-62` 的 `LayerPage` 文档仍写 "main / merge / mipmap 三层、mipmap 尚未实现"。
- debug 标签混杂：`"draw_active_cache"` / `"draw_erase_cache"` / `"paint_seed"` 与 `"draw_active"` 风格不统一。

## R17. {P3} 原始图层修剪仍未实现

`wrapper.rs:490` `// TODO Evaluate raw chunks and turn suitable chunks back into associated/transparent`（issue 的"自动保存/区块卸载时评估是否有必要保留原始数据"）。与 R9 是同一件事的两端：一个决定"什么时候不必留"，一个决定"不留时怎么落盘"。

---

# 三、热点路径性能分析

结论：**是的，`b1b8f82` 让每个绘制批从 1 个 compute pass 变成 2 个；但"双 pass"本身不是新发明 —— 旧实现（`2b71d34`）在非替换模式笔刷上也是 2 次提交，只是第二次是增量优化过的。**

## 3.1 各版本的每批提交次数

| 路径 | `2b71d34`（重构前） | `a2ec5f1` | 现在（`2946aac`） |
|---|---|---|---|
| 普通笔刷 · 替换模式（pixel / tint / blur / smudge） | 1 encoder + 1 cpass | 1 + 1 | **2 + 2** |
| 普通笔刷 · 覆盖模式（round / soft） | 2 encoder + 2 cpass（`draw` + `cache_merge_scratch`） | 1 + 1 | **2 + 2** |
| 长按擦除（`draw_erase`） | 1 + 1（`self.erase.draw`） | 1 + 1 | **2 + 2** |
| `submit()`（抬笔） | 1 + 1 | 1 + 1 | 1 + 1 |

所以：
- 对**替换模式**笔刷：确实是**新增**了一个 pass（旧版 1 个）。
- 对**覆盖模式**笔刷：pass 数持平，但第二次的**内容**退化了（见 3.2）。
- 你看到的"双 cpass"在普通笔刷路径上以前只对覆盖模式存在，现在所有笔刷都有 —— 所以你"以前没发现"是对的。

## 3.2 第二个 pass 的实际代价

`stroke.submit_cache` 的开销与 dirty 矩形成正比：

1. 遍历 `scratch.chunks`（= `stroke.dirty` 覆盖的所有 chunk）；
2. 每个 chunk 一次 `copy_chunk`（替换模式）或 `merge_chunk_over_cross`（覆盖模式），dispatch 是 `stroke.dirty ∩ chunk_rect`；
3. 末尾 `cache_mipmaps(cache, stroke.dirty)`：枚举 level 0..6 × 每层 `stroke.dirty` 覆盖的 chunk，命中的才 dispatch。

关键点是 **dispatch 用的是 `stroke.dirty`（整笔画累积并集）而不是 `batch.dirty`（本批）**：

- 假设笔画分 N 批、第 i 批结束时并集面积 Aᵢ（单调递增），那么第二 pass 的总工作量 ≈ Σ Aᵢ ≈ N·A/2（A 为最终并集面积），而"只在 `submit()` 做一次"是 A。
- 一次长笔画（比如 200 批、跨越多个 512 区块）在这里是**几十倍的冗余像素搬运**。这不是渐近复杂度的问题，是常数因子的量级问题。
- 旧实现的 `cache_merge_scratch` 用的是 `self.batch.dirty`（增量），并且只在 `stroke.replace == false` 时才做；`layer_prepare_rect` 也只会为新出现的 key 分配。也就是说**旧实现的双 pass 是"增量增量"，新实现的双 pass 是"每批全量"**。

## 3.3 放大第二 pass 成本的两件事

1. **`prepare_scratch` 每批都可能新建 512² 纹理**：`(Filled, Filled)` 之外的组合会走 `create_empty_chunk`，配合 R10（池子只进不出）就是"每批新建 + 永久不回收"。在 4096×4096 的笔画上，这本身就够卡。
2. **`SwapCache` 即标脏**：第二 pass 里每个被创建的 cache 区块都会 `swap_chunk` → `ThreadInput::SwapCache` → 流线程 `cache_unsaved.insert`；`Autosave` 或区块卸载时对每一块都做 `copy_texture_to_buffer` + `device.poll(wait_indefinitely)` + zstd 压缩写库。擦除路径每批提交，意味着长按擦除可能在一次操作里把几十上百个区块的落盘全排上，表现为周期性顿卡（这与 §二 R5 是同一件事的性能面）。

## 3.4 不是问题的地方

- 放大状态（只存在 level 0）时 `cache_mipmaps` 是空转：`upper_chunk_of` 找不到上层 `Filled`，只有 CPU 侧的迭代。**7 层 × N 块的枚举是固定成本，不值得单独优化**，但可以顺手在 `mipmap_cap == 0` 时短路（现在 `cache.mipmap_cap - 1` 在 cap 为 0 时会 u8 下溢 panic）。
- `merge_chunk_over_cross` 在非 RW 设备上用的是 `[dispatch, read, read, write]` 布局，`dst=raw` 与 `swp=new cache chunk` 是两张不同纹理，没有读写别名问题。
- `brush_preview.rs` 的 4 个 encoder 是每次重绘预览各 1 个（清屏 / 两条色带 / 笔画），只受 `outdated` 门控，不影响绘制热路径。

## 3.5 建议（按性价比排序）

1. **合并两个 encoder 为一个**：`draw_active` 的两段之间没有跨队列依赖，第二段完全可以在同一个 `CommandEncoder` 里开第二个 `ComputePass`（甚至并进第一个 pass，只要 bind group 顺序正确）。这直接消掉一半的 encoder/queue submit 固定成本。
2. **把第二 pass 的 dispatch 从 `stroke.dirty` 换成 `batch.dirty`**：语义不变（本批新画的部分才需要同步给 cache），但工作量从 Σ Aᵢ 降到 Σ aᵢ ≈ A。
3. **去掉重复的 `upload_dispatch`**（R8），每批省 4 次 `queue.write_buffer`。
4. 如果第二 pass 的唯一目的是"让替换模式笔刷在笔画中途能采样到当前结果"，考虑**只对 `bridge_mode() || replace_mode()` 的笔刷保留**，覆盖模式笔刷（round/soft）保持旧行为（抬笔时统一提交）。
5. `prepare_scratch` 接上 `ChunkPool`（R10），避免每批新建纹理。

---

# 四、优先级建议

| 顺序 | 项 | 级别 | 说明 |
|---|---|---|---|
| 1 | **R2** raw 磁盘加载 | P0 | 不修就无法绘制、无法验证其它修改 |
| 2 | **R1** 透明区块快照 + `undo()` 失败返回 | P0 | 你的已知问题 2；需要给 `SnapshotSection` 增加"当时为空"的表达 |
| 3 | **R11 / R12** 崩溃与迁移 | P0 | 与绘图内核解耦，可并行 |
| 4 | **R3 / R4** 删除后 LOD 失效、快照与内存状态耦合 | P1 | 都是"删除/卸载"这条路径上的语义漏洞 |
| 5 | **R7 / R8 / R10** 性能与资源 | P2 | 见 §3.5 |
| 6 | **R5 / R6** 擦除时序、非 RW 设备 | P1 | R6 影响面小但会静默失效 |
| 7 | **R9 / R13 / R14 / R15 / R16 / R17** | P2/P3 | 清理与长尾 |
