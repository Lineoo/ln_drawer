# cache / raw 图层系统缺陷报告

基线：`9fe1fb8 temporary fix for brush_preview` + 工作区未提交改动（`layer/brush.rs`、`layer/stream.rs`、`layer/wrapper.rs`、`save/chunk.rs`）。
本文件只记录**当前仍然存在**的问题；本轮已修好的对照清单见 §4。
严重度：**P0** 功能不可用 / 丢数据 · **P1** 功能错误（有前置条件或可绕过）· **P2** 资源、性能、落盘 · **P3** 死代码与文档。

## 0. 自查手段

- 状态栏的流线程 debug（`layer/stream.rs:261+`）：重启后画布有内容但 `decode / load_real == 0`，说明全部走了"读失败 → 当空块"的兜底（§4 的 `load_raw` 就属于这类）。
- 日志关键字：`call \`lp.raw_remove_associated\``（状态机顺序错）、`todo: transparent chunk`（快照丢节）、`cache not loaded` / `raw not loaded`（就绪门禁与加载顺序）、`loading {key:?} from database error`（格式问题）、`section skipped`。
- redb raw 行字节布局：`version(4B LE) + flag(1B) [+ zstd payload]`，**flag=0 才有 payload**，flag=1（associated）只有 5 字节。
- GPU 侧可以直接看 `raw_real_cnt`（现在把 `AssociatedCached` 也算"占纹理"）与 `base.raw.len()` 的差值。

---

# 1. 现状速查

## 1.1 变种与不变式

| 层 | 变种 | 语义 |
|---|---|---|
| raw | `Transparent` | raw 的内容为空（权威） |
| raw | `Associated` | raw 的内容 == cache level 0（不持有纹理） |
| raw | `AssociatedCached(Chunk)` | 同上，但额外持有一份"等于 cache level 0"的纹理副本，用于免拷贝物化 |
| raw | `Filled(Chunk)` | raw 自有纹理是权威（笔画期间的备份、或从盘上读回） |
| cache | `Transparent` | 该键已加载，内容为空 |
| cache | `Filled(Chunk)` | 有内容 |

不变式：`cache[0]` 是 raw 的渲染视图；`Associated` 表示两者内容一致。"cache 领先于 raw" 只在笔画期间合法（笔画结束由 `merge` 把 raw 收成 `Associated`）。

## 1.2 四组流程与对应函数

| 阶段 | 步骤 |
|---|---|
| batch-draw | `DrawsBatch::draw`（`brush.rs:302`）：`upload` → `draw_prepare_scratch` → `draw_into_scratch` → `raw_remove_associated` → `cache_generate_with_scratch` → `cache_mipmaps`，单 encoder 单 submit |
| stroke-submit | `Stroke::submit`（`brush.rs:99`）：`upload` → `raw_remove_associated` → `merge`（raw→`Associated`）→ `cleanup` → `cache_recycle_raw`（被 `merge` 遮住，见 R20）；`traveler.stock` 在此之前由 `wrapper.submit` 单独提交 |
| stroke-discard | `Stroke::discard`（`brush.rs:133`）：`cleanup` → `raw_remove_associated` → `cache_generate_without_scratch`（回滚）→ `cache_mipmaps` → `cache_recycle_raw` |
| redo/undo | `Traveler::undo/redo`（`traveler.rs:185/280`）：`stroke.upload` → `raw_remove_associated` → place（atlas 拷贝写进 raw）；随后 `wrapper.rs:473/506` 做 `cache_generate_without_scratch` + `cache_mipmaps` + `cache_recycle_raw` |

就绪门禁：`DrawsBatch::satisfied`（`brush.rs:359`）在 `wrapper.rs:343` 检查 raw key 与 **全部 LOD 层**的 cache key 是否都存在，不满足则丢弃这一批（不 `confirm_batch`）。

---

# 2. 剩余缺陷

## P0

### R1. `still_has_tasks` 用了 `&&`，raw 队列会被永久搁置

```rust
// layer/stream.rs:670
fn still_has_tasks(&self) -> bool {
    self.cache_front < self.cache.len() && self.raw_front < self.raw.len()
}
```

