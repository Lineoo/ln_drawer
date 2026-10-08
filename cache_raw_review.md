# `a2ec5f1 cache+raw layer system` 缺陷清单

范围：仅列本次提交引入（相对 `2b71d34`）的有关缺陷。格式：`旧版本行为 → 本版本行为 → 期望`。

---

## A. 绘制链路（P0，功能不可用）

1. (DONE) **绘制目标区块根本不创建** — `layer/wrapper.rs` / `layer/brush.rs`
   - 旧：`DrawPipeline::prepare_scratch` 用 `layer_prepare_rect` + `ChunkPool::pop` 在 scratch 层补齐 `batch.dirty` 覆盖的区块（替换模式还先从 raw/主层拷入副本）。
   - 新：没有任何代码调用 `ChunkPool::pop_chunk_stack`，`PlainLayer.chunks` 永远为空，`DrawsBatch::draw_layers` 中 `scratch.chunks.get(&key)` 永远 `None`。整个绘制循环空转；`pool` 只被 `push` 从没被 `pop`。
   - 期望：绘制前在 scratch 层按 dirty 补齐区块（替换模式拷入源数据），笔刷结果才会落到 scratch 并参与渲染。

2. (INVALID) **`DrawsProcessor::processor_output` 立刻 panic（越界写）** — `layer/brush.rs:104-108, 356-388`
   - 旧：`Vec<Self::Draw>` + `out.push(...)`，容量 0 时也能分配。
   - 新：`Stroke::batch` 传 `Vec::new()`（容量 0）给 `&mut Vec<u8>`，`processor_output` 在 `out.len() > limit` 检查后直接 `out.extend_from_slice(bytes_of(&processed))`；此时 `len == 0` 而元素 ≥ 12 字节，必然触发 *capacity overflow* panic。现实绘制路径 100% 命中（预置笔刷最大 Draw 28 字节 > 最小非零容量 8）。
   - 期望：调用方预分配（如 `Vec::with_capacity(DRAWS_ARRAY_CAPACITY)`），或按 `len + size_of::<Draw>() > cap` 判界。

3. (DONE) **笔画 `init` 反转，`start` 永远为 false** — `layer/brush.rs:46-89`
   - 旧：`self.batch.start = self.stroke.is_none()`，笔画的第一批 `start = true`，`prepare_stroke` 在笔迹开始前执行一次。
   - 新：`Stroke::new` 里 `init: false`，而 `batch()` 却在 `else if self.init { self.init = false; start: true }`。新建笔画的第一批 `start = false`，之后也永远为 `false`。
   - 后果：`smudge_prepare_stroke`（清空 `draws_state[0]` 携带色）永不执行，晕染颜色跨笔画残留；这是 `paint_stroke()` 生成后 `draws_state` 携带色被后续笔画复用的直接原因。
   - 期望：`init` 初始为 `true`（或改回 `stroke.is_none()` 语义），使每笔画首批 `start == true`。

4. (DONE) **首个笔迹点丢失 / 轻触无任何反应** — `layer/brush.rs:46-64`
   - 旧：`prev = None` 时直接 `draws.push(brush.process(target))`，落笔点必有一个 dab。
   - 新：`Stroke::new` 把 `prev` 设为落笔点，第一次 `batch(target=落笔点)` 时 `from == to`，`processor_output` 循环 0 次 → 无 dab、`dirty` 为零矩形 → `batch()` 返回 `None`。点到抬笔（不移动）什么都不画；落笔点只有在第二次事件、且路径从起点重新扫过时才补上。
   - 期望：每笔画至少落下落笔点的第一个 dab（`prev` 应为 `Option<Draw>` 或首批强制 `process(target)`）。

5. (DONE) **`draw_erase` 混用两支笔刷** — `layer/wrapper.rs:335-365`
   - 旧：`self.erase.draw(&mut self.draw, &self.main, draw)`，擦除实体、管线、替换/桥接标志全部来自 `erase`。
   - 新：`Stroke::new(&self.erase, draw)` 只用来取 `replace/bridge`，而 `stroke.batch(self.active.as_ref(), draw)` 与 `draw_layers(..., self.active.as_ref())` 用的是当前预设笔刷。于是"长按擦除"实际用当前笔刷（如 smudge/blur，`bridge_mode() == true`、`replace_mode() == true`）的 dab 与管线去擦除，而 `Stroke.bridge` 却来自 `erase`（false），桥接块既不准备也不清理。
   - 期望：擦除路径全程使用 `erase`（或让 erase 与 active 的 `replace/bridge` 语义显式统一）。

