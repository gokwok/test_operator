# macOS Action Effects Design

日期：2026-03-28

## 1. 目标

为 `operator-platform-macos` 增加一组**仅用于执行反馈**的操作特效，用于人工调试、演示和可视化验证。

第一阶段目标特效：

- `click effect`
- `move trail`
- `drag path`
- `scroll flash`
- `keyboard HUD`

这些特效的目标是：

- 提高人工调试时的可见性
- 帮助验证点击、拖拽、滚动、键盘输入是否落在预期位置
- 不改变 Operator 现有 northbound shell contract

## 2. 非目标

第一阶段**不做**以下事情：

- 不修改 `operator-core` 中的 `Action`、`ActionRequest`、`ActionOutcome`
- 不新增 CLI 参数、MCP 参数、Agent 参数
- 不新增公共 capability
- 不通过 target 配置或 runtime 配置控制特效
- 不保证特效一定显示成功
- 不把特效本身视为业务副作用

一句话：**特效是 macOS driver 内部的可选反馈层，不是自动化语义的一部分。**

## 3. 总体原则

### 3.1 仅限 macOS driver 内部

所有设计和实现都限定在：

- [`crates/operator-platform-macos`](/Users/gokwok/code/work/Operator/crates/operator-platform-macos)

不把实现扩散到：

- `operator-core`
- `operator-runtime`
- `operator-cli`
- `operator-mcp`
- `operator-agent`

### 3.2 编译期开关，默认关闭

特效通过 Cargo feature 控制：

- `operator-platform-macos/action-effects`

默认关闭。

这意味着：

- 常规构建不受影响
- 自动化路径默认不带视觉噪音
- 调试构建可显式打开

### 3.3 best-effort

特效渲染失败时：

- 不能让 action 失败
- 不能改变 action 成功/失败语义
- 不能阻塞自动化链路

允许的行为只有：

- 静默降级为 no-op
- 或在 driver 内部记录调试日志

### 3.4 不污染公共结果模型

第一阶段不修改：

- `ActionOutcome`
- `ActionSideEffect`
- `ToolResult`

特效显示不是新的业务 side effect，也不是 northbound 稳定返回字段。

## 4. 第一阶段效果范围

### 4.1 Click effect

在点击点显示短暂的 ring / ripple 效果。

触发动作：

- `click`

覆盖点击模式：

- `Left`
- `Right`
- `Middle`
- `Double`

### 4.2 Move trail

在指针移动目标点显示短暂尾迹或落点 pulse。

触发动作：

- `move`

### 4.3 Drag path

在拖拽路径上显示从 `from -> to` 的短暂路径反馈。

触发动作：

- `drag`
- `swipe`

### 4.4 Scroll flash

在滚动发生点显示短暂方向闪烁或脉冲。

触发动作：

- `scroll`

### 4.5 Keyboard HUD

在屏幕上短暂显示按键文本或组合键标识。

触发动作：

- `type`
- `press`
- `hotkey`

## 5. 架构边界

### 5.1 不进入 core action 语义

当前公共动作语义定义在：

- [`crates/operator-core/src/action.rs`](/Users/gokwok/code/work/Operator/crates/operator-core/src/action.rs)

第一阶段不往这些类型里添加：

- `effect_enabled`
- `effect_style`
- `render_feedback`
- 任何类似字段

原因：

- 这些字段不是跨平台共性
- 它们不改变 automation 语义
- 把它们放进 core 会污染 CLI/MCP/Agent schema

### 5.2 不新增 capability

当前能力模型定义在：

- [`crates/operator-core/src/capability.rs`](/Users/gokwok/code/work/Operator/crates/operator-core/src/capability.rs)

第一阶段不新增：

- `Capability::Extension(macos.action_effects)`

原因：

- 特效不是“平台是否支持某种自动化能力”的问题
- 它只是调试反馈层

### 5.3 由 macOS driver 自己调度

当前动作执行主链在：

- [`crates/operator-platform-macos/src/driver.rs`](/Users/gokwok/code/work/Operator/crates/operator-platform-macos/src/driver.rs)

输入事件合成在：

- [`crates/operator-platform-macos/src/input.rs`](/Users/gokwok/code/work/Operator/crates/operator-platform-macos/src/input.rs)

第一阶段建议的职责分工：

- `InputSynthesizer`
  - 只负责发真实输入事件
- `ActionEffects`
  - 只负责视觉反馈
- `MacosDriver`
  - 在动作成功后调用效果层

即：

- 不把效果逻辑塞进 `input.rs`
- 不让 `input.rs` 同时承担“发事件 + 画特效”两类职责

## 6. 代码布局

第一阶段建议新增：

```text
crates/operator-platform-macos/
  src/
    driver.rs
    input.rs
    effects.rs
```

其中：

- `effects.rs`
  - 定义 `ActionEffects` facade
  - 定义 feature-enabled 与 feature-disabled 两条实现路径

建议结构：

