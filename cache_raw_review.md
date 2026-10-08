# `a2ec5f1 cache+raw layer system` 缺陷清单

范围：仅列本次提交引入（相对 `2b71d34`）的有关缺陷。格式：`旧版本行为 → 本版本行为 → 期望`。

> **进度复核**（针对 `6c0d782` + `ceb24b7`）：已逐条核验原有标注，见每条的 **核验** 段落，以及文末「复核结果 / 新增问题」。
> 结论提要：`(INVALID)` 两处（A2、C11）**判定正确**，A2/C11 均已从清单移除有效性；`(DONE)` 中 **A6、B8 标注有误**，实际只修了一半或完全未修，已改回未完成。

---

## A. 绘制链路（P0，功能不可用）

1. **(DONE)** **绘制目标区块根本不创建** — `layer/wrapper.rs` / `layer/brush.rs`
   - 旧：`DrawPipeline::prepare_scratch` 用 `layer_prepare_rect` + `ChunkPool::pop` 在 scratch 层补齐 `batch.dirty` 覆盖的区块（替换模式还先从 raw/主层拷入副本）。
   - 新：没有任何代码调用 `ChunkPool::pop_chunk_stack`，`PlainLayer.chunks` 永远为空，`DrawsBatch::draw_layers` 中 `scratch.chunks.get(&key)` 永远 `None`。整个绘制循环空转；`pool` 只被 `push` 从没被 `pop`。
   - 期望：绘制前在 scratch 层按 dirty 补齐区块（替换模式拷入源数据），笔刷结果才会落到 scratch 并参与渲染。

2. **(INVALID)** **`DrawsProcessor::processor_output` 立刻 panic（越界写）** — `layer/brush.rs:104-108, 356-388`
   - 旧：`Vec<Self::Draw>` + `out.push(...)`，容量 0 时也能分配。
   - 新：`Stroke::batch` 传 `Vec::new()`（容量 0）给 `&mut Vec<u8>`，`processor_output` 在 `out.len() > limit` 检查后直接 `out.extend_from_slice(bytes_of(&processed))`；此时 `len == 0` 而元素 ≥ 12 字节，必然触发 *capacity overflow* panic。现实绘制路径 100% 命中（预置笔刷最大 Draw 28 字节 > 最小非零容量 8）。
   - 期望：调用方预分配（如 `Vec::with_capacity(DRAWS_ARRAY_CAPACITY)`），或按 `len + size_of::<Draw>() > cap` 判界。
   - **核验：INVALID 判定正确，我原报告有误。** `Vec::extend_from_slice` 内部先 `reserve`，只在 `len + additional > isize::MAX` 时才 panic（capacity overflow），`len == 0` 时不可能触发；容量不足只会重新分配。`ceb24b7` 又补了 `from: Option<Draw>` 的单 dab 分支，逻辑正确。

3. **(DONE)** **笔画 `init` 反转，`start` 永远为 false** — `layer/brush.rs:46-89`
   - 旧：`self.batch.start = self.stroke.is_none()`，笔画的第一批 `start = true`，`prepare_stroke` 在笔迹开始前执行一次。
   - 新：`Stroke::new` 里 `init: false`，而 `batch()` 却在 `else if self.init { self.init = false; start: true }`。新建笔画的第一批 `start = false`，之后也永远为 `false`。
   - 后果：`smudge_prepare_stroke`（清空 `draws_state[0]` 携带色）永不执行，晕染颜色跨笔画残留；这是 `paint_stroke()` 生成后 `draws_state` 携带色被后续笔画复用的直接原因。
   - 期望：`init` 初始为 `true`（或改回 `stroke.is_none()` 语义），使每笔画首批 `start == true`。
   - **核验：已修，语义等价。** `init: bool` 已删除，改为 `Stroke::prev: Option<Draw>` + `DrawsBatch.prev`；`draw_layers`/`draw_standalone` 以 `self.prev.is_none()` 判定首批并调用 `prepare_stroke`，与旧 `stroke.is_none()` 完全对应。

4. **(DONE)** **首个笔迹点丢失 / 轻触无任何反应** — `layer/brush.rs:46-64`
   - 旧：`prev = None` 时直接 `draws.push(brush.process(target))`，落笔点必有一个 dab。
   - 新：`Stroke::new` 把 `prev` 设为落笔点，第一次 `batch(target=落笔点)` 时 `from == to`，`processor_output` 循环 0 次 → 无 dab、`dirty` 为零矩形 → `batch()` 返回 `None`。点到抬笔（不移动）什么都不画；落笔点只有在第二次事件、且路径从起点重新扫过时才补上。
   - 期望：每笔画至少落下落笔点的第一个 dab（`prev` 应为 `Option<Draw>` 或首批强制 `process(target)`）。
   - **核验：已修。** `processor_output` 的 `from: None` 分支直接 `process(to)` 并返回 `(to, dirty, 1)`；`Stroke::confirm_batch` 在 `batch.prev.is_none()` 时用 `Rectangle::new_half(next.position, ZERO)` 重置 dirty、否则 grow，与旧 `prev = None` 行为一致。

5. **(DONE)** **`draw_erase` 混用两支笔刷** — `layer/wrapper.rs:335-365`
   - 旧：`self.erase.draw(&mut self.draw, &self.main, draw)`，擦除实体、管线、替换/桥接标志全部来自 `erase`。
   - 新：`Stroke::new(&self.erase, draw)` 只用来取 `replace/bridge`，而 `stroke.batch(self.active.as_ref(), draw)` 与 `draw_layers(..., self.active.as_ref())` 用的是当前预设笔刷。于是"长按擦除"实际用当前笔刷（如 smudge/blur，`bridge_mode() == true`、`replace_mode() == true`）的 dab 与管线去擦除，而 `Stroke.bridge` 却来自 `erase`（false），桥接块既不准备也不清理。
   - 期望：擦除路径全程使用 `erase`（或让 erase 与 active 的 `replace/bridge` 语义显式统一）。
   - **核验：已修。** `draw_erase` 的 `Stroke::new` / `stroke.batch` / `batch.draw_layers` 三处已统一使用 `&self.erase`。⚠️ 但为此新增了"擦除每批都提交到 raw+cache"的行为，见新增问题 **N2**。

