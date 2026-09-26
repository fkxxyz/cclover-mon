# cclover-mon - 项目指南

## 本项目做什么

`cclover-mon` 是一个面向 Linux 和 Windows 的低开销原生桌面系统监控器。项目保持一套共享的 Rust 数据模型和 Iced UI，同时把原生采集与桌面集成隔离在平台后端中。

## 架构

**主要技术栈**：Rust 2024、Cargo、Iced 0.14。Linux Wayland 窗口定位使用 `iced_layershell` 0.19.1。

**运行时指标流**：

```text
OS native interfaces
  ↓
platform: Linux | Windows
  ↓
core: sampling + model + history + aggregation
  ↓
app / UI
```

**源码静态依赖边界**：

```text
app composition root ───────→ core
        │                     ↑
        └────────→ platform ──┘
```

职责规则：

- `core` 负责平台中立的指标类型、delta/rate 推导、Top-N 聚合、有界历史与采样契约，包括 `Collector`。
- `platform` 负责操作系统特定的采集与桌面集成，实现由 `core` 定义的采样契约，并返回由 `core` 定义的平台中立快照。`core` 不得依赖 `platform`。
- `ui` 只渲染共享 `MonitorState`；不采集指标，也不调用平台 API。
- 原生数据保持类型化并在进程内传递。不要为指标流引入内部 JSON 或前后端 IPC。
- 优先使用原生采集，不使用周期性 subprocess 轮询。Linux 数据源应按场景使用 `/proc`、`/sys`、netlink、ioctl、socket 或 D-Bus。
- 采样节奏与渲染节奏保持独立。
- 历史数据保持有界，并使用固定容量环形缓冲语义。
- 仅在 SDK 只能通过 C++ 使用时引入 C++，并通过窄 C ABI 隔离，最终链接进同一个可执行文件。

## 项目结构

```text
src/app.rs                 应用状态、采样订阅、UI 更新流
src/core/model.rs          共享类型化快照与历史模型
src/core/sampler.rs        delta/rate 推导、Top-N、采样状态
src/core/history.rs        有界历史更新
src/platform/linux/        Linux 原生采集器，按指标职责拆分
src/platform/windows.rs    Windows 后端；当前采集器仍为占位实现
src/ui/mod.rs              共享 Iced 展示层
src/ui/graph.rs            历史曲线渲染
docs/architecture/         架构 Views 与治理数据
archdoc.ts                 架构文档导航与校验工具
```

## 架构工作流

开始仓库工作前，先运行：

```bash
bun archdoc.ts --help
```

修改实现前，使用 `archdoc` 找出与任务相关的 Views：

```bash
bun archdoc.ts choices --stakeholder developer
bun archdoc.ts views --stakeholder developer --concern <concern> --activity <activity> --facet area=<area> [--viewpoint <viewpoint>]
```

在改代码前先阅读返回的 Markdown 文件。架构文档发生变化时，运行：

```bash
bun archdoc.ts check
```

## 开发规则

- 从旧 Quickshell 监控器迁移时保留产品行为，但具体实现必须遵守本仓库架构，不复制旧代码结构。
- 新增指标时，先扩展共享类型模型与 core 推导，再实现平台采集器，最后接入 UI。
- Linux 设备发现应尽量基于 capability，而不是维护设备名黑名单。例如，物理块设备通过 sysfs capability（`/sys/block/<dev>/device`）识别。
- 指标不可用时应表达为不可用数据，不要伪造为 0；只有在 0 本身就是正确语义时才使用 0。
- 平台特有名称、handle、struct 与 API 不得进入共享 UI 或 core-facing 契约。
- 保持每个平台单进程、单可执行文件的部署模型。

## 验证

实现改动后运行完整项目验证：

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
bun archdoc.ts check
```

Linux UI 或窗口定位发生变化时，还要做真实 Wayland runtime 验证。如果 shell 没有继承桌面环境，先在 `/run/user/$(id -u)/wayland-*` 下找到 compositor socket，再设置匹配的 `XDG_RUNTIME_DIR` 和 `WAYLAND_DISPLAY`，然后启动 `target/release/cclover-mon`。

## 常见陷阱

- `iced_layershell` 0.19.1 当前要求精确 pin `winit-core` 与 `winit-common` `0.31.0-beta.2`。没有完成构建与 runtime 验证前，不要放宽这些版本约束。
- Linux 已实现并完成 runtime 验证；Windows collector 仍是占位实现。不要把 Windows 指标能力描述为已经完整对齐。
- UI 改动不能只依赖静态编译验证。此前真实 runtime 检查发现过布局重叠和虚拟块设备误入，而 Rust 测试与 Clippy 都没有发现。
- Linux 面板刻意使用右上角锚定、bottom layer、零 exclusive zone。除非产品行为有意改变，否则保留这些语义。

## 构建与运行

Release 构建：

```bash
cargo build --release
```

开发运行：

```bash
cargo run --release
```

构建产物：

```text
target/release/cclover-mon
```

每个目标平台最终产出一个应用可执行文件，其中包含所选平台后端以及任何必要的原生 bridge 对象。