```rust
pub(crate) struct ActionEffects;

impl ActionEffects {
    pub(crate) fn on_click(&self, point: Option<Point>, mode: ClickMode) {}
    pub(crate) fn on_move(&self, point: Point) {}
    pub(crate) fn on_drag(&self, from: Point, to: Point) {}
    pub(crate) fn on_scroll(&self, point: Option<Point>, dx: f64, dy: f64) {}
    pub(crate) fn on_keyboard(&self, label: &str) {}
}
```

feature-disabled 路径下：

- 全部为 no-op

feature-enabled 路径下：

- 走真实渲染实现

## 7. Cargo feature 设计

第一阶段只定义一个总开关：

```toml
[features]
default = []
action-effects = []
```

原因：

- 第一阶段目标是快速落一个可用的调试反馈层
- 先不把 feature 过度细分成：
  - `click-effects`
  - `pointer-effects`
  - `keyboard-effects`

如果后续需要更细粒度控制，再拆。

可选地，在 [`crates/operator-cli/Cargo.toml`](/Users/gokwok/code/work/Operator/crates/operator-cli/Cargo.toml) 提供转发 feature，方便最终调试构建：

```toml
[features]
default = []
macos-action-effects = ["operator-platform-macos/action-effects"]
```

## 8. Driver 接入点

建议在动作成功路径中触发特效：

- `click()` 成功后 -> `on_click`
- `move_pointer()` 成功后 -> `on_move`
- `drag()` / `swipe()` 成功后 -> `on_drag`
- `scroll()` 成功后 -> `on_scroll`
- `type_text()` / `press()` / `hotkey()` 成功后 -> `on_keyboard`

必须满足：

- 先执行真实动作
- 再尝试渲染特效
- 特效失败不可覆盖真实动作结果

伪代码：

```rust
self.input_synthesizer.click(point, mode)?;
let _ = self.effects.on_click(point, mode);
return Ok(outcome);
```

这里的 `let _ =` 表示：

- 特效渲染失败不传播

## 9. 渲染语义

### 9.1 默认关闭

这是第一阶段最重要的行为约束。

原因：

- 当前 Operator 依赖截图进行：
  - `capture`
  - macOS / Harmony 平台验证
  - agent 自动 observe
- 如果特效默认打开，可能污染截图和视觉验证

因此：

- release 默认不启用
- 正常 CI 默认不启用
- 仅调试或人工演示构建显式启用

### 9.2 与截图的关系

第一阶段必须在文档和验证里明确记录：

- 特效是否会被 `screencapture` 捕获
- 特效是否会干扰自动 observe
- 若会干扰，是否需要人工调试专用 target / build

在没有真实验证结论之前，不允许默认打开。

### 9.3 不追求严格同步

第一阶段不要求：

- 特效播放完成后才返回 action
- 特效时序精确对齐每一帧输入

目标是“足够可见”，不是“精确可视化回放系统”。

## 10. 平台实现约束

第一阶段只要求“实现落在 macOS driver 层”，不要求一开始就固定渲染 backend。

允许的实现演进方式：

- 先做一个最小 in-process overlay
- 若 AppKit/线程限制过强，再在 driver 内部切换到更稳的私有渲染后端

但这些都必须满足：

- 对外 API 不变
- 仍然只由 `operator-platform-macos` 私有维护
- 不向上层暴露新的 northbound 概念

## 11. 验证策略

### 11.1 自动化验证

自动化验证重点不在“看见特效”，而在：

- feature 关闭时不影响现有行为
- feature 打开时编译通过
- driver 路径仍然正常执行动作

最小自动化验证：

```bash
cargo test -p operator-platform-macos
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

并补一条构建验证：

```bash
cargo build -p operator-cli
cargo build -p operator-cli --features macos-action-effects
```

### 11.2 人工验证

由于特效是视觉反馈，第一阶段必须做人机辅助验证。

至少验证：

- `click`
- `move`
- `drag`
- `scroll`
- `type`
- `press`
- `hotkey`

人工验证需要确认：

- 特效是否出现
- 位置是否基本正确
- 会不会阻塞真实动作
- 会不会影响后续 capture / observe

## 12. 风险

### 12.1 截图污染

最大风险。

若特效会被截图捕获：

- agent loop 的视觉判断会被污染
- capture 验收可能不稳定

应对策略：

- 第一阶段默认关闭
- 在验证报告中明确记录行为

### 12.2 UI backend 约束

macOS 的 overlay / HUD 渲染可能受：

- AppKit 主线程
- 事件循环
- 权限

等因素影响。

因此第一阶段必须坚持：

- 特效失败不影响 action 成功

### 12.3 过度扩散

如果把特效做成：

- 新 capability
- 新 CLI flag
- 新 runtime config
- 新 Action 字段

会迅速污染当前稳定 shell contract。

第一阶段必须避免这种扩散。

## 13. 实施拆分

建议 Linear 实施顺序：

1. 设计与边界确认
2. feature-gated 基础设施
3. pointer effects
4. keyboard HUD
5. 人工验证与文档发布

该链条的目标是：**先落一个只影响 macOS driver、默认关闭、可演示的调试反馈层。**