6. (DONE) **`request_stream` 被删除，区间未加载时整批绘制静默丢弃** — 旧 `brush.rs::request_stream` / 新 `stream.rs:213-218`
   - 旧：每批绘制后按 `stroke.dirty` 对各 mip 层发送 `ThreadInput::RequestReal(key)`（下级已加载才请求上级），并把范围外区块拉进来。
   - 新：`RequestRaw`/`RequestCache` 处理分支是 `// TODO` 空实现，且 `LayerPage` 侧根本不再发送；新区块只能靠相机队列里的 cache 被动加载。
   - 期望：绘制前确保 raw + cache + scratch 三者覆盖 dirty（issue 里"检查相关区块全部加载，否则请求并放弃这一批"）。

7. (DONE) **`MarkRawUnsaved` / `MarkCacheUnsaved` 无发送方，脏块不落库** — `stream.rs:205-212`，`wrapper.rs`
   - 旧：`submit()` 结束后对 dirty 覆盖的所有 `(x, y, level)` 发送 `MarkUnsaved`。
   - 新：两个输入变体没有任何调用点，`ChunksBase::raw_unsaved` / `cache_unsaved` 恒为空集。
   - 后果：`persist_raw` / `persist_cache` 首行 `if !unsaved.contains(&key) { return }` 使所有持久化路径失效（见 D 组）。
   - 期望：绘制/撤销后标记改动区块为 unsaved。

---

## B. cache / raw 一致性（P0，画完不显示）

8. (DONE) **raw 的修改从不回写 cache，渲染看不到新笔画，undo/redo 无视觉变化** — `wrapper.rs:376-390, 515-557`
   - 旧：被编辑的 `main` 层既是数据源又是渲染源，`submit` 直接改它；`merge` 层提供笔迹预览，渲染按 `merge > scratch > main` 回退。
   - 新：`stroke.submit_layers()` 只把 scratch 合成到 `raw`，之后 `cache_mipmaps(&self.cache, dirty)` 只更新 **cache 自身**的高层 mipmap；cache 的 level 0 没有任何刷新路径，渲染又只读 cache（`render()` 中 `CacheLayerChunk::Filled`）。撤销/重做同样只改 raw。
   - 期望：raw 变更后同步 cache level 0，再由 cache 生成 mipmap，渲染始终与 raw 一致。

9. **`Associated` 不变量失效** — `layer.rs:109-122`，`brush.rs:104-127`，`wrapper.rs:467-479`
   - 旧：`RawLayerChunk::Associated` 表示"数据等于对应 cache level 0 区块"，两者同步演进。
   - 新：绘制会直接改写 raw 区块，cache level 0 不变；`ThreadOutput::RawInsert` 也不更新 cache。于是 `Associated` 区块的实际内容可能与 cache 不一致；反之 `cache_mipmaps` 从 level 0 重新下采样时会把已被 raw 修改的内容覆盖回旧值。
   - 期望：明确谁是权威（raw），并在提交时把 raw 区块内容写回 cache level 0 / 或把状态降级为 `Filled`。

10. (DONE) **cache 的 level 0 无来源，mipmap 链无法建立** — `stream.rs:283-346`
    - 旧：主层自身被加载并生成 mipmap（`generate_mipmaps` 在编辑后按 `main.mipmap_levels` 逐级下采样）。
    - 新：`load()` 只把磁盘上的 cache 区块放进来，绘制结果不进 cache，也没有"由 raw 生成 cache level 0"的步骤，`cache_mipmaps` 又只在上下层都已 `Filled` 时才 dispatch。干净启动后 cache 里只有磁盘残留的层级，LOD 链基本不存在。
    - 期望：raw 提交时生成/刷新对应 cache level 0，再逐级生成 mipmap。

---

## C. 缓存图层纹理尺寸（P0，mip 渲染与生成均错）