`recv()`（`stream.rs:244-256`）用它决定继续干活还是**阻塞** `input_rx.recv()`。`&&` 的含义是"只有两个队列都有活才算有活" → cache 队列一被走完线程就 park，即使 `raw_front = 0` 一个 raw 都没加载。

叠加 `generate_batch` 的另一个问题（`stream.rs:643-666`）：两个循环**共用** `batch_cnt`，cache 循环先把 8 个名额占满，于是 raw 循环条件立刻为假 → 启动时（cache key 全新）raw 每个 batch 拿到 **0** 个 → cache 走完 → park → `raw.chunks` 恒空 → `satisfied` 恒假 → **"渲染有但画不了"**，直到某个输入（相机更新、切页、autosave）把它唤醒。

- 期望：`||`（一行）。
- 期望（可选）：给 raw 独立预算，让 raw 与 cache 并行加载，而不是严格排在 cache 之后。
- 备注：`restock` 的 raw 门禁是 `camera.center.2 <= 1`（`stream.rs:620-624`），所以 level ≥2 时不加载 raw 是**既定策略**（见 R7），但 level 0/1 必须能加载。

### R2. traveler 不支持透明区块（快照缺节 / 静默部分撤销）

三处仍是：

```rust
Some(RawLayerChunk::Transparent) => { log::error!("todo: transparent chunk"); continue; }
// traveler.rs:140 (stock) / 224 (undo) / 319 (redo)
```

- **stock 侧**：空白区块（raw 与 cache 都是 `Transparent`）无法入栈 → 该区块的撤销永远不生效（"在空白处画第一笔"就是这种场景，因为那时 raw 是 `Transparent`）。
- **语义缺口**：`SnapshotSection` 只能表达"把 atlas 里的旧内容拷回某区块"，无法表达"这一块当时是空的"。所以这不是加一个 match 分支，需要新增一种 section 状态（例如 `enum SectionData { Atlas(..), Empty }`），还原时对应走 `clear_chunk`。
- **undo/redo 侧**：`continue` 只跳过该节，函数仍返回 `Some(Stroke)` → 部分撤销，用户无感知。
- 已缓解：`stock` 现在在 `sections.is_empty()` 时不再入栈（`traveler.rs:161-165`），因此不会再有"空快照吃掉一次 Ctrl+Z"的情况，`undo_available` 也会返回 false。

### R3. 中途保存会让 DB 快照不自洽（已知，进行中）

笔画进行中落盘时：cache 是"这一笔的中途状态"，raw 可能是笔画前的 `Filled`（若该批做过物化），而 `save_raw_transparent` 是**删行**（`chunk.rs:129-132`）→ 重启后两侧内容不一致，`(cache Filled, raw Transparent)` 之类的组合会直接丢掉矩形内的内容（见 R10）。

- 顺带一个能同时缓解性能与这个问题的改动：**replace 笔刷的 batch 路径根本不需要 raw 做基准**。`cache_generate_with_scratch`（`brush.rs:499+`）的 replace 分支只用 scratch + cache；`stock` 自己会先 `raw_remove_associated`（`traveler.rs:80-92`），所以 replace 笔刷完全可以在 batch 阶段不物化 raw。
  - 好处：省掉每批每块一次 512² 拷贝（R16）；中途保存时 raw 保持 `Associated`（= "raw 的内容就是 cache[0]"）→ 快照自洽，只是这一笔被截断。
  - 代价：`cache_generate_with_scratch` 的 `(Filled cache, Associated|AssociatedCached)` 分支需要对 replace 模式放行（改成 `copy_chunk` 到 cache），而不是现在的 `log::error!`。
- 你也提到的另一条路：`docs/draw.md:106-127` 的"先通知卸载、主线程确认后再读盘"。

### R4. 版本与旧存档迁移仍是空的

- `DATABASE_VERSION = 0`（`save.rs:22`），`migrate_format` 是 `unimplemented!("unsupported migration")`（`save.rs:115-127`）→ 一旦 bump 就会 panic；当前因为恒等所以不可达。
- `legacy_classic`（`chunk.rs:166+`）：旧表 `stroke_chunk` / `stroke_chunk_meta`、`CHUNK_META0_FORMAT = 3`、以及 0→3 的 sRGB/alpha 迁移**全部被注释掉** → 0.4.0 及更早的存档（表名、格式、色彩空间都不同）没有读取路径。
- `CHUNK_VERSION = 0`（`chunk.rs:5`）只做 per-chunk 版本；`load_*` 遇到未知版本返回 `Err("not supported version")` → 走兜底当空块（现在至少会打 error 日志），属于**静默丢内容**而非显式拒绝。

