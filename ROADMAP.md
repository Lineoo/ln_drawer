> NOTE: This page is written in Chinese.

# 待办事项 #
- [x] 右键菜单
    - [x] 菜单项目
    - [x] 菜单显示
        - [x] SDF 圆角矩形
        - [x] 浮点渲染
- [x] 交互操作改进
    - [x] 操作层级统一
        - [x] 使用 `when_insert` 和 `depend` 来注册操作元素
            - [x] 删除 `Service` 和 `Property` 并保持 `world` 模块整洁
            - [x] 删除各种 Entry, Other 写法
            - [x] (trait) ElementDescriptor 用于直接在世界新建 Element，自动获取 interface 等等的资源
        - [x] 无限大操作区域
    - [x] Alt 取色
        - [x] 更新调色盘
        - [x] 选定色彩和调色盘分离
    - [x] 右键元素并删除
- [x] 更新 ROADMAP.md
- **LnDrawer v0.1.1-alpha2**
- [ ] `Dependency` 仿照 Observer 将其元素化
- [ ] 元素编组
- [ ] 元素编组可单例与遍历
- [ ] 大家全部变成编组吧 - v- / 世界默认编组
- [ ] 圆角描边与阴影
- [ ] 变换工具
    - [ ] 变换接口元素
    - [ ] 位置移动
    - [ ] 矩形调整
- [ ] 简单噪音播放器
    - [ ] 开关按键
    - [ ] 音频库
    - [ ] 噪声生成
- [ ] 序列化保存 & 加载
- **LnDrawer v0.1.1-alpha3**
- [ ] 音乐播放器
    - [ ] 用户界面
- [ ] 吸色实时显示颜色
- [ ] 可修改的圆角大小
- [ ] 曲率连续圆角
- [ ] 重写 Interface
    - [ ] 使用世界元素来注册 Interface
- [ ] 更多画板工具
    - [ ] 按钮列
        - [ ] 按钮图片显示
        - [ ] 单选组
    - [ ] 画板工具状态机
        - [ ] 画笔按钮
    - [ ] 全 Alpha 区块垃圾清理
- [ ] 偏移与精度修复
- [ ] Observer & Dependency 清理
    - [ ] Element 的 `when_remove` 动作
- [ ] Any 观察者
- [ ] 使用 `Attaches` 简化元素对应
- [ ] 分数单位支持
    - [ ] 修复"无限大矩形"精度问题
    - [ ] 相机位移
    - [ ] 缩放
    - [ ] 文字的高精度渲染
- [ ] Palette & TextEdit 纹理更新时占用太大

# 技术细节 #

## 为什么 interface 成为了一个 Element 而 lnwin 没有成为 Element？

这个主要是由于——事件循环。虽然窗口和事件循环并不是一一对应的，但是就对于 ln_drawer 而言，目前窗口和事件循环还是绑定在一块儿的。

但是 interface 渲染不一样，它不负责事件循环，但他需要和新生成的元素交互（创建渲染组件），这部分而言其实和我们未来要做的相交检测等功能是并列的，即元素之间的互动、更新，将 interface 也作为一个 Element 使用可以更方便我们使用。

比如说，现在 interface 保存在 lnwin 里面，我们只能在 lnwin 里面直接创建 (`self.interface.create_*`) 但是对于以后的功能，我们可能希望从 World 中的一个普通元素进行生成（想象一下按下一个按钮生成一个组件），这就要求我们能够从 World 里访问到目前的 interface 实例。且不说用 singleton 写这个有多方便，如果我们不把 interface 作为 element 实现，那我们就只能使用一个命令队列 element 把操作发送给 lnwin ，到头来还是要加一个 Element，代码逻辑也不减反增，这就完全没必要了。

而且虽然现在窗口仍然保留为一个直隶于事件循环的成员，到时候或许我们也会把它作为一个元素。刚好其实我们实际上也区分了 Lnwin（外层控制） 和 Lnwindow（真窗口），改起来就更方便了不是嘛。所以可能以后还是会把 Lnwindow 变成 Element ，留下那个 Lnwin 负责事件循环。

