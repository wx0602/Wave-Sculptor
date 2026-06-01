# Wave-Sculptor

Wave-Sculptor 是一个使用 Rust 编写的 WAV 音频查看、分析、编辑与批处理工具。项目同时提供原生 GUI 和命令行 CLI，两者共用同一套 `audio / wav / playback` 核心逻辑，适合作为 Rust 课程项目展示模块化设计、错误处理、所有权管理、测试与工程化能力。

## 功能概览

### GUI 功能

- 打开本地 `.wav` 文件
- 支持 16-bit PCM、单声道和双声道 WAV
- 显示波形、播放、停止、另存为
- 鼠标拖拽实时选择区域，并高亮显示选区
- 右侧显示选区起点、终点、时长和帧范围
- 播放时显示 playhead 播放位置
- 显示当前播放时间 / 总时长
- 支持撤销 / 重做
- 支持静音、放大、归一化、淡入、淡出、反转选区、裁剪为选区、导出选区
- 支持去除首尾静音
- 支持波形显示模式切换：
  - 混合
  - 左声道
  - 右声道
  - 分离立体声
- 支持鼠标滚轮缩放波形
- 支持右键拖动平移波形
- 支持按钮左右平移和 `适配视图`
- 右侧显示分析信息：
  - 峰值振幅
  - RMS / 均方根
  - clipping / 削波采样点数量
  - 静音片段数量

### CLI 功能

- 查看统计信息
- 归一化输出
- 按时间区间静音
- 按时间区间淡入
- 按时间区间淡出

示例：

```bash
wave-sculptor input.wav --stats
wave-sculptor input.wav --normalize -o output.wav
wave-sculptor input.wav --mute 2.0 5.0 -o output.wav
wave-sculptor input.wav --fade-in 0.0 3.0 -o output.wav
```

## 编译与运行

### GUI 模式

```bash
cargo run
```

### CLI 模式

```bash
cargo run -- input.wav --stats
cargo run -- input.wav --normalize -o normalized.wav
cargo run -- input.wav --mute 2.0 5.0 -o muted.wav
```

### 格式化与检查

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## GUI 使用说明

1. 点击 `打开` 选择本地 WAV 文件。
2. 在波形区域按下鼠标并拖动，实时形成选区。
3. 可以使用 `播放` / `停止` 试听整段或选区。
4. 通过 `静音`、`放大`、`归一化`、`淡入`、`淡出`、`反转选区` 等按钮进行编辑。
5. 使用 `撤销` / `重做` 回退或恢复编辑结果。
6. 使用滚轮缩放波形，用右键拖动平移，或点击 `向左` / `向右` / `适配视图` 控制视图。
7. 使用 `导出选区` 保存片段，或用 `另存为` 保存当前完整音频。

## CLI 使用说明

### 查看统计

```bash
wave-sculptor input.wav --stats
```

### 归一化到 90% 峰值

```bash
wave-sculptor input.wav --normalize -o output.wav
```

### 对 2.0s 到 5.0s 做静音

```bash
wave-sculptor input.wav --mute 2.0 5.0 -o output.wav
```

### 对 0.0s 到 3.0s 做淡入

```bash
wave-sculptor input.wav --fade-in 0.0 3.0 -o output.wav
```

### 对 3.0s 到 6.0s 做淡出

```bash
wave-sculptor input.wav --fade-out 3.0 6.0 -o output.wav
```

## 模块设计

```text
src
├── lib.rs
├── main.rs
├── cli.rs
├── error.rs
├── audio
│   ├── buffer.rs
│   ├── selection.rs
│   ├── edit.rs
│   ├── analyze.rs
│   └── history.rs
├── wav
│   ├── reader.rs
│   └── writer.rs
├── playback
│   └── player.rs
└── gui
    ├── app.rs
    ├── fonts.rs
    ├── viewport.rs
    ├── waveform.rs
    └── waveform_mode.rs
```

### 模块职责

- `audio/buffer.rs`
  - 定义 `AudioBuffer`
  - 提供 frame / sample / time 基础转换
- `audio/selection.rs`
  - 管理选区数据结构 `Selection`
  - 负责时间到帧、帧到 sample index 的转换
- `audio/edit.rs`
  - 放置所有核心编辑逻辑
  - 包括 `mute / amplify / normalize / fade in / fade out / reverse / cut / trim silence`
- `audio/analyze.rs`
  - 负责峰值、RMS、削波点和静音片段统计
- `audio/history.rs`
  - 提供 `AudioDocument` 和撤销 / 重做历史栈
  - GUI 不直接管理历史细节
- `wav/reader.rs`
  - 解析 RIFF / fmt / data chunk
  - 读取 16-bit PCM WAV
- `wav/writer.rs`
  - 将 `AudioBuffer` 写回标准 PCM WAV
- `playback/player.rs`
  - 统一播放控制
  - 暴露播放状态、playhead 与播放时间
- `gui/viewport.rs`
  - 管理视图缩放和平移
- `gui/waveform.rs`
  - 绘制波形、选区和 playhead
- `gui/app.rs`
  - 负责交互编排、按钮事件和信息展示
- `cli.rs`
  - 使用 `clap` 解析命令行参数
  - 复用 `audio / wav` 核心逻辑进行批处理

## Rust 特性体现

- 模块化设计
  - GUI、CLI、音频编辑、分析、历史、播放、WAV 编解码分层清晰
- 所有权与借用
  - `AudioBuffer` 通过不可变借用 / 可变借用在不同模块间安全共享
- 错误处理
  - 核心函数统一返回 `Result<T, WaveSculptorError>`
  - 尽量避免 `unwrap / expect` 进入核心逻辑
- `struct / enum`
  - `AudioBuffer`
  - `Selection`
  - `AudioAnalysis`
  - `PlaybackStatus`
  - `WaveformMode`
- 泛型
  - WAV 读写层继续使用 `Read + Seek`、`Write` 泛型
- 可测试性
  - 编辑、分析、选区转换等核心能力均可脱离 GUI 单测

## 测试覆盖

当前代码包含以下方向的测试：

- WAV Header 解析
- WAV 采样读取
- normalize
- fade in / fade out
- reverse selection
- RMS 计算
- silence detection
- Selection 时间与 sample index 转换

## 说明

- 当前项目聚焦 16-bit PCM WAV，不支持压缩编码、浮点 WAV、24-bit / 32-bit WAV 和多于双声道的输入。
- GUI 与 CLI 使用同一套 `audio / wav` 核心逻辑，避免重复实现。
- 若在 Windows 上显示中文方框，请确认系统存在常见中文字体，例如 `simhei.ttf`、`msyh.ttc`、`simsun.ttc`。
