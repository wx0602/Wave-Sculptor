# Wave-Sculptor

Wave-Sculptor 是一个使用 Rust 编写的 WAV 音频查看、分析、编辑与批处理工具。项目同时提供原生 GUI 和命令行 CLI，两者复用同一套 `audio / wav / playback` 核心逻辑，适合作为 Rust 课程项目展示模块化设计、错误处理、所有权与借用、泛型、trait、测试和工程化实践。

## 1. 项目功能

### GUI 功能

- 打开本地 `.wav` 文件
- 显示波形并支持深色 / 浅色主题
- 支持播放、停止、另存为、导出选区
- 鼠标拖拽选择音频区间并高亮显示选区
- 鼠标滚轮缩放波形，右键拖动平移波形
- 支持 `适配视图`、按钮左右平移
- 显示播放线、播放进度、总时长、选区时长
- 支持撤销 / 重做
- 支持静音、放大、归一化、淡入、淡出、反转选区、裁剪为选区、去除首尾静音
- 右侧展示音频信息、选区信息和分析信息

### CLI 功能

- 查看音频统计信息
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
wave-sculptor input.wav --fade-out 3.0 6.0 -o output.wav
```

## 2. 项目结构

```text
.
├── Cargo.toml
├── assets
│   ├── dark.png
│   ├── light.png
│   ├── dark_bar.png
│   └── light_bar.png
└── src
    ├── lib.rs
    ├── main.rs
    ├── cli.rs
    ├── error.rs
    ├── audio
    │   ├── analyze.rs
    │   ├── buffer.rs
    │   ├── edit.rs
    │   ├── history.rs
    │   ├── mod.rs
    │   └── selection.rs
    ├── gui
    │   ├── app.rs
    │   ├── backgrounds.rs
    │   ├── fonts.rs
    │   ├── mod.rs
    │   ├── theme.rs
    │   ├── viewport.rs
    │   ├── waveform.rs
    │   └── waveform_mode.rs
    ├── playback
    │   ├── mod.rs
    │   └── player.rs
    └── wav
        ├── mod.rs
        ├── reader.rs
        └── writer.rs
