## 颜色空间表示

用以下记法声明颜色空间

- 用 `L` 表示原始线性色彩（原始色彩）
- 用 `P` 表示进行 Alpha 预乘（贡献函数）
- 用 `S` 表示进行 gamma 编码（转移函数）

这种记法下：

- `S(L)` 表示 srgb 色彩，也是显示器期望的颜色
- 感知混合 `P(S(L))` 和光学混合 `P(L)` 进行 alpha 预乘考虑贡献
- PNG 格式保存的内容是 `S(L)` 也就是 gamma 编码非预乘
- `L_srgb` 表示光学线性的 linear-srgb 空间
- `L_palette` 表示调色板的色彩空间，比如 oklab 或者 hsl

## 重要的色彩空间规定

- 主无限画布，使用 `S(P(L))`
- 渲染表面，使用 `S(P(L))`
- 笔刷系统，使用 `S(L)`
- 绘制 Shader 上传，使用 `S(L)`
- 绘制 Shader 下载，使用 `S(L)`
- UI Shader 上传，使用 `P(L)` 
- UI Canvas，使用 `P(L)`

在光学线性（linear-srgb）`P(L)` 混合的：

- 画布渲染采样（硬件 sampler）
- mipmap 合成（`mipmap.wgsl`）
- 渲染组件的合成（通道 blending）
 
在感知线性（srgb）`P(S(L))` 混合的：

- 画布和背景图案（`render.wgsl`）
- 笔刷和画布（笔刷自带与 `merge.wgsl`）
- 图层之间合并（`merge.wgsl`）
- 矢量图形（还没做）

## 具体颜色行为

“颜色转换的最好实践就是不转换颜色。” ——「我说的」

### 1. 主无限画布存储

存储的是：`S(P(L))`

PNG 格式存储的是 `S(L)`，两者需要进行格式转换，并且我们的区块不能够完美存储所有数据。

### 2. {brush}/merge.wgsl

CPU 上传的是 `S(L)`

1. 笔刷颜色 `S(L)` 针对 alpha 通道添加遮罩，然后转换成 `P(S(L))`
2. 读取画布颜色 `S(P(L))` 转换成 `P(S(L))`
3. `P(S(L))` 感知混合
4. 转换回 `S(P(L))` 存储

### 3. mipmap.wgsl

1. 读取画布颜色 `S(P(L))`，解 gamma 变成 `P(L)`
2. `P(L)`光学混合
3. 编 gamma 变成 `S(P(L))` 存回去

### 4. render.wgsl

1. 直接采样得到的就是光学混合过滤好的 `P(L)`
2. 转换成 `P(S(L))`
3. 和背景纹样进行 `P(S(L))` 感知混合，此时 `alpha = 1`，可与 `S(L)` 等价
4. 解 gamma 得到 `L`，同样，可与 `P(L)` 等价
5. 按照 `P(L)` replace 输出

### 5. readback.wgsl

CPU 下载的是 `S(L)`

1. 图像加载得到的是 `S(P(L))`
2. 转换成 `S(L)` 并交给 CPU

### 6. {quad-widget}/canvas.wgsl

CPU 上传的是 `P(L)`

1. 输入 `P(L)`，进行处理后直接输出，quad 也期望 `P(L)`

### 7. {pallette-widget}

CPU 上传的是 `L_palette`

1. 输入 `L_palette` 用于匹配数据，生成 `L_srgb`
2. `L_srgb` 转换成主机色域 `L` 再预乘得到 `P(L)` 输出

CPU 端自己也会进行取色运算

1. 坐标得到 `L_palette`，这个数据传给 GPU
2. `L_palette` 生成 `L_srgb` 再转换成 `L` 然后 gamma 编码得到 `S(L)` 传给笔刷系统

有些色彩空间（hsl, hsv等）是生成 `S(L_srgb)` 的，目前直接可以用（L=L_srgb），可以跳过转换