6. **（原标注 DONE 有误，实际未修）`request_stream` 被删除，区间未加载时整批绘制静默丢弃**
   - 旧：每批绘制后按 `stroke.dirty` 对各 mip 层发送 `ThreadInput::RequestReal(key)`（下级已加载才请求上级），并把范围外区块拉进来。
   - 新：`RequestRaw`/`RequestCache` 处理分支是 `// TODO` 空实现，且 `LayerPage` 侧根本不再发送；新区块只能靠相机队列里的 cache 被动加载。
   - 期望：绘制前确保 raw + cache + scratch 三者覆盖 dirty（issue 里"检查相关区块全部加载，否则请求并放弃这一批"）。
   - **核验：标注有误，应保持未完成。** `RequestRaw`/`RequestCache` 只是从 `ThreadInput` 里被删除（连空的处理分支都没了），替代通道并未建立：`brush.rs:95` 明写 `// TODO find a way to request stream thread to load chunks`；`prepare_scratch` 仍以 `None => continue` 静默跳过未加载的 raw 区块；`LayerPage` 侧没有任何向流线程请求区块的路径。**"未就绪则请求并放弃该批"这条契约仍未实现**，只是从"TODO 分支"变成"没有分支"。

7. **(DONE)** **`MarkRawUnsaved` / `MarkCacheUnsaved` 无发送方，脏块不落库** — `stream.rs:205-212`，`wrapper.rs`
   - 旧：`submit()` 结束后对 dirty 覆盖的所有 `(x, y, level)` 发送 `MarkUnsaved`。
   - 新：两个输入变体没有任何调用点，`ChunksBase::raw_unsaved` / `cache_unsaved` 恒为空集。
   - 后果：`persist_raw` / `persist_cache` 首行 `if !unsaved.contains(&key) { return }` 使所有持久化路径失效（见 D 组）。
   - 期望：绘制/撤销后标记改动区块为 unsaved。
   - **核验：已修，且换了实现路径。** 改为"换块即脏"：`RawLayer::swap_chunk` / `CacheLayers::swap_chunk`（`layer.rs:512-543`）发 `SwapRaw` / `SwapCache`，流线程 `base.raw_insert/cache_insert` 后把 key 塞进对应 unsaved 集合（`stream.rs:195-207`）；`persist_*` 里原来的 unsaved 提前返回已删除。比 Mark* 方案更直接。⚠️ 两个 `Mark*` 变体因此变成死代码（编译器警告 *never constructed*），见新增问题 **N6**。

---

## B. cache / raw 一致性（P0，画完不显示）

8. **(DONE，但只修了 submit 一半)** **raw 的修改从不回写 cache，渲染看不到新笔画，undo/redo 无视觉变化** — `wrapper.rs:376-390, 515-557`
   - 旧：被编辑的 `main` 层既是数据源又是渲染源，`submit` 直接改它；`merge` 层提供笔迹预览，渲染按 `merge > scratch > main` 回退。
   - 新：`stroke.submit_layers()` 只把 scratch 合成到 `raw`，之后 `cache_mipmaps(&self.cache, dirty)` 只更新 **cache 自身**的高层 mipmap；cache 的 level 0 没有任何刷新路径，渲染又只读 cache（`render()` 中 `CacheLayerChunk::Filled`）。撤销/重做同样只改 raw。
   - 期望：raw 变更后同步 cache level 0，再由 cache 生成 mipmap，渲染始终与 raw 一致。
   - **核验：draw/submit 路径已修，undo/redo 未修。**
     - 已修：`Stroke::submit_cache` / `submit_raw_cache` 按 (cache, raw) 六种组合把 scratch 写进 cache level 0，并把 raw 统一降级为 `Associated`；随后 `cache_mipmaps` 逐级重算；`wrapper.rs::submit` 已改为调用 `submit_raw_cache`。
     - 未修：`wrapper.rs::undo/redo` 在 `Traveler::undo/redo` 之后调用 `stroke.submit_raw_cache(..., &self.scratch, ...)`，而这条路径上 `self.scratch` **恒为空**（`prepare_scratch` 只在 `draw_active`/`draw_erase` 里被调用），于是 `submit_cache` 的 `for (&chunk_key, scratch_chunk) in &scratch.chunks` 循环体一次都不执行，cache level 0 完全没被更新 —— 撤销/重做只改了 raw 纹理，画面不变，上层 LOD 反而被 `cache_mipmaps` 用**陈旧的** cache level 0 重新覆盖一遍。详见新增问题 **N1**。

9. **`Associated` 不变量失效** — `layer.rs:109-122`，`brush.rs:104-127`，`wrapper.rs:467-479`
   - 旧：`RawLayerChunk::Associated` 表示"数据等于对应 cache level 0 区块"，两者同步演进。
   - 新：绘制会直接改写 raw 区块，cache level 0 不变；`ThreadOutput::RawInsert` 也不更新 cache。于是 `Associated` 区块的实际内容可能与 cache 不一致；反之 `cache_mipmaps` 从 level 0 重新下采样时会把已被 raw 修改的内容覆盖回旧值。
   - 期望：明确谁是权威（raw），并在提交时把 raw 区块内容写回 cache level 0 / 或把状态降级为 `Filled`。
   - **核验：仍未闭环。** `submit_raw_cache` 现在会在提交后用 cache 作为权威内容并把 raw 置为 `Associated`，但该不变量在两条路径上仍会破：
     - (a) `Traveler::undo/redo` 直接改写 raw 的纹理（`Filled` / `Associated` 两种都可能）而完全不动 cache；
     - (b) `persist_raw` / `persist_cache` 各自独立落盘（raw 存状态、cache 存像素），重载后两者可能来自不同时刻，`Associated` 的"等于 cache level 0"无法保证。
     - 另外 `Traveler::stock` 只快照 `RawLayerChunk::Filled` 的区块（`traveler.rs:110`），而提交后 raw 恒为 `Associated`，导致**提交后再撤销时快照为空**，见新增问题 **N5**。