11. (INVALID) **所有 mip 层级的 cache 区块都用第 0 级像素尺寸创建**
    - 旧：`chunk_prepare` 用 `create_chunk_texture(UVec2::splat(chunk_size << z))`，第 z 级区块纹理是 `(chunk_size << z)²`，与 `chunk_to_rect` 的世界范围一致。
    - 新：`stream.rs:291-297`（cache 分支）统一用 `self.config.chunk_size` 创建纹理，但 `chunks_stack_key_to_rect(key, ...)` 给出的是 `(chunk_size << z)²` 的世界矩形。cache 区块的 rect 与纹理尺寸彻底失配。
    - 后果：第 z ≥ 1 级区块渲染时，`render.wgsl` 用 `chunk.size`（= `512<<z`）铺满屏幕四边形，却只采样 512×512 的纹理 → 内容被放大 `2^z` 倍（读越界靠 clamp）。同时 `mipmap.wgsl` 的 `dst_coords = position / scale`（scale 恒为 2）会越出 512×512 目标纹理，写出未定义内容。
    - 期望：第 z 级 cache 区块纹理为 `(chunk_size << z)²`（或让 rect/纹理/`scale` 三者自洽）。

---

## D. 流式加载与持久化（P0，存档写不进去 / 读不出来）

12. **raw 图层完全不加载、不请求**
    - 旧：`stroke_chunk` 表就是主层，按相机与 `RequestReal` 加载。
    - 新：`LoadQueue::restock` 只填充 `queue.cache`；`queue.raw` 只在 `restock` 里被 `clear()`，没有任何 `insert`；`consume` 的 raw 分支因此永不产出 `ChunkKeyKind::Raw`。唯一的补齐通道 `RequestRaw` 是 TODO。
    - 后果：`LayerPage.raw.chunks` 永远为空 → 拾色（`pick_color` 只认 `RawLayerChunk::Filled`）静默失效、撤销/重做（Traveler 只认 raw）失效、绘制无处落笔、原有作品不再显示。
    - 期望：raw 与 cache 一样由相机队列 + 显式请求加载。

13. **`ChunksBase::raw_unsaved` / `cache_unsaved` 从不写入 → 区块永远不保存**
    - 旧：`unload`/`unload_all`/`autosave` 都会检查 `base.unsaved` 并回读纹理写库；`write_chunk_data` 还会把"全透明"区块从表中删除。
    - 新：`persist_raw`/`persist_cache` 的 `if !unsaved.contains(&key) { return }` 永远为真 → `RawLayerChunk::Filled` 分支被跳过；`save_all()` 又只遍历 `cache_unsaved`（恒空）。
    - 期望：绘制与加载后标记 unsaved，落盘。

14. **绘制结果在溢出卸载 / 切页时被丢弃**
    - 旧：`unload` 对 unsaved 区块回读并写库，随后从内存移除。
    - 新：`unload_overflow`/`unload_all` 只调用 `persist_*`，而它因 unsaved 为空直接返回 → 未保存的 raw 内容随内存释放消失；`set_page` 的 `unload_all` 同样丢数据。
    - 期望：卸载前把 unsaved 内容持久化。

15. (DONE) **`autosave` 不覆盖 raw 图层**
    - 旧：`autosave` 遍历整个 `base.unsaved`（单一图层）写 `stroke_chunk`。
    - 新：`save_all()` 只收集 `cache_unsaved`，完全没有 raw 任务；`ThreadInput::Autosave` 因此不可能保住原图层。
    - 期望：autosave 同时落盘 raw + cache。

16. (DONE) **透明 / 伴生区块的落盘无 `unsaved` 判定，会删掉未改动数据** — `stream.rs:503-509`
    - 旧：一整套 `unsaved` 守卫。
    - 新：`RawLayerChunk::Associated` → `save_raw_associated`、`Transparent` → `save_raw_transparent`（执行 `table.remove`）都在守卫之后；只要卸载一个未加载数据的 raw 区块，就会把它在库里的记录删掉或覆盖成伴生标记。
    - 期望：透明/伴生状态变更需先标记 unsaved 再落盘。

