## 数据流收发架构

三个主要概念：

1. 发者 - 指释放数据的人，比如 `BrushFlow` 通过 `SliderValue(f32)` 释放滑条数据，内部实现
2. 收者 - 指接受数据的人，比如 `Slider` 监听 `SetSliderValue(f32)` 来接受数据，内部实现
3. 数据流 - 指中间的传递过程，比如将 `BrushFlow` 的数据接到 `Slider` 上的这个过程，由外部进行连接

重要规则：

- 数据节点可以同时是发者和收者，比如 `Tabs` 接受 `SetButtonSelected`，并写入当前 tab 的 `Val<bool>` / `Val<Rectangle>`
- 布局和可见性数据优先使用 `Val` 直接绑定（见 `docs/val-pattern.md`），不要再在组件之间转发 `SetWidgetRectangle` / `SetWidgetVisible`
- 有一些比较难混杂的事件，比如指针悬浮在按钮上发出的 `ButtonHover` 事件
    - 在这里按钮属于发者，不过按钮自己不存储 `hover` 状态，但仍然在释放数据
    - `ButtonHover` 事件还用于 `Button` 自己内部的显示，收者是对应的渲染组件，但是既然是内部实现我们可以当做不存在
- 并不意味着一定就必须是两个节点，比如 `Transform` 从源 `Val<Rectangle>` 派生出新的 `Val<Rectangle>`，这里就有三个节点，`Transform` 可以归为数据流的一部分。
    - 深究的话 observer 内部也是用节点实现的， 所以实际上肯定不止两个节点
- 收者没有必要一定是 Command Event 驱动的。现有代码仍然有许多遗留的使用 `when_modify` 甚至一些还有需要手动调用的代码，它们大类上也属于收者，只不过没有一个对应的事件将其解耦，内部行为可能会入侵到数据流控制里面。
- 支持收数据 ≠ 必须收数据，`Slider` 和 `Button` 这类结构一般支持接受 `Theme` 数据。但是往往我们只依赖初始化给的数据进行显示也完全够用了。

代码示例：

- `QuadMesh` 是通用的泛型方形渲染器，是演示**数据接收**和**收者**内部如何如何上传 GPU 的良好示例
- `Slider` 是滑条组件，是演示如何完整、正确地与交互组件 `ColliderTool` 和渲染组件 `RRect` 共同组织数据流的良好示例