### R5. `merge` 无条件把所有 scratch key 声明为 `Associated`

`brush.rs:168-181`。`cache_generate_with_scratch` 因 cache/raw 未加载而 `log::error!` + `continue` 的那些 key 也会被声明"raw == cache[0]"，并且 raw 的 `Filled` 纹理（那一刻内容的唯一副本）被直接丢弃 → 内容丢失。

- 现在 `satisfied` 门禁把这条路径的概率压得很低（门禁要求 cache key 存在），但**流线程可以在门禁通过之后把区块淘汰掉**（`CacheRemove`/`RawRemove`，见 R11/R12 的淘汰策略）→ 竞态窗口仍然存在。
- 期望：随 fallback（`docs/draw.md:84-85` 方案二"悬挂 stroke"）一起，只对"确实写好 cache 的 key"做关联。

## P1

### R6. 撤销/重做依赖 raw 与 level 0 cache 都在内存里

`traveler.rs:110-124`（stock）、`181-195`（undo）、`273-287`（redo）遇到 `None`/`Transparent`/`Associated` 都是 `continue`：

- 少一节的后果是**部分撤销**（用户看不出为什么只恢复了一半）。
- `stock` 的 `continue` 发生在 `self.alloc(...)` **之后** → 该节占用的 atlas 空间不归还（`traveler.rs:93-124`）；undo/redo 里 `continue` 会跳过 `section.atlas_id` 的解分配（循环末尾才 dealloc）→ 同样泄漏。
- 期望：要么在快照/还原前补齐依赖（阻塞或请求加载），要么让 `undo_available` 能识别"依赖缺失"并直接拒绝这次操作。

### R7. `satisfied` 在缩小时恒假（已确认为既定策略）

`brush.rs:359-373` 要求 dirty 矩形在**所有层 `0..mipmap_cap`** 的 cache 块都存在，而 `restock`（`stream.rs:624`）只请求 `camera.center.2.saturating_sub(1)..caps` → 相机 level ≥2 时低层永远不存在 → 门禁恒假。

- 既定策略：缩小时不可能把 level 0 全加载进来（内存会炸），留给主动区块请求。已在 R11/R12 记录内存侧的问题。
- 两个附带影响，值得写在实现里：
  1. 门禁在 `confirm_batch` **之前** return（`wrapper.rs:343-347`）→ 被丢弃的批不推进 `prev` → 恢复绘制时 `processor_output(prev_old, to)` 会把中间那段补成一条直线段。
  2. 门禁只是"就绪"检查（`contains_key`，`Transparent` 也算存在）→ 它挡不住状态机层面的内容错误（例如之前的 `load_raw` flag bug 就骗过了它）。

### R8. 流线程出错会静默终止

`StreamThread::run`（`stream.rs:161-242`）返回 `Result`，`load()` 里的 `?`（读事务失败、表打不开、`ChunkLoad::new` 出错）会让 `run` 直接返回 `Err` → 线程结束，主线程继续运行但**不再加载也不再保存**（也不会有任何重试或用户可见提示）。期望：至少 `log::error!` + 降级重试（例如跳过这一批）。

### R9. 非 `STORAGE_READ_WRITE` 设备上绘制管线不成立

- `merge_pipelines`（`layer.rs:1431-1445`）的 `read_write: bool` 参数**已不再使用**，管线布局硬编码为 `[dispatch, read, read, write]`（会有未使用参数 warning）。
- `brush_pipelines`（`layer.rs:1548-1567`）仍保留 `read_write == false` 的 `[dispatch, read, write]` 布局，但 `draw_chunk_read_write`（`layer.rs:503-517`）在 group 1 与 group 2 上绑的都是 `chunk.read_write`（非 RW 设备时它等于 **read** 布局）→ 与 `write` 期望不匹配。
- 结论：`support_read_write` 目前是"存了但只影响 bind group layout 构造"的半成品；`merge_chunk_over_read_write`（`layer.rs:540`）与 `draw_chunk_swap`（`layer.rs:480`）都没有调用者。