17. **加载遇错即崩** — `stream.rs:286-340, 172`
    - 旧：`zstd::decode_all` 失败被 `if let Ok(...)` 丢弃，损坏区块只当作未加载。
    - 新：`load_cache`/`load_raw` 对无法解析的 4 字节 version 或 zstd 解压失败返回 `Err`，`load()` 用 `?` 上传，`run()` 用 `?` 返回，最终 `wrapper.rs:143-147` 的 `.unwrap()` panic 掉整个进程。
    - 期望：单个区块损坏只跳过并记录日志。

---

## E. 存档迁移与兼容（P0，升级即丢画 + 可能崩溃）

18. **旧表迁移完全未实现，只留注释占位** — `save/chunk.rs:163-218`
    - 旧：`stream.rs` 里 `chunk_migration` 有 0→3 的 `format` 迁移（sRGB 编码、移除废弃 mipmap 字段、乘 alpha 预乘还原），并且读不到 meta 时会"直接按当前格式读取"以挽救损坏区块；旧表 `stroke_chunk` + `stroke_chunk_meta` 就是数据本体。
    - 新：`legacy_classic` 模块只有两个 `TableDefinition` 常量，`chunk_migration` 整个函数体被注释掉，没有任何读取 `stroke_chunk` 并写入 `layer_chunk` / `layer_chunk_raw` 的代码。
    - 期望：按 issue 描述"读取旧表，全量升级迁到新表，并给原始图层补一个伴生记录"。

19. **上一版刚写出的存档也读不回来，并且会崩**
    - 旧（`2b71d34`）：`write_chunk_data` 把 `zstd::encode_all(bytes)` **不带头部**写入 `stroke_chunk`；`layer_chunk` 表那时只是新增的 `TableDefinition`，没有写入路径。
    - 新：只读 `layer_chunk` / `layer_chunk_raw`（`save/chunk.rs:7-10`）。旧库存的裸 zstd 数据既不在新表里，若被当作新格式解析，前 4 字节会被读成 `version` 而失败 → 按第 17 条直接 panic。
    - 期望：至少能识别旧表 + 旧编码并迁移；无论如何不得因旧数据 panic。

20. **`save.rs` 与数据库版本号都停在旧状态** — `save.rs:21-22, 34-41, 115-131`
    - 旧：文档注释表格描述 `stroke_chunk` / `stroke_chunk_meta`；`DATABASE_VERSION` 为 0，`migrate_format` 的循环体是 `unimplemented!`。
    - 新：注记与表结构都没随本次重构更新；`Scene` 里没有"是否已迁移到 cache/raw"的状态位，`migrate_format` 依旧不会执行任何迁移。
    - 期望：表文档随 `docs/save.md` 更新；通过版本号或显式标记驱动一次性迁移，而不是 `unimplemented!`。

21. **`CHUNK_VERSION` 与文档不一致，raw 的 `associated` 只有 0/1 两种写入方式** — `save/chunk.rs:5, 103-124`；`docs/save.md`
    - 文档写 "version 版本 v1"，代码 `CHUNK_VERSION = 0`；`save_raw` 恒写 `associated = 0`，只有单独的 `save_raw_associated` 能写 1。
    - 期望：文档与实现对齐。

---

## F. 其他退化与隐患

22. **最大缩放层级无数据** — `layer.rs:43, 297, 605` vs `stream.rs:602`
    - `DEFAULT_MIPMAP_CAP = 8`；`restock` 加载 `center.2-1 .. 8`（即最大第 **7** 级），`cache_mipmaps` 经 `rect_chunks_stack_iter(.., cap-1)` 只生成到第 **7** 级；而 `render()` 在最大缩放时请求 `actual_mipmap = min(zoom, cap-1) = 7`。生成/加载的层级与请求的层级边界不一致。
    - 期望：LOD 各级都保证被生成或加载，或渲染按实际存在的最高层级回退。

23. **`unload_overflow` 在候选都在队列中时空转** — `stream.rs:414-446`
    - `continue` 分支不递减 `frnt`，若 `cache_overflow()` 持续为真会反复扫描同一批条目（不致命，但每帧空转）。
    - 期望：记录有效卸载数，无进展即提前退出。