## 循环数位和相对的尺度

我们实现了无限画布——好吧其实不是无限的，毕竟受到 32 位整数限制。但是我们希望即使真的哪个神经病到达了地图边缘我们仍然能够愉快的处理这种现象，我的答案是——循环。

因为循环其实从计算机的角度来看相当合理嘛，是一种很自然的处理方式。不过目前的 rect 在处理溢出回绕时会导致负尺寸并且无法正常显示（实际上会导致三角形完全炸掉），所以为了处理循环顺便防止负尺寸，rect 我们使用左下角点的位置 + 相对的宽度高度尺寸来处理。

## Commit 格式

用中文写，模块开头，优化、修复等用点号分隔，最后标题。

比如：
- `image.feat: 图像模块`
- `label.remove`
- `button: 小修改`
- `world.fix: 某个 BUG`
- `mixed.clean: 清理代码`
- `proj.ROADMAP`
- `ver: v0.1.0-alpha4`

## Observer & Trigger 系统综述

我们推荐 observer 的正统用法。这意味着以下写法是不推荐的：
```rust
struct ElementUpdate(ElementHandle);
listener.observe::<ElementUpdate>(/* .. is that thing updated? */);
world.trigger(ElementUpdate(handle));
```

而以下是推荐的:
```rust
struct ElementUpdate;
let obv = that.observe::<ElementUpdate>(/* .. I need to send it back to the listener .. */)
(world.entry(obv).unwrap()).depend(listener);
that.trigger(ElementUpdate);
```

## Element, non-Element 和 Descriptor 模式

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

同样不推荐使用 `Option<T>` 的写法：
```rust
impl Baz {
    pub fn new(property: Property) -> Bar {
        /* .. */
    }
}
impl Element for Baz {
    fn when_inserted(&mut self, handle: ElementHandle, world: &WorldCell) {
        // 过度包揽了自己不该干的活儿
        let interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_painter(/* .. */);
        // self.inner 为 None 的状态只在初始化时存在，很别扭
        self.inner = Some(inner);
    }
}
```

如果有对应的描述器，也推荐如下写法：
```rust
struct BazDescriptor {
    property: Property
}
impl ElementDescriptor for BazDescriptor {
    type Target = Baz;
    fn prepare(self, world: &WorldCell) -> Self::Target {
        // 描述器专门用于从世界中提取数据进行构建
        let interface = world.single_mut::<Interface>().unwrap();
        let inner = interface.create_painter(/* .. */);
        // 没有非法状态
        Baz { inner }
    }
}
```

## Element 重构

既然 interface 也是 Element，是不是应该把 interface 也放进 elements 里面？

啊当然不是。其实如果 interface 也放在里面的话，我甚至觉得整个程序放在 elements 里也没问题（

所以我们会慢慢地把 elements 里面的东西，相反的，挪*出来*。

- interface: 所有跟 wgpu 有关的代码
- text: 所有跟 cosmic-text 有关
- tools: 有关用户输入处理
- widgets: 预设的用户组件

## 数据持久化

extension: `ln-save`

Windows: `%AppData%/Roaming/LnDrawer/world.ln-save`
Linux: `$XDG_DATA_HOME/LnDrawer/world.ln-save`

## 元素编组

元素编组主要是为了解决管理多元素交互，可见性，权限管理与元素分层等需求。

比如单选框就是这个功能非常典型的应用：只需简单地将同组的其他单选框取消选择，即可实现一定范围内的单选。

使用 `Group` 和 `group()` 函数来指定使用的编组：

```rust
// 这是 World 默认使用的编组
world.group_switch(Group::default());
```

更多时候会可能会希望进行临时切换：

```rust
// 会在离开时触发 Drop
let _guard: GroupGuard<'_> = world.group(Group::default());
```

接下来的 `single` 和 `foreach` 指令都会使用该 group。