### R10. 已接受的 desync 组合（记录在案，避免重复讨论）

| 组合 | 代码行为 | 何时有害 |
|---|---|---|
| `(cache Filled, raw Transparent)` | `raw_remove_associated` 显式空分支放过（`layer.rs:281-284`）；`cache_generate_with_scratch` 按"底下是空"重写 dirty 矩形（`brush.rs:551-560`，replace/非 replace 都是 copy） | 正常态（空区块起笔，batch ≥2）下这是**正确**的；只有 DB 里出现"cache 行有内容 + raw 行缺席"（中途保存、或 `load_raw` flag 反了那类历史库）时才会丢一次矩形内容 |
| `(cache Transparent, raw Filled)` | 新建 cache 块只写 dirty 矩形，其余为空（`brush.rs:561-591`）；随后 `merge` 丢掉 raw 的纹理 | 属于"cache 未加载"的加载顺序问题 → 矩形外内容丢失；同属主动请求待办 |

---

## P2 资源 / 性能 / 落盘

### R11. raw 队列无上限 + 淘汰保护队列内 key → 上限形同虚设

- `restock`（`stream.rs:613-639`）把可见矩形内的所有 level-0 块塞进 `queue.raw`（`// TODO overflow detect`）。zoom 每降一级可见世界矩形按 2× 放大 → level-0 块数按 **4×** 增长。现在的门禁 `center.2 <= 1` 只把它限制到 level-1 的面积（1440p 约 240 块 ≈ 240 MiB）。
- `unload_overflow`（`stream.rs:429-460`）跳过"仍在队列里的 key"：

  ```rust
  if (self.queue.raw).contains(self.base.raw.get_index(frnt).unwrap().0) { continue; }
  ```

  而队列在整个 restock 周期内不会移除 key → **本轮加载进来的块全部受保护、一个也淘汰不掉** → `CHUNK_RAW_REAL_CAPS/HARD_CAPS` 实际上不生效，常驻量会长到队列大小。
- 期望：restock 只入队最近的 K 个（K 与 caps 同量级）；淘汰改成纯距离序（不需要保护"即将加载"的 key，队列本身就是计划）。

### R12. `CHUNK_RAW_REAL_CAPS = 64` 偏小

一个 4K / 大窗口在 zoom 1 的 level-0 可见矩形就可能 40～60+ 块，极限情况下超过 64 → 即使不缩放，raw 也会"加载→淘汰→再加载"抖动（每块都有 GPU 纹理创建 + zstd 解码 + 淘汰时的回读编码）。而 `satisfied` 要求 raw key 常驻 → 被淘汰的位置直接画不了。期望：cap 至少按"一个 level-0 可见矩形 × 安全系数"定。

### R13. mipmap 就地写上层块不标脏 → 重启后 LOD 陈旧

`cache_mipmaps` 的 `(Filled, Filled)` 分支直接写上层块（`layer.rs:309-314`），没有 `cache.swap_chunk` 也没有 `MarkCacheUnsaved` → 上层 LOD 的更新不落盘，重启后缩小视图会看到旧内容（直到那一带重画）。另两个分支走 `swap_chunk` ✓ 有标记。属于派生数据，但恢复不出来只能靠重画。

### R14. `raw_insert` / `cache_insert` 不处理被替换掉的旧值 → 计数漂移

```rust
// stream.rs:569 / 576
fn raw_insert(&mut self, key: (i32, i32), chunk: RawLayerChunk) {
    if matches!(chunk, RawLayerChunk::Filled(_) | RawLayerChunk::AssociatedCached(_)) {
        self.raw_real_cnt += 1;
    }
    self.raw.insert(key, chunk);   // ← 旧值被丢弃，没有对应的 -1
}
```

`SwapRaw` 可能覆盖同一个 key（`Associated` → `Filled` → `AssociatedCached` → …）→ 计数只增不减（例如 `Filled` → `Associated` 之后永远 +1）→ 漂移，进而触发多余的 `unload_overflow`。你把 `AssociatedCached` 纳入计数是对的 ✓，但要配合 `if let Some(old) = self.raw.insert(..)` 的反算（`cache_insert` 同理，虽然目前没有把 `Filled` cache 换成 `Transparent` 的路径）。