24. **`restock` 的 `debug_assert!(self.cache.len() < CHUNK_CACHE_REAL_CAPS - 1)` 容易被真实视锥触发** — `stream.rs:608`
    - 该 assert 统计的是"队列长度"而非"真实纹理数"，缩放到 0 级附近时 512 区块的视锥很容易超过 511 个条目而在 debug 构建 panic。
    - 期望：断言应对齐真实容量语义，或改为运行时降级。

25. **重构残留死代码** — `layer.rs:124-129`（`Layer` 结构体已无任何构造点）、`layer.rs:100/114`（`CacheLayers.thread_tx`、`RawLayer.thread_tx` 无人使用）、`layer.rs:137-140`（`ChunkPool.chunk_size` 无人读）、`save/chunk.rs:22-26`（`ChunkLoadResult.unsaved` 恒 false）、`brush.rs:162-191`（`submit_layers` / `submit_standalone` 无调用者，且 `submit_layers` 还带一大段注释掉的 swap 分支）。
    - 期望：清理或补齐。

26. **文档/注释与实现脱节**
    - `widgets/brush_preview.rs:37-41` 仍称使用 `DrawPipeline::draw_standalone`；`wrapper.rs:56-62` 的 `LayerPage` 文档仍描述 "main / merge / mipmap 三层、mipmap 尚未实现"；`save.rs:34-37` 的表结构与实际表不符；`docs/save.md` 缺尾换行；`StreamThread` 中 `CHUNK_RAW_REAL_CAPS = 64` 相对 cache 的 512 无明显依据。

---

## G. 仍未实现（issue 提及但本次未闭环）

- 原始图层修剪：`wrapper.rs:427` `// TODO Evaluate raw chunks and turn suitable chunks back into associated/transparent`（issue "在自动保存/区块卸载时评估是否有必要保留原始数据"）。
- 并行绘制架构中的"草图来源"：`RawLayer` 未自带草图/桥接；`Standalone` 之外的 scratch 语义未落地。
- `LayerPipeline.support_read_write == false` 的 `draw_chunk_swap` 路径在 `draw_layers` / `submit_layers` 中都没有调用者（旧实现的两条分支现在只剩一条）。
- `layer.rs:144` `// TODO dynamic offset`；`stream.rs:39` `// TODO use Vec<ChunkKey> to minimize thread collision`；`brush.rs:129` `// TODO swap draw`。

---

## 影响速查（按用户可见程度）

| 现象 | 直接原因 |
|---|---|
| 一画就崩溃 | A2（`processor_output` 越界写） |
| 画不出任何东西 | A1（scratch 不存在）、A4（落笔点丢失）、A6（区间未加载即丢弃） |
| 提交后画面不更新、撤销重做无变化 | B8、B10（raw 与 cache 不同步） |
| 晕染颜色跨笔画污染 | A3（`init` 反转，`prepare_stroke` 不执行） |
| 长按擦除行为错乱 | A5（erase/active 笔刷混用） |
| 缩放后模糊 / LOD 错乱 / 生成越界 | C11（mip 层级纹理尺寸错误） |
| 拾色失效 | D12（raw 不加载） |
| 重启后作品丢失、存档不增长 | D13–D16 |
| 打开旧存档内容消失 | E18、E19 |
| 打开旧存档直接崩溃 | E19 + D17（`Err` 链到 `unwrap`） |

---

# 修复复杂度评估

分级标准：

| 级别 | 含义 | 典型工作量 |
|---|---|---|
| **S** | 本地逻辑错误/笔误，风险自洽，改几行 | 数行 ~ 数小时 |
| **W** | 接线缺失：机制已存在（消息、函数、字段），只是没人调用/没接上 | 数小时 ~ 1 天 |
| **M** | 功能不存在于代码库，但设计（issue/文档）已明确，属"照图施工" | 1 ~ 3 天 |
| **L** | 当前设计缺口：类型/数据流/所有权无法承载预期行为，需要小重构定案后再实现 | 3 天以上，且需先做设计决策 |
| **D** | 纯文档/注释/死代码 | 分钟级 |

## 一、本地小修（S / W / D）