10. **(DONE)** **cache 的 level 0 无来源，mipmap 链无法建立** — `stream.rs:283-346`
    - 旧：主层自身被加载并生成 mipmap（`generate_mipmaps` 在编辑后按 `main.mipmap_levels` 逐级下采样）。
    - 新：`load()` 只把磁盘上的 cache 区块放进来，绘制结果不进 cache，也没有"由 raw 生成 cache level 0"的步骤，`cache_mipmaps` 又只在上下层都已 `Filled` 时才 dispatch。干净启动后 cache 里只有磁盘残留的层级，LOD 链基本不存在。
    - 期望：raw 提交时生成/刷新对应 cache level 0，再逐级生成 mipmap。
    - **核验：已修。** `submit_cache` 在 cache level 0 缺失时用 `create_empty_chunk` 新建并写入，`submit_cache` 末尾调用 `cache_mipmaps` 逐级向上；`CacheLayers::swap_chunk` 把新块同步给流线程供落盘。⚠️ 依赖 N1 的撤销路径仍是断的。

---

## C. 缓存图层纹理尺寸（已判定为设计意图，非缺陷）

11. **(INVALID)** **所有 LOD 层级的 cache 区块都用第 0 级像素尺寸创建**
    - 旧：`chunk_prepare` 用 `create_chunk_texture(UVec2::splat(chunk_size << z))`，第 z 级区块纹理是 `(chunk_size << z)²`，与 `chunk_to_rect` 的世界范围一致。
    - 新：`stream.rs:291-297`（cache 分支）统一用 `self.config.chunk_size` 创建纹理，但 `chunks_stack_key_to_rect(key, ...)` 给出的是 `(chunk_size << z)²` 的世界矩形。cache 区块的 rect 与纹理尺寸彻底失配。
    - 后果：第 z ≥ 1 级区块渲染时，`render.wgsl` 用 `chunk.size`（= `512<<z`）铺满屏幕四边形，却只采样 512×512 的纹理 → 内容被放大 `2^z` 倍（读越界靠 clamp）。同时 `mipmap.wgsl` 的 `dst_coords = position / scale`（scale 恒为 2）会越出 512×512 目标纹理，写出未定义内容。
    - 期望：第 z 级 cache 区块纹理为 `(chunk_size << z)²`（或让 rect/纹理/`scale` 三者自洽）。
    - **核验：INVALID 判定正确，我原报告有误。** 该设计内部自洽，按 LOD（固定 512² 纹理 / 逐级翻倍的世界覆盖）理解后没有问题：
      - `mipmap.wgsl`：`scale = source.size / textureDimensions(source_texture) * 2`，源与目标同为 512² 时 `scale == 2`，`dst_coords = (position - dst.coords) / 2` 最大 511，**不会越出 512² 目标纹理**；`src_coords = dst_coords % 256 * 2` 恰好取 2×2 邻域，是正确的一级下采样。
      - `render.wgsl`：用 `chunk.size`（= `512 << z`）铺满世界矩形、采样整张 512² 纹理，LOD 语义正确。
      - 我原先"dst_coords 越界写"的推论源于误以为 `scale` 会随层级增长，实际它恒为 2。

---

## D. 流式加载与持久化（P0，存档写不进去 / 读不出来）

12. **raw 图层完全不加载、不请求**
    - 旧：`stroke_chunk` 表就是主层，按相机与 `RequestReal` 加载。
    - 新：`LoadQueue::restock` 只填充 `queue.cache`；`queue.raw` 只在 `restock` 里被 `clear()`，没有任何 `insert`；`consume` 的 raw 分支因此永不产出 `ChunkKeyKind::Raw`。唯一的补齐通道 `RequestRaw` 是 TODO。
    - 后果：`LayerPage.raw.chunks` 永远为空 → 拾色（`pick_color` 只认 `RawLayerChunk::Filled`）静默失效、撤销/重做（Traveler 只认 raw）失效、绘制无处落笔、原有作品不再显示。
    - 期望：raw 与 cache 一样由相机队列 + 显式请求加载。
    - **核验：仍未修复，且被新架构放大为"新笔画完全画不出"。**
      - `queue.raw`（`stream.rs:92`）在整份代码里只有 `clear`/`sort`/`get_index`/`contains`，**没有任何 `insert`**；`restock` 只填 `queue.cache`（`stream.rs:589-593`）。因此 `generate_batch` 的 raw 分支（`stream.rs:619-628`）永远取不到 key，`ChunkKeyKind::Raw` 不可达，`load_raw` / `RawInsert` / `RawRemove` 整条链在当前构建中是死代码。
      - raw 块的**唯一来源是主线程 `RawLayer::swap_chunk` → `SwapRaw`**，而 `swap_chunk` 只在 `submit_cache` 内部对"已经存在"的 raw 块调用（`brush.rs:173/233`）。干净启动时 `raw.chunks` 恒空 → `prepare_scratch` 的 `None => continue` 触发 → **每一批绘制都被静默丢弃**。
      - 关联影响不变：`pick_color` 只认 `RawLayerChunk::Filled` 故拾色失效；`Traveler` 只认 raw 故撤销/重做只能作用于本会话内被 `swap` 过的块。

13. **(DONE)** **`ChunksBase::raw_unsaved` / `cache_unsaved` 从不写入 → 区块永远不保存**
    - 旧：`unload`/`unload_all`/`autosave` 都会检查 `base.unsaved` 并回读纹理写库；`write_chunk_data` 还会把"全透明"区块从表中删除。
    - 新：`persist_raw`/`persist_cache` 的 `if !unsaved.contains(&key) { return }` 永远为真 → `RawLayerChunk::Filled` 分支被跳过；`save_all()` 又只遍历 `cache_unsaved`（恒空）。
    - 期望：绘制与加载后标记 unsaved，落盘。
    - **核验：已随第 7 条一并修复**（`SwapRaw`/`SwapCache` 直接写入 unsaved 集合，`persist_*` 的提前返回已删除）。

14. **(DONE)** **绘制结果在溢出卸载 / 切页时被丢弃**
    - 旧：`unload` 对 unsaved 区块回读并写库，随后从内存移除。
    - 新：`unload_overflow`/`unload_all` 只调用 `persist_*`，而它因 unsaved 为空直接返回 → 未保存的 raw 内容随内存释放消失；`set_page` 的 `unload_all` 同样丢数据。
    - 期望：卸载前把 unsaved 内容持久化。
    - **核验：已修。** `unload_raw` / `unload_cache` 现在显式判 `unsaved.contains(&key)` 才 `persist_*` 并移除标记（`stream.rs:436-466`），`unload_all` 复用同一函数。⚠️ `unload_overflow` 的 `continue` 空转仍在（F23）。

15. **(DONE)** **`autosave` 不覆盖 raw 图层**
    - 旧：`autosave` 遍历整个 `base.unsaved`（单一图层）写 `stroke_chunk`。
    - 新：`save_all()` 只收集 `cache_unsaved`，完全没有 raw 任务；`ThreadInput::Autosave` 因此不可能保住原图层。
    - 期望：autosave 同时落盘 raw + cache。
    - **核验：已修。** `save_all` 先遍历 `raw_unsaved`、再遍历 `cache_unsaved`，随后统一 `device.poll(wait_indefinitely)` 再 `commit`，最后清空两个集合。

16. **(DONE)** **透明 / 伴生区块的落盘无 `unsaved` 判定，会删掉未改动数据** — `stream.rs:503-509`
    - 旧：一整套 `unsaved` 守卫。
    - 新：`RawLayerChunk::Associated` → `save_raw_associated`、`Transparent` → `save_raw_transparent`（执行 `table.remove`）都在守卫之后；只要卸载一个未加载数据的 raw 区块，就会把它在库里的记录删掉或覆盖成伴生标记。
    - 期望：透明/伴生状态变更需先标记 unsaved 再落盘。
    - **核验：已修。** `persist_raw` / `persist_cache` 的守卫已全部移除，改成由 `unload_*` 在调用前显式判 unsaved，语义变为"未改动就不写"。副作用见新增问题 **N3**（透明块不再被回收，数据库只增不减）。

17. **加载遇错即崩** — `stream.rs:286-340, 172`
    - 旧：`zstd::decode_all` 失败被 `if let Ok(...)` 丢弃，损坏区块只当作未加载。
    - 新：`load_cache`/`load_raw` 对无法解析的 4 字节 version 或 zstd 解压失败返回 `Err`，`load()` 用 `?` 上传，`run()` 用 `?` 返回，最终 `wrapper.rs:143-147` 的 `.unwrap()` panic 掉整个进程。
    - 期望：单个区块损坏只跳过并记录日志。
    - **核验：仍未修复。** `ChunkLoad::load_raw/load_cache` 的 `Err("()")` / `Err("not supported version")` / zstd `?` 原样保留，经 `StreamThread::load()?` → `run()?` → `wrapper.rs:141-145` 的 `.unwrap()` 直接 panic 整个进程。

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
    - **核验：仍未修复**（`save.rs` 未被两个新提交触及）。

21. **`CHUNK_VERSION` 与文档不一致，raw 的 `associated` 只有 0/1 两种写入方式** — `save/chunk.rs:5, 103-124`；`docs/save.md`
    - 文档写 "version 版本 v1"，代码 `CHUNK_VERSION = 0`；`save_raw` 恒写 `associated = 0`，只有单独的 `save_raw_associated` 能写 1。
    - 期望：文档与实现对齐。
    - **核验：仍未修复**（`save/chunk.rs` 未被两个新提交触及）。

---

## F. 其他退化与隐患

22. **最大 LOD 层级无数据** — `layer.rs` / `stream.rs`
    - `DEFAULT_MIPMAP_CAP = 8`；`restock` 加载 `center.2-1 .. 8`（含第 **7** 级），`cache_mipmaps` 经 `rect_chunks_stack_iter(.., cap-1)` 只生成第 **0..6** 级（即产出到第 7 级）；而 `render()` 在最大缩放时请求 `actual_mipmap = min(zoom, 7) = 7`。
    - **核验：实际自洽，建议降级为观察项。** 第 7 级由 `cache_mipmaps` 从第 6 级生成（不是靠磁盘加载），渲染请求第 7 级落在生成范围内；`render` 的请求上界 `cap-1` 与生成上界一致。原表述"生成/加载层级与请求层级边界不一致"不成立。

23. **`unload_overflow` 在候选都在队列中时空转** — `stream.rs:414-446`
    - `continue` 分支不递减 `frnt`，若 `cache_overflow()` 持续为真会反复扫描同一批条目（不致命，但每帧空转）。
    - 期望：记录有效卸载数，无进展即提前退出。

24. **`restock` 的 `debug_assert!(self.cache.len() < CHUNK_CACHE_REAL_CAPS - 1)` 容易被真实视锥触发** — `stream.rs:608`
    - 该 assert 统计的是"队列长度"而非"真实纹理数"，缩放到 0 级附近时 512 区块的视锥很容易超过 511 个条目而在 debug 构建 panic。
    - 期望：断言应对齐真实容量语义，或改为运行时降级。