### R15. 泄漏

- `ChunkPool::pop_chunk_stack`（`layer.rs:652`）**从未被调用** → `pool.list` 只增不减（`cleanup` 每批 push，`brush.rs:183-205`），每块 512² 纹理。
- `traveler`：`alloc` 之后 `continue` 的路径不归还 atlas（`traveler.rs:93-124`；undo/redo 里同样跳过 `section.atlas_id` 的 dealloc）。

### R16. 每批固定开销

- `raw_remove_associated`（`layer.rs:245-283`）对 dirty 矩形内每个 `Associated` 块做一次 512² 全块拷贝（`Filled` 走空分支 ✓）。replace 笔刷可以整个跳过（见 R3）。
- `satisfied` 每批扫 raw ×1 + cache ×层数（8 层）；`MarkCacheUnsaved` 每批每块一次 channel send（`brush.rs:228/523`）；`cache_mipmaps` 每批枚举 `0..mipmap_cap-1` 层并对每个缺失层打 `log::error!`（`layer.rs:301/305` —— 上层缺失是**常态**，会刷日志并掩盖真正的错误，建议降级为 debug 或静默）。

### R17. 落盘路径

`persist_raw` / `persist_cache`（`stream.rs:490-552`）每块一次 `device.poll(wait_indefinitely)`（全设备同步）+ 纹理回读 + zstd 编码；`brush_preview` 每次刷新用 4 个 encoder 多次 submit（`widgets/brush_preview.rs`）。批量/异步化留待以后。

## P3 死代码与文档

### R18. 死代码清单

| 位置 | 说明 |
|---|---|
| `layer.rs:480` `draw_chunk_swap` | 无调用者 |
| `layer.rs:540` `merge_chunk_over_read_write` | 无调用者 |
| `layer.rs:652` `ChunkPool::pop_chunk_stack` | 无调用者 |
| `layer.rs:788` `lower_chunk_of` | 无调用者 |
| `layer.rs:1433` `merge_pipelines` 的 `read_write` 参数 | 未使用（warning） |
| `save/chunk.rs:156` `ChunkSave::is_transparent` | 未使用 |
| `save/chunk.rs:25` `ChunkLoadResult.unsaved` | 只写不读（`stream.rs:303/334` 之后无消费者） |
| `save/chunk.rs:166+` `legacy_classic` | 整模块只被注释引用 |

### R19. 文档漂移

- `docs/save.md`：raw 表格式没有写明 **flag=1 表示没有 payload**（这次的 `load_raw` 事故正好撞在这里）；建议补一句并给出 flag=0/1 的完整布局。
- `docs/draw.md`：
  - `68-74` 失败路径：1/2 已由 `satisfied` + `confirm_batch` 顺序实现，3（主动请求）未做；
  - `100` 的优化（replace 不必 associate）未应用，注意它现在**是可行**的（`stock` 会自行物化），见 R3；
  - `102-104` 中途保存（进行中，见 R3）；
  - `106-127` 的流线程同步：`SetPage` 目前是 `unload_all()` 后立刻换页（`stream.rs:165-169`），主线程还没确认就被移除 → 主线程后续的 `SwapRaw/SwapCache` 会带着**已更新的** `config.page` 落盘，可能把上一页的改动写进新页；
  - `19` 行"raw 语义回收不必回收"与实际调用点（`submit` 里被 `merge` 遮住，见 R20）不一致。
- `brush.rs:487-491`：bridge 笔刷的 standalone 预览是 TODO → blur/smudge 的笔刷预览目前画不出东西（你在 commit message 里标了 temporary）。
- `docs/widget.md` 未复核。

### R20. 次要

