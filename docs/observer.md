[Github Issue](https://github.com/Lineoo/ln_drawer/issues/86)

## Data flow architecture (observer pattern)

All inter-element communication follows a three-part data-flow model (the whole project's observer pattern is built on it). ROADMAP's observer/trigger guidance is outdated — follow this and the code.

1. **发者 Sender** — the element that *releases* data. e.g. `Slider` emits `SliderValue(f32)` while dragged; `color_picker.rs` observes it and writes into `LayerWrapper`'s brush settings. Emission is internal to the sender's implementation.
2. **收者 Receiver** — the element that *accepts* data. e.g. `Slider` listens for `SetSliderValue(f32)` to apply an external value. Acceptance is internal to the receiver's implementation.
3. **数据流 Data flow** — the *transfer* between them, wired externally (by whoever composes the two). e.g. `color_picker.rs` observes a `SliderValue` event and `world.queue_trigger`s a `SetSliderValue` at the target slider.

Naming: a paired event type is a *command* with a `Set` prefix (observed by the receiver, e.g. `SetSliderValue`) and a *notification* with the bare name (emitted by the sender, e.g. `SliderValue`). `Echo` (`widgets/echo.rs`) exists to re-emit a `Set*` command as its matching notification on the same node.

Rules:

- A node can be both sender and receiver: e.g. `Tabs` accepts `SetWidgetRectangle` and re-emits the sub-panel `WidgetRectangle`.
- The sender does not need to own state for the data it releases. e.g. `Button` emits `WidgetHover` as a sender; the hover state it keeps is only for its own internal rendering, which also consumes `WidgetHover` as a receiver — that self-loop is an implementation detail and is ignored when wiring.
- A flow need not have exactly two nodes. e.g. `Transform` names an explicit `source`/`target` pair and is itself part of the data flow (it transforms `WidgetRectangle` from the source into `SetWidgetRectangle` at the target) — three logical nodes. Internally observers are also nodes, so real flows always have more than two.
- A receiver is not necessarily command-event driven. Legacy code uses `when_modify` (e.g. `ToolCollider`, `RoundedRect`) and some paths require manual calls; conceptually they are still receivers, just without a decoupling event, and their internal behavior can leak into the data-flow control. Prefer command events for new code.
- Accepting data is not mandatory: `Slider`/`Button` support receiving `Theme` data, but usually display is driven only by init-time values, which is fine.

Example:

- `QuadMesh` (`widgets/renderer/quad.rs`) is a generic, generalized square renderer and serves as a good example of demonstrating how **data reception** and the internals of the **receiver** upload data to the GPU.
- `Slider` (`widgets/slider.rs`) is a slider component and serves as a good example of demonstrating how to fully and correctly organize the data flow together with the interactive component `ColliderTool` and the rendering component `RRect`.

> NOTE: 以下是中文原文

## 数据流收发架构

三个主要概念：

1. 发者 - 指释放数据的人，比如 `BrushFlow` 通过 `SliderValue(f32)` 释放滑条数据，内部实现
2. 收者 - 指接受数据的人，比如 `Slider` 监听 `SetSliderValue(f32)` 来接受数据，内部实现
3. 数据流 - 指中间的传递过程，比如将 `BrushFlow` 的数据接到 `Slider` 上的这个过程，由外部进行连接

重要规则：

- 数据节点可以同时是发者和收者，比如 `Tabs` 同时接受 `SetWidgetRectangle` 并发送子面板的 `WidgetRectangle`
- 有一些比较难混杂的事件，比如指针悬浮在按钮上发出的 `ButtonHover` 事件
    - 在这里按钮属于发者，不过按钮自己不存储 `hover` 状态，但仍然在释放数据
    - `ButtonHover` 事件还用于 `Button` 自己内部的显示，收者是对应的渲染组件，但是既然是内部实现我们可以当做不存在
- 并不意味着一定就必须是两个节点，比如 `Transform` 直接指定发者和收者，这里就有三个节点，`Transform` 可以归为数据流的一部分。
    - 深究的话 observer 内部也是用节点实现的， 所以实际上肯定不止两个节点
- 收者没有必要一定是 Command Event 驱动的。现有代码仍然有许多遗留的使用 `when_modify` 甚至一些还有需要手动调用的代码，它们大类上也属于收者，只不过没有一个对应的事件将其解耦，内部行为可能会入侵到数据流控制里面。
- 支持收数据 ≠ 必须收数据，`Slider` 和 `Button` 这类结构一般支持接受 `Theme` 数据。但是往往我们只依赖初始化给的数据进行显示也完全够用了。

代码示例：

- `QuadMesh` 是通用的泛型方形渲染器，是演示**数据接收**和**收者**内部如何如何上传 GPU 的良好示例
- `Slider` 是滑条组件，是演示如何完整、正确地与交互组件 `ColliderTool` 和渲染组件 `RRect` 共同组织数据流的良好示例