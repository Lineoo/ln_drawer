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