- `Stroke::submit` / `Stroke::discard` 在 `prev.is_none()` 时早退、不清理 scratch（`brush.rs:105-108/139-142`）。
- `set_page`（`wrapper.rs:539-544`）只 `traveler.clear()`，不清 `stroke` / `scratch` / `pool` → 切页时若有 in-flight stroke，`merge` 会把上一页的 key 声明成新页的 raw 行。
- 笔画中途按 Ctrl+Z / Shift+Z 无互斥（`input.rs:113-123` 不看 `self.stroke`）；`undo`/`redo` 现在会调用 `cache_generate_without_scratch`，与正在进行的 stroke 会互相覆盖。
- `cache_mipmaps` 的 `cache.mipmap_cap - 1`（`layer.rs:294`）在 `mipmap_cap == 0` 时 u8 下溢（当前恒为 8，不可达）。
- `submit` 里 `merge` 抢在 `cache_recycle_raw` 之前 → 该调用恒空转（`brush.rs:174-180`）。按 `docs/draw.md:37` 这是有意的（submit 后"一定是 associated"），可以直接删掉这个调用或改由它承担回收。
- `merge` 里新增的 `MarkRawUnsaved`（`brush.rs:176-179`）与紧跟的 `swap_chunk` 重复（`swap_chunk` 已经发 `SwapRaw`，流线程会 `raw_unsaved.insert`）。

---

# 3. 建议顺序

1. **R1**（一行）→ 启动即可绘制。
2. **R3 + R5**（中途保存 + fallback）：一起做，把"cache 没写好却声明 associated"彻底堵上；顺带做 replace 免物化（同时缓解 R16，并让中途保存快照自洽）。
3. **R2**（transparent section）→ 撤销完整性。
4. **R11 / R12**（raw 队列上限与 caps）→ 内存与抖动；这一步做完，主动请求（R7 后续）才有干净的地基。
5. **R6**（依赖缺失时阻塞或拒绝）+ 主动区块请求。
6. **R13 / R14 / R4 / R8 / R9**，最后 **R15–R20**。

---

# 4. 本轮已修复（对照）

| 项 | 位置 |
|---|---|
| `raw_remove_associated` 物化后没写回 raw（导致 raw 恒为 `Associated`、后续全线 `log::error!`） | `layer.rs:271` |
| `undo()` 缺 cache 0 生成 + `cache_mipmaps`（与 redo 不对称 → 撤销不可见） | `wrapper.rs:473-505` |
| undo/redo 未写 `work_rect`（cache 生成被陈旧 dispatch 矩形裁剪） | `traveler.rs:169-180 / 262-273` |
| `discard` 未回滚 cache（弃笔内容留在画面与盘上） | `brush.rs:155-160` |
| `cache_recycle_raw` 的 key 集合改用 dirty 矩形（`AssociatedCached` 从此有真实构造点） | `brush.rs:282-289` |
| `load_raw` 的 flag 语义写反（flag=1 无 payload，flag=0 才有）→ `Filled`/`Associated` 全被当成 `Transparent` | `chunk.rs:48-54` |
| 读失败兜底补上 `log::error!`（不再静默吞掉格式错误） | `stream.rs:295-305 / 326-336` |
| cache 就地写入未标脏（绘制到已加载块时不落盘）→ 补 `MarkCacheUnsaved` | `brush.rs:228 / 523` |
| `cache_mipmaps` 用错管线（clear 而非 mipmap）+ 除数应为 `1 << upper_key.2` | `layer.rs:292 / 298` |
| `confirm_batch` 首帧 dirty 为零尺寸（单批笔画无法撤销、快照为空） | `brush.rs:84-88` |
| batch 阶段合并为单 encoder / 单 submit；`draw_active` 与 `draw_erase` 合并为 `draw_brush` | `brush.rs:302-320`、`wrapper.rs:327-349` |
| `raw_remove_associated` 的 match 改为穷尽（8 种组合逐一表态，编译期约束） | `layer.rs:264-284` |
| 新增 `satisfied` 就绪门禁 + 丢弃批前不 `confirm_batch` | `brush.rs:359-373`、`wrapper.rs:343-347` |
| `stock` 自行 `raw_remove_associated`，并在 `sections.is_empty()` 时不再入栈 | `traveler.rs:71-93 / 161-165` |
| `raw_real_cnt` 纳入 `AssociatedCached` | `stream.rs:569-588` |
| `raw` 加载在 `center.2 >= 2` 时停止（内存策略） | `stream.rs:620-624` |