| # | 问题 | 级别 | 为什么 |
|---|---|---|---|
| A2 | `processor_output` 越界写 | **S** | `Vec::with_capacity` 或判界一行；逻辑本身正确 |
| A3 | `Stroke::init` 反转 | **S** | 字段初始值 + 分支互换 |
| A4 | 落笔点丢失 | **S** | `prev` 改回 `Option<Draw>` 并在 `None` 时补 `process(target)`；旧实现原样可搬 |
| A5 | `draw_erase` 混用笔刷 | **S~W** | 把 `batch`/`draw_layers` 的笔刷参数换成 `erase`；但 erase 实体存在 `LayerPage` 上、`batch` 需要 `&T`，需调整一次取用方式 |
| C11 | mip 层纹理尺寸 | **S** | 恢复 `create_chunk_texture(chunk_size << z)`；需同时确认 rect/scale 三处自洽 |
| D17 | 损坏区块导致整线程 panic | **S** | 把 `?` 改成 `continue` + 日志 |
| F23 | `unload_overflow` 空转 | **S** | 加"本次是否真卸载过"的判断 |
| F24 | `restock` 的 `debug_assert` 语义错 | **S** | 断言改用真实容量 |
| A7 | `Mark*Unsaved` 没有发送方 | **W** | 消息与集合已存在；在 `submit`/`undo`/`redo`/`draw_active` 后按 dirty 发消息即可 |
| D13 | unsaved 恒空导致永不落盘 | **W** | 与 A7 同源；补上发送方后大部分自动恢复 |
| D16 | 透明/伴生落盘缺守卫 | **W** | `persist_*` 里把 `unsaved` 判断移到 match 各分支内 |
| D15 | autosave 不覆盖 raw | **W** | `save_all` 复制一份 raw 收集循环 + `save_raw_*` 调用 |
| D14 | 卸载/切页丢数据 | **W** | 依赖 A7/D13 修好；`unload*` 里补 raw 分支 |
| E21 | `CHUNK_VERSION`/文档不一致 | **D** | 常量与 `docs/save.md` 对齐 |
| F25 | 死代码（`Layer`、`thread_tx`、`unsaved` 字段） | **D/W** | 删是 D；若把 `Layer` 复活为通用类型则是 M |
| F26 | 注释/文档脱节 | **D** | 纯文档 |

**S 级小计：A2、A3、A4、C11、D17、F23、F24 → 半天到一天可全部完成，立刻消掉"一画就崩"和"mip 错乱"。**

## 二、需要"照图施工"的新实现（M）

功能在当前代码库中**完全不存在**（不是注释掉，是从未被写过），但 issue/文档已给出契约，属实现量而非设计量。

| # | 问题 | 级别 | 需要新增什么 |
|---|---|---|---|
| A1 | scratch 层区块创建与播种 | **M** | 移植 `layer_prepare_rect` + `get_missing_chunks` + `ChunkPool::pop` 到 `PlainLayer`/`ChunkPool`（本提交已把 `pop_chunk_stack` 写好，只是没人调用）；核心是"替换模式先把 raw 内容拷进 scratch 副本"、"非替换模式拷 swapped 副本"两条路径，旧代码可直接参照 |
| A6 | 绘制区间按需请求 + 未就绪则放弃该批 | **M** | `RequestRaw/RequestCache` 处理分支（现在是 `// TODO`）+ 绘制前 `validate`/`request`；issue 的"三区块一致才提交 cpass，否则放弃"需要 `LayerPage` 增加"本批被放弃"的分支 |
| D12 | raw 层的流式加载 | **M** | `LoadQueue.raw` 的填充规则（目前只在 `restock` 里被 `clear()`）、raw 相机队列、raw 取舍与卸载；`ThreadOutput`/`ChunksBase` 侧结构已在，属"另一半没写" |
| D18 | 旧表迁移 | **M~L** | `legacy_classic` 只有两个空 `TableDefinition`，`chunk_migration` 函数体全被注释。要重写"读 `stroke_chunk`(+meta) → 逐块 format 迁移 → 写 `layer_chunk` + `layer_chunk_raw`（伴生）→ 删旧表"，并接进 `Save::touch`/`migrate_format`。**复杂度取决于是否保留 0→3 的历史 format 兼容**（保留则 L） |
| E19 | 上一版裸 zstd 数据兼容 | **M** | 与 D18 同一条迁移链，多一个"无 header"分支；可顺带用 `Scene` 新增标记位驱动一次性迁移 |
| G | 原始图层修剪（transparent/associated 化） | **M** | `cleanup` 的 TODO。需要"全透明检测"（`ChunkSave::is_transparent` 已有）与降级时机（autosave/卸载），并保证与 cache 一致 |
| G | `support_read_write == false` 的 swap 路径 | **M** | `draw_chunk_swap` 管线与函数都还在，但 `draw_layers`/`submit_layers` 只剩 read_write 一条分支；补回旧的两分支即可 |