25. **重构残留死代码**
    - `layer.rs`（`Layer` 结构体已无构造点）、`ChunkPool.chunk_size`、`save/chunk.rs`（`ChunkLoadResult.unsaved` 恒 false）、`brush.rs`（`submit_standalone` 无调用者）。
    - **核验：已用编译器交叉验证。** `cargo check -p ln_drawer --lib` 明确报出：`field support_read_write never read`、`methods draw_chunk_swap and merge_chunk_over_read_write never used`、`field chunk_size never read`（`ChunkPool`）、`method pop_chunk_stack never used`、`function lower_chunk_of never used`、`method submit_standalone never used`、`variants MarkRawUnsaved and MarkCacheUnsaved never constructed`、`field unsaved never read`、`method is_transparent never used`、`type alias ChunkKey / CHUNK_META0_FORMAT / TABLE_LAYER_CHUNK / TABLE_LAYER_CHUNK_META never used`（共 15 条警告）。其中 `pop_chunk_stack` 与 `lower_chunk_of` 是**功能性**死代码，见 N4。
    - 期望：清理或补齐。

26. **文档/注释与实现脱节**
    - `widgets/brush_preview.rs:37-41` 仍称使用 `DrawPipeline::draw_standalone`；`wrapper.rs:55-62` 的 `LayerPage` 文档仍描述 "main / merge / mipmap 三层、mipmap 尚未实现"；`save.rs:34-37` 的表结构与实际表不符；`docs/save.md` 缺尾换行。
    - **核验：均未修复**（三个新提交未触及这些注释）。

---

## G. 仍未实现（issue 提及但本次未闭环）

- 原始图层修剪：`wrapper.rs:427` `// TODO Evaluate raw chunks and turn suitable chunks back into associated/transparent`（issue "在自动保存/区块卸载时评估是否有必要保留原始数据"）。
- 并行绘制架构中的"草图来源"：`RawLayer` 未自带草图/桥接；`Standalone` 之外的 scratch 语义未落地。
- `LayerPipeline.support_read_write == false` 的 `draw_chunk_swap` 路径在 `draw_layers` / `submit_layers` 中都没有调用者（旧实现的两条分支现在只剩一条）。
- `layer.rs:144` `// TODO dynamic offset`；`stream.rs:39` `// TODO use Vec<ChunkKey> to minimize thread collision`；`brush.rs:129` `// TODO swap draw`。

---

## 影响速查（按用户可见程度）

> 下表的"直接原因"已按两个修复提交后的实际代码更新。

| 现象 | 直接原因 |
|---|---|
| ~~一画就崩溃~~ | ~~A2~~ → **已判定 INVALID，不存在此崩溃** |
| 画不出任何东西（重启后尤其明显） | **D12**（raw 块从不从磁盘加载；`prepare_scratch` 对缺失 raw 走 `continue`）、A6（缺请求通道） |
| 提交后画面不更新 | 已修（`submit_cache` 写 cache level 0） |
| **撤销/重做画面不变，LOD 反被旧内容覆盖** | **B8 未修部分 + N1**（undo 只改 raw，`submit_raw_cache` 拿到空 scratch） |
| 撤销/重做丢数据（快照为空） | **N5**（`Traveler::stock` 只认 `Filled`，而提交后 raw 恒为 `Associated`） |
| 晕染颜色跨笔画污染 | 已修（A3） |
| 长按擦除行为错乱 | 已修（A5） |
| 擦除时卡顿/存档暴涨 | **N2/N7**（擦除每批都读回并落盘 + 重建 LOD） |
| 缩放后 LOD 显示错乱 | 已判定 INVALID（C11 为设计意图） |
| 拾色失效 | D12（raw 不加载） |
| 重启后作品丢失、存档不增长 | 已修（A7/D13/D14/D15/D16） |
| 数据库只增不减 | **N3**（透明块与 `Transparent` 状态不再回收） |
| 长时间绘制内存持续上涨 | **N4**（`ChunkPool` 只进不出） |
| 打开旧存档内容消失 | E18、E19 |
| 打开旧存档直接崩溃 | E19 + D17（`Err` 链到 `unwrap`） |

---

# 复核结果（对 `6c0d782` + `ceb24b7`）

## 结论：原标注中 2 条需改正

| 条目 | 原标注 | 复核结论 |
|---|---|---|
| A2 `processor_output` 越界写 | INVALID | ✅ **判定正确**。`extend_from_slice` 会先 `reserve`，`len == 0` 不可能触发 capacity overflow；是我原报告的分析错误。 |
| C11 LOD 纹理尺寸 | INVALID | ✅ **判定正确**。`scale = source.size / textureDimensions * 2` 恒为 2，`dst_coords` 最大 511，不越界；按"固定 512² 纹理 + 逐级翻倍覆盖范围"的 LOD 设计自洽。 |
| A6 请求 + 未就绪弃批 | DONE | ❌ **应改回未完成**。只是删掉了 `RequestRaw/RequestCache`，未建立替代通道；`brush.rs:95` 仍留 `// TODO find a way to request stream thread to load chunks`。 |
| B8 raw→cache 回写 | DONE | ⚠️ **只修了一半**。`submit` 路径已修；`undo`/`redo` 走 `submit_raw_cache` 时 `scratch` 恒空，cache 完全没更新。 |
| F22 最大 LOD 层级 | 未标注 | ⚠️ 复核后认为**实际自洽**，建议降级为观察项。 |

## 真正确认修好的

`A1`（`prepare_scratch` 落地）、`A3`、`A4`、`A5`、`A7`+`D13`（改为 `SwapRaw`/`SwapCache` 换块即脏）、`D14`、`D15`、`D16`、`B10`（cache level 0 有了来源）。

其中 `SwapRaw`/`SwapCache` 这个改法值得肯定：把"谁拥有 GPU 内容"讲清楚了 —— 主线程换块时把同一个 `Chunk`（含纹理与各 bind group）交给流线程，避免了两边各自持有一份内容而不知道对方状态的问题，比原来的 `Mark*Unsaved` 方案更直接。

## 仍未修复

`A6`、`B8`(undo/redo 部分)、`B9`、`D12`、`D17`、`E18`、`E19`、`E20`、`E21`、`F23`、`F24`、`F25`、`F26`。

## 两个新提交同时引入的新问题

- **最大的一条**：raw 图层仍然没有磁盘加载路径（D12），而 `prepare_scratch` 对缺失 raw 的处理是 `continue`，导致**重启后无论如何都画不上**（旧提交是整批空转，现在是"有 raw 才能画、而 raw 永远不会被加载"）。
- `N2`：`draw_erase` 变成"每批提交并落盘"，语义与性能都变差。
- `N4`：`ChunkPool` 变成只进不出。
- `N5`：`Traveler::stock` 与"提交后 raw 恒为 `Associated`"冲突，撤销快照为空。
- `N6`/`N3`：若干死代码与"透明块不再回收"。

---

# 新增问题（本轮引入或首次暴露）

## N1. `undo`/`redo` 只改 raw、不动 cache，且空 scratch 让 `submit_raw_cache` 形同虚设

`wrapper.rs:552-606`：

```rust
let stroke = self.traveler.undo(&mut encoder, &self.raw).unwrap();
...
stroke.submit_raw_cache(&self.lp, &mut cpass, &self.scratch, &mut self.raw, &mut self.cache);
```

- `Traveler::undo` 通过 `copy_into_origin` 直接改写 **raw** 纹理（`traveler.rs:183`），**不碰 cache**。
- 紧接着的 `submit_raw_cache` 以 `self.scratch` 为源，但 `self.scratch` 只由 `draw_active` / `draw_erase` 填充，撤销路径上**恒为空** → `submit_cache` 的 `for (&chunk_key, scratch_chunk) in &scratch.chunks` 零次迭代 → cache level 0 不更新。
- 更糟的是 `submit_cache` 末尾无条件调用 `cache_mipmaps(cpass, cache, self.dirty)`，于是 **LOD 上层被"陈旧的 cache level 0"重新覆盖一遍**。
- 期望：撤销后把 raw 变更同步进 cache level 0（或让 cache level 0 直接以 raw 为源重绘），再重建 LOD。
- 严重度：**P0**。撤销/重做是核心功能，当前表现为"按下无反应 + 缩小时的 LOD 被写回旧内容"。

## N2. `draw_erase` 每批都提交到 raw + cache

`wrapper.rs:379-395`：`draw_erase` 在每批绘制后都 `stroke.submit_raw_cache(...)`，而 `draw_active` 只在抬笔时 `submit()`。

- 后果：(a) 擦除的每个绘制批都触发 `save_raw` / `save_cache` 的纹理回读与 zstd 压缩写库，长按擦除（0.4s 计时器 + 持续移动）会显著卡顿并放大存档；(b) `submit_raw_cache` 会把 raw 降级为 `Associated`，而 `Traveler::stock` 只快照 `Filled`（见 N5），撤销基线被提前破坏。
- 期望：擦除期间的草稿只落在 cache/scratch（渲染可见），raw 与落盘统一延后到 `submit()`。

## N3. 透明块与 `Transparent` 状态不再被回收，数据库只增不减

`persist_raw` / `persist_cache` 现在无条件写入，而 `ChunkSave::is_transparent`（`chunk.rs:153`，编译器报 *never used*）与旧的"全透明则从表中删除"逻辑都没接上。

- 后果：擦掉的内容会在库里留下全零纹理；`RawLayerChunk::Transparent` / `CacheLayerChunk::Transparent` 也会被写成 `associated` / 空行，长期使用后数据库体积只增不减，且与 issue 中"评估是否有必要保留原始数据"的意图相反。
- 期望：落盘前用 `is_transparent` 判定，透明则删行/降级。

## N4. `ChunkPool` 只进不出，草稿区块不再复用（内存无界增长）

`cargo check` 报 `method pop_chunk_stack is never used`。`cleanup()` 每次把 draft 区块 `push` 进 `self.pool.list`，但**没有任何地方 pop**；`prepare_scratch` 改用 `lp.create_empty_chunk(...)` 直接新建 512² 纹理。

- 后果：每个绘制批都新建纹理，池子持续增长，纹理所占显存随绘制量无界累积。旧实现是 `layer_prepare_rect` + `ChunkPool::pop` 复用。
- 期望：`prepare_scratch` 改为从池中取块（`pop_chunk_stack`），或干脆去掉池子只保留"清理后回收"的一侧。

## N5. `Traveler::stock` 只快照 `Filled` 区块，与"提交后 raw 恒为 `Associated`"冲突

`traveler.rs:110`：

```rust
let Some(RawLayerChunk::Filled(chunk)) = raw.chunks.get(&key) else {
    log::error!("failed to find chunk");
    continue;
};
```

而 `submit_raw_cache` 在提交后会把本次涉及的 raw 区块**全部**置为 `Associated`（`brush.rs:276-278`）。

- 后果：提交后再次落笔时，`stock` 找不到 `Filled` 区块 → 快照 sections 为空 → 撤销时 `undo` 找不到可恢复数据（`traveler.rs:155` 走 `log::warn!` + `continue`），表现为"撤销丢数据/无反应"。
- 期望：raw 为 `Associated` 时也应快照对应 cache level 0 的内容，或让 raw 在提交后保持 `Filled` 而不是无条件降级。

## N6. 死代码与语义残留（编译器可验证）

`cargo check -p ln_drawer --lib` 的 15 条警告中，与本次重构直接相关的：