```

### `mod` / `crate` 组织方式

项目不是只按文件夹摆放代码，而是明确使用了 Rust 的模块系统：

- `src/lib.rs` 作为 crate 根，显式导出：

```rust
pub mod audio;
pub mod cli;
pub mod error;
pub mod gui;
pub mod playback;
pub mod wav;
```

- `src/audio/mod.rs` 继续拆分核心音频逻辑：

```rust
pub mod analyze;
pub mod buffer;
pub mod edit;
pub mod history;
pub mod selection;
```

- `src/gui/mod.rs` 继续拆分界面模块：

```rust
pub mod app;
pub mod backgrounds;
pub mod fonts;
pub mod theme;
pub mod viewport;
pub mod waveform;
pub mod waveform_mode;
```

- `src/playback/mod.rs` 导出 `player` 模块。
- `src/wav/mod.rs` 导出 `reader` 和 `writer` 模块。
- `src/main.rs` 作为可执行入口，根据参数选择调用 `wave_sculptor::gui::run()` 或 `wave_sculptor::cli::run()`。

### 模块职责

- `audio/buffer.rs`
  - 定义 `AudioBuffer`
  - 提供 frame / sample / time 转换和切片能力
- `audio/selection.rs`
  - 定义 `Selection`
  - 处理选区的帧范围、时间范围与采样索引换算
- `audio/edit.rs`
  - 实现静音、放大、归一化、淡入淡出、反转、裁剪、去静音等编辑算法
- `audio/analyze.rs`
  - 计算峰值、RMS、削波采样点和静音片段
- `audio/history.rs`
  - 管理撤销 / 重做历史
- `wav/reader.rs`
  - 解析 RIFF / WAVE / fmt / data chunk
  - 读取 16-bit PCM WAV
- `wav/writer.rs`
  - 将 `AudioBuffer` 写回标准 PCM WAV
- `playback/player.rs`
  - 统一播放控制和播放状态查询
- `gui/app.rs`
  - 负责 GUI 状态管理和交互编排
- `gui/waveform.rs`
  - 负责波形绘制、选区绘制和播放线绘制
- `gui/backgrounds.rs`
  - 负责背景图片加载和纹理缓存

## 3. 依赖说明

- `eframe` / `egui`
  - 构建原生 GUI
- `clap`
  - 命令行参数解析
- `rodio`
  - 音频播放
- `rfd`
  - 本地文件选择对话框
- `thiserror`
  - 统一错误类型定义
- `image`
  - 读取卡片背景图片并转换为 `egui::ColorImage`

## 4. 编译与运行

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

## 5. 使用说明

### GUI 使用流程

1. 点击 `打开` 选择本地 WAV 文件。
2. 在波形区域拖拽形成选区。
3. 使用 `播放` / `停止` 试听完整音频或当前选区。
4. 使用工具栏执行静音、放大、归一化、淡入、淡出、反转选区等编辑操作。
5. 使用 `撤销` / `重做` 管理编辑历史。
6. 使用滚轮缩放、右键拖动平移，或点击 `向左` / `向右` / `适配视图` 控制可视区域。
7. 使用 `导出选区` 或 `另存为` 保存结果。

### CLI 使用流程

```bash
wave-sculptor input.wav --stats
wave-sculptor input.wav --normalize -o output.wav
wave-sculptor input.wav --mute 2.0 5.0 -o output.wav
wave-sculptor input.wav --fade-in 0.0 3.0 -o output.wav
wave-sculptor input.wav --fade-out 3.0 6.0 -o output.wav
```

## 6. 课程技术要求对应说明

### 6.1 模块化设计

- 项目使用 `mod` 将音频处理、WAV 编解码、播放控制、GUI、CLI、错误处理拆分到不同模块。
- 项目使用单一 `crate` 暴露 `audio / gui / wav / playback / cli / error` 模块，crate 根位于 `src/lib.rs`。
- `audio`、`gui`、`playback`、`wav` 不是纯目录，而是通过 `mod.rs` 继续声明子模块，体现了 Rust 模块系统的层次化组织。
- 本项目规模适中，因此未拆分为 `workspace`；保留单 crate 结构可以减少样板代码并保持教学项目的清晰度。

### 6.2 错误处理

- 全局统一使用 `type Result<T> = std::result::Result<T, WaveSculptorError>`。
- 业务逻辑优先使用 `?` 传播错误。
- 通过 `thiserror` 定义 `WaveSculptorError`，覆盖 I/O、格式不支持、无效 WAV、无效选区、播放错误和 CLI 参数错误等情况。
- `src` 目录下没有使用 `unwrap()` / `expect()` 直接规避核心错误处理。

### 6.3 Rust 核心特性

- ownership / borrowing
  - 编辑函数通过 `&mut AudioBuffer` 修改数据，分析和渲染逻辑通过 `&AudioBuffer` 只读访问，体现借用规则。
- struct / enum
  - 典型结构包括 `AudioBuffer`、`Selection`、`AudioAnalysis`、`AudioDocument`、`PlaybackStatus`。
  - 典型枚举包括 `WaveSculptorError`、`ThemeMode`、`WaveformMode`。
- trait
  - GUI 主状态实现了 `eframe::App` trait。
  - CLI 参数结构体派生 `clap::Parser`。
- 泛型
  - `parse_wav<R: Read + Seek>`、`write_wav<W: Write>`、`apply_edit<F>` 等函数体现了泛型和 trait bound 的使用。
- 生命周期
  - 本项目以所有权和借用为主，未额外引入显式生命周期参数；编译器可通过省略规则完成推断。

### 6.4 并发或异步

- 本项目未额外引入 `thread`、`tokio` 或 `async/await`。
- 原因是核心任务以本地音频处理和单窗口交互为主，当前同步实现已经满足需求。
- 播放由 `rodio` 库负责，应用层重点放在音频编辑和 GUI 交互逻辑。

### 6.5 测试

- 当前 `src` 内共包含 11 个单元测试。
- 覆盖方向包括：
  - WAV 头解析
  - WAV 采样读取
  - 归一化
  - 淡入 / 淡出
  - 反转选区
  - 裁剪选区
  - RMS 计算
  - 静音片段检测
  - 时间到帧的选区转换
  - 双声道 sample index 计算

### 6.6 工程规范

提交前建议执行：

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

### 6.7 README 与文档

- 本 README 包含项目简介、功能说明、依赖说明、编译运行方式、技术要求对应说明和局限性说明。
- 另附 [期末实验报告.md](./期末实验报告.md) 作为课程报告草稿，可直接整理进老师提供的 `.docx` 模板。

## 7. 测试与质量检查

推荐按以下顺序执行：

```bash
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

如果需要仅验证库代码，也可以执行：

```bash
cargo test --lib
```

## 8. 已知限制

- 当前项目仅支持 16-bit PCM WAV。
- 当前仅支持单声道和双声道输入。
- 不支持压缩编码、浮点 WAV、24-bit / 32-bit WAV 和多于双声道的音频。
- 当前未加入频谱图、批量目录处理和插件化效果链。