## 三、当前设计缺口（L，需先定案）

不是"忘了写"，而是**现有类型与数据流无法表达预期行为**，照 issue 直写会撞上语义空白：

1. **raw → cache 的数据流方向没有定义（B8/B9/B10，最核心）**
   - 事实：`RawLayerChunk` 与 `CacheLayerChunk` 的 `Filled` 在代码里**没有任何一处互相派生**；`cache_mipmaps` 只从 cache 下采样到 cache 上层；渲染只读 cache。
   - 空白：raw 改完后谁填 cache level 0？`Associated`（"等于 cache level 0"）在 raw 变更后是否还成立？撤销/重做后 cache 是整体失效重载，还是从 raw blit？
   - 小重构：把"cache 是 raw 的导数 + 草稿的载体"落成 `sync_raw_to_cache(key)` / `demote_to_associated(key)` 一类接口，并定下 `render` 的 fallback（cache 缺失时退回 raw/scratch）。**B 组全部条目怎么写取决于此。**
2. **"合并视图"概念消失（B8 的表现层）**
   - 旧设计用独立的 `merge` 层表达"raw ⊕ 未提交草稿"；新设计让草稿直接写进 cache，于是 cache 既是 raw 的导数、又是草稿宿主，提交时就必须把草稿从 cache 剥离并回写 raw——而这条剥离线在实现中不存在（`submit_layers` 正是一个空骨架）。
   - 需决策：草稿是否单独占一层（回到三层思路），还是接受"cache 被草稿污染、提交时用 raw 覆盖回去"。
3. **区块供给的生命周期无人负责（D12/A1/A6 的上位问题）**
   - 旧 `Layer` 是"数据 + mipmap 层级 + `controlled`"的自洽单元；新区块供给散落在 `prepare_scratch`/`get_missing_chunks`/`request_stream`/`validate_chunks` 四处，本次全部被删且无等价统一入口。
   - 需决策：把"确保 (raw, cache, scratch) 三方在某个矩形上就绪"提炼成一个函数（issue 的"检查相关区块全部加载／否则请求并放弃"正是其规格），否则 A1/A6/D12 会各自实现一遍同步逻辑。

## 四、工作量与顺序建议

- **阶段 0（半天，S 级）**：A2、A3、A4、C11、D17、F23、F24。目标是"不崩、能画出东西、mip 不乱"；此时仍不落盘、不开旧档，但可回归调试。
- **阶段 1（1~2 天，设计定案 + 第三组 1、3）**：定下"区块就绪"入口与 raw/cache 契约，这是所有 M 级工作的前置。
- **阶段 2（3~5 天，M 级）**：A1、A6、D12、D13–D16 + A7。
- **阶段 3（1~3 天，M/L）**：D18/E19/E20（迁移链 + 版本号）、G（原始图层修剪）。
- **阶段 4（分钟级，D）**：F25、F26、E21。

**总体判断**：不是"几处接线漏了"的量级。S 级仅 7 条，集中在笔刷数学与纹理尺寸这类局部；真正让程序不可用的是**三类成体系的缺失**——绘制前的区块供给（A1/A6/D12）、cache↔raw 的同步契约（B8/B9/B10，含 L 级设计决策）、持久化与迁移链（A7/D13–D16/D18/E19）。其中第二类是唯一必须先做设计决策的：现有类型没有任何一处表达 raw 与 cache 的派生关系，硬写会得到两套互相打架的真相来源。

相比之下，`update_goal` 这类产品行为未受影响；退化面集中在绘图内核与存档，可按阶段 0→4 逐步回绿。