- `variants MarkRawUnsaved and MarkCacheUnsaved are never constructed` —— `Swap*` 方案上线后这两个变体已无用途。
- `method pop_chunk_stack is never used`、`function lower_chunk_of is never used` —— 前者是 N4，后者说明多级 `cache_mipmaps` 的"下级"枚举已不再需要。
- `methods draw_chunk_swap and merge_chunk_over_read_write are never used`、`field support_read_write is never read` —— `support_read_write == false` 的整条分支（旧实现两条分支之一）已成为死路，非 `STORAGE_READ_WRITE` 设备上绘制会静默失效。
- `field unsaved is never read`（`ChunkLoadResult`）、`method is_transparent is never used`、`type alias ChunkKey / CHUNK_META0_FORMAT / TABLE_LAYER_CHUNK / TABLE_LAYER_CHUNK_META are never used` —— 分别是兼容字段与迁移占位。

## N7. 擦除与提交路径的 LOD 重建粒度

`submit_cache` 末尾无条件 `cache_mipmaps(cpass, cache, self.dirty)`，而 `self.dirty` 是**整笔画**的并集。擦除每批都调用（N2），于是每批都在重建整个 dirty 矩形覆盖的所有 LOD 层级。

- 期望：`cache_mipmaps` 用**本批** dirty（`batch.dirty`）而非 `stroke.dirty`，或把 LOD 重建推迟到 `submit()`。

## N8. 次要

- `wrapper.rs:380` 第二个 compute pass 的 label 写成 `"draw_erase_cache"`，与 `"draw_active"` 风格不一致（`brush_preview.rs` 也有 `"paint_seed"` 与 `"draw_active"` 混用），仅影响调试可读性。
- `LayerPage::process_stream` 每个绘制帧都要 drain 一遍通道，`Swap*` 又新增了双向的同一 `Chunk` 所有权传递；当前无锁竞争问题，但 `Chunk` 同时存在于 `LayerPage` 与 `ChunksBase` 两侧，后续若引入区块内容失效标记需要成对处理。

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

| # | 问题 | 级别 | 状态 | 为什么 |
|---|---|---|---|---|
| A2 | `processor_output` 越界写 | **S** | **INVALID** | 前提不成立（`extend_from_slice` 会先 reserve），无此崩溃 |
| A3 | `Stroke::init` 反转 | **S** | **已修** | 改为 `prev: Option<Draw>`，语义等价 |
| A4 | 落笔点丢失 | **S** | **已修** | `from: None` 分支直接出单 dab |
| A5 | `draw_erase` 混用笔刷 | **S~W** | **已修** | 三处统一为 `&self.erase`（代价见 N2） |
| C11 | LOD 层纹理尺寸 | **S** | **INVALID** | 固定 512² + 翻倍覆盖是自洽设计，`scale` 恒为 2 |
| D17 | 损坏区块导致整线程 panic | **S** | 待修 | 把 `?` 改成 `continue` + 日志 |
| F23 | `unload_overflow` 空转 | **S** | 待修 | 加"本次是否真卸载过"的判断 |
| F24 | `restock` 的 `debug_assert` 语义错 | **S** | 待修 | 断言改用真实容量 |
| A7 | `Mark*Unsaved` 没有发送方 | **W** | **已修（换方案）** | 改为 `SwapRaw`/`SwapCache` 换块即脏 |
| D13 | unsaved 恒空导致永不落盘 | **W** | **已修** | 随 A7 |
| D16 | 透明/伴生落盘缺守卫 | **W** | **已修** | 守卫移到 `unload_*`（副作用见 N3） |
| D15 | autosave 不覆盖 raw | **W** | **已修** | `save_all` 补 raw 循环 |
| D14 | 卸载/切页丢数据 | **W** | **已修** | `unload_*` 判 unsaved 后落盘 |
| E21 | `CHUNK_VERSION`/文档不一致 | **D** | 待修 | 常量与 `docs/save.md` 对齐 |
| F25 | 死代码 | **D/W** | 待修（清单见 N6） | `cargo check` 已给出 15 条警告 |
| F26 | 注释/文档脱节 | **D** | 待修 | 纯文档 |

**S/W 级进度：原 8 条 S 级中，2 条判定 INVALID、2 条已修；W 级 5 条全部完成。**

## 二、需要"照图施工"的新实现（M）

功能在当前代码库中**完全不存在**（不是注释掉，是从未被写过），但 issue/文档已给出契约，属实现量而非设计量。

| # | 问题 | 级别 | 状态 | 需要新增什么 |
|---|---|---|---|---|
| A1 | scratch 层区块创建与播种 | **M** | **已完成** | `DrawsBatch::prepare_scratch` 已落地（replace 播种 / 非 replace 新建）。⚠️ 未走 `pop_chunk_stack`（见 N4） |
| A6 | 绘制区间按需请求 + 未就绪则放弃该批 | **M** | 待做 | 需要 `LayerPage` 侧"请求 + 本批放弃"通道；`prepare_scratch` 目前返回 `()`，且 `brush.rs:95` 留着 `// TODO find a way to request stream thread to load chunks` |
| D12 | raw 层的流式加载 | **M** | 待做（**当前最大阻塞**） | `LoadQueue.raw` 的填充规则、`ChunkKeyKind::Raw` 的可达路径、raw 取舍与卸载。raw 现在只能靠 `SwapRaw` 产生，形成闭环 |
| D18 | 旧表迁移 | **M~L** | 待做 | `legacy_classic` 仍只有两个未使用常量。要重写"读 `stroke_chunk`(+meta) → format 迁移 → 写 `layer_chunk` + `layer_chunk_raw`（伴生）→ 删旧表"。是否保留 0→3 历史 format 兼容决定 M 还是 L |
| E19 | 上一版裸 zstd 数据兼容 | **M** | 待做 | 与 D18 同一条迁移链，多一个"无 header"分支 |
| G | 原始图层修剪（transparent/associated 化） | **M** | 待做 | `cleanup` 的 TODO；`ChunkSave::is_transparent` 已备好但从未使用（N3） |
| G | `support_read_write == false` 的 swap 路径 | **M** | 待做 | 编译器已确认 `draw_chunk_swap` / `merge_chunk_over_read_write` / `support_read_write` 全部未被使用 —— 非 `STORAGE_READ_WRITE` 设备上绘制会静默失效 |

## 三、当前设计缺口（L，需先定案）

不是"忘了写"，而是**现有类型与数据流无法表达预期行为**，照 issue 直写会撞上语义空白：

1. **raw → cache 的数据流方向没有定义（B8/B9/B10，最核心）** —— *（部分已定案）*
   - 复核后：`submit_cache` + `SwapRaw`/`SwapCache` 已经给出了"提交时 scratch → cache level 0、raw 降级为 `Associated`"的答案，方向问题**基本定案**。
   - 仍然空白的部分：raw 被**非提交路径**改写时（`Traveler::undo/redo`）谁负责回灌 cache；以及 `Associated` 在持久化往返后是否仍然成立。剩余的是"边界情况"而非"方向"。
2. **"合并视图"概念消失（B8 的表现层）** —— *（已定案）*
   - `submit_cache` 的六种 (cache, raw) 组合 + `merge_chunk_over_cross` 事实上回答了"cache 被草稿污染、提交时以 raw 为准覆盖回去"这一选项。
   - 遗留：撤销路径需要一个不含 scratch 的入口（见 N1）。
3. **区块供给的生命周期无人负责（D12/A1/A6 的上位问题）** —— *（A1 已解决，其余仍在）*
   - `prepare_scratch` 已落地，但"从哪来块、怎么请求、何时放弃整批"依然没有统一入口：`queue.raw` 无填充、`RequestRaw` 被删、`pop_chunk_stack` 无人调用。**这条仍是本清单里最大的结构性缺口。**

## 四、工作量与顺序建议（已按两个修复提交更新）

**已完成**：原阶段 0 的 S 级项中，`A3`、`A4` 已修，`A2`、`C11` 判定为 INVALID 无需修；`A7`/`D13`、`D14`、`D15`、`D16`、`A1`、`A5`、`B10` 已完成。

**剩余工作，按"挡住可玩"的顺序：**

1. **D12 —— raw 层的磁盘加载（唯一的 P0 阻塞项，M）**
   现在 raw 只能靠 `SwapRaw` 产生，而 `SwapRaw` 只对已存在的 raw 块调用，形成闭环。必须补上：`LoadQueue.raw` 的填充规则（按相机 + level 0）、`ChunkKeyKind::Raw` 的可达路径、raw 的取舍与卸载。**在它修好之前，重启后完全无法绘制**，其余一切都无从验证。
2. **N1 —— undo/redo 同步 cache（S~W，但依赖一个语义决定）**
   两条可选路线：(a) 让撤销路径也走"从 raw 重绘 cache level 0"，用一个不含 scratch 的专用函数（如 `sync_cache_from_raw(dirty)`）；(b) 让 `Traveler` 直接以 cache level 0 为快照源。路线 (a) 改动最小，但要求 raw 语义明确（见 B9）。
3. **N5 —— 撤销快照只认 `Filled`（S，取决于 B9 的取舍）**
   只要决定"提交后 raw 是否保持 `Filled`"，这条就是一行判断；若坚持降级为 `Associated`，则需让 `stock` 在 `Associated` 时改从 cache level 0 取内容。
4. **A6 —— 请求 + 未就绪弃批（M，依赖第 1 条）**
   与 D12 合并做最省事：raw 加载队列建好后，"请求"就是往队列里插 key，"未就绪"就是 `prepare_scratch` 返回 `false` 让本批放弃（现在返回的是 `()`，需要改成布尔并在 `LayerPage` 侧处理）。
5. **N2 + N7 —— 擦除/提交的落盘与 LOD 粒度（S~W）**
   把 `draw_erase` 的 `submit_raw_cache` 撤掉（回到与 `draw_active` 对称），`cache_mipmaps` 改用 `batch.dirty`。
6. **N4 —— `ChunkPool` 复用（S）**
   让 `prepare_scratch` 走 `pop_chunk_stack`，或删掉 `pool`。
7. **N3 —— 透明块回收（S）**
   接上 `is_transparent`：透明则 `save_raw_transparent` / `save_cache_transparent`。
8. **D17 —— 损坏区块不要崩进程（S）**、**F23/F24 —— 卸载与断言（S）**。
9. **E18/E19/E20/E21 —— 迁移链与版本号（M~L）**、**G —— 原始图层修剪（M）**。
10. **N6/F25/F26 —— 死代码与文档（D）**，可随时穿插。

**总体判断（更新后）**：修复提交把"笔刷本身"这条线基本理顺了（落笔、首批、擦除笔刷、换块即脏），复杂度评估里的 S 级项基本清空。**剩下的问题集中在"区块供给"与"持久化/迁移"两大块**，而这两块恰好是 issue 里描述最详细、实现投入最大的部分：

- 供给侧：D12（raw 不加载）+ A6（不请求、不弃批）+ N4（不复用）——**这三条是同一个"区块生命周期"问题的三个侧面**，建议一次性设计（评估里的 L 组第 3 条）。
- 一致性侧：N1 + N5 + B9 ——**都指向同一个未定案的语义："提交后 raw 的 `Associated` 到底代表什么、谁是权威"**。建议先把这个决定写进 `docs/save.md` 或 issue，再动代码；否则每修一处都会在别处漏出一个新的不一致。
- 迁移侧：E18/E19/E20 —— 与绘图内核解耦，可以并行推进。

**一个需要确认的设计问题**：`draw_erase` 为什么要每批都落到 raw？如果是为了让"擦除"即时反映到磁盘（避免崩溃丢失），那与 `draw_active` 的不对称需要注释说明；如果不是，建议按 N2 改回只在 `submit()` 落盘。
