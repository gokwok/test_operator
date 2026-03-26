# Operator Agent Module Design

日期：2026-03-26

## 目的

本文档定义 `operator-agent` 当前阶段的权威设计。

当前阶段的目标是提供一个**本地单 session、单 target、单 agent loop** 的执行器，并通过统一 `operator` CLI 暴露自然语言入口：

- 支持多平台 runtime
- 支持多模型接入
- agent 内部不通过 CLI / MCP 调用操作
- 直接复用 `operator-runtime` 的 `ToolRegistry`
- agent loop 的热路径尽可能贴近 `/Users/gokwok/code/work/operation_agent/agent_v3`

本文档不覆盖：

- A2A northbound protocol
- 多会话调度
- pause / resume
- AX tree 裁剪与树驱动热路径

## 设计输入

本设计基于以下前提：

1. Operator 当前已经具备：
   - `operator-core` 的 typed 领域模型
   - `operator-runtime` 的 `RuntimeCore` / `ToolRegistry`
   - `operator-platform-macos` 的平台实现
   - 统一的 `operator` CLI / `operator mcp serve`
2. 模型层抽象直接参考 `/Users/gokwok/code/work/kernel_agent/crates/base`
3. agent loop 的热路径应尽可能贴近 `/Users/gokwok/code/work/operation_agent/agent_v3`

这里“贴近 `agent_v3`”指的是：

- 状态保留在内存
- 动作后自动刷新视觉状态
- planner 消费前后视觉输入，而不是默认消费完整树
- 模型上下文保持紧凑，避免把审计日志直接喂回模型

## 当前优化焦点

当前阶段只聚焦以下三项优化：

1. **原生视觉输入**
   - 前后两张截图必须真正作为模型图片输入进入 planner
   - 不再只把 artifact id 作为文本传给模型
2. **语义化 prompt 组装**
   - planner prompt 不再以 pretty JSON 形式序列化工程对象
   - 改成更接近 `agent_v3` 的任务语义渲染
3. **模型上下文与审计日志分层**
   - 审计/回放需要完整 transcript
   - 模型只消费 compact history、notes、工具结果摘要和前后视觉

以下差异明确留到后续阶段：

- finish gate 进一步向纯视觉闭环靠拢
- parser / validator 向更宽容的 `agent_v3` 风格靠拢
- 增加更丰富的人机协作状态，例如 installed apps、foreground bundles、input-required

## 设计目标

- `operator-agent` 作为独立 crate，不让 runtime 反向依赖模型/provider
- agent 直接调用内建工具，不经由 CLI tool bridge，也不经由 MCP
- agent 不感知具体平台实现，只依赖 `Target`、`CapabilitySet` 和工具 schema
- `gpt-5.4` 与 `doubao-seed` 使用统一 planner contract
- 热路径优先内存增量更新，不把完整 snapshot store / session store 读写放进每一步规划前
- 默认 loop 以 screenshot-only observe 为主，不把 AX tree 放进当前热路径
- 当前/上一轮截图必须以模型原生图片内容进入 planner
- 日志与模型上下文分层，避免 prompt 负载随审计需求膨胀

## 非目标

- 第一期开 A2A northbound surface
- 第一期开多 session scheduler
- 通过 CLI 作为 agent 的工具桥
- 让 runtime 内核依赖模型/provider SDK
- 为每个模型单独设计 planner 协议
- 当前阶段把 AX tree 纳入默认热路径
- 当前阶段重写 parser / validator 协议

## 总体架构

`operator-agent` 采用 **独立 crate + 内建工具直调 + 视觉优先的内存增量 loop** 结构。

```text
operator-agent
  ├── AgentRunner
  ├── LoopState
  ├── ModelRegistry / ModelProvider
  ├── PlannerRenderer
  ├── ModelContextBuffer
  ├── VisualContextLoader
  ├── DecisionParser / DecisionValidator
  ├── FinishGate
  ├── ToolExecutor
  ├── LoopStateContextManager
  ├── ObservationCache
  ├── SessionJournal
  └── AgentConfig
```

核心原则：

- Agent 是 entry 层，不是 runtime 内核的一部分
- tool execution 统一走 `ToolRegistry`
- 模型只负责“生成下一步决策”，不直接接触平台或 CLI
- 平台差异通过 `CapabilitySet + ToolCatalog` 下沉
- 热路径中只保留当前视觉、上一轮视觉、少量文本上下文和工具结果摘要
- 模型上下文与审计日志严格分层

## 与 `operation_agent/agent_v3` 的关系

要复用的是执行流模式，不是技术栈。

保留的思想：

- planner / validator / executor 分层
- 状态在 agent 内聚
- 动作后自动刷新视觉状态
- planner 默认消费前后视觉状态
- transcript 与 planner 上下文保持不同粒度

不引入的东西：

- Thinkflow runtime
- LangGraph
- 通过 MCP 再转一次工具调用
- A2A ingress 约束

在 `agent_v3` 中，executor 通过远程 `tools/call` 刷新截图；在 Operator 中，这层改成**直接调用本地 `ToolRegistry`**。

## crate 边界

新增 workspace member：

- `operator-agent`

依赖方向：

```text
operator-agent
  ├── depends on operator-core
  ├── depends on operator-runtime
  └── does NOT get depended on by operator-runtime
```

## 第一期开箱能力

第一期 `operator-agent` 提供一个库级本地执行器；其 northbound shell surface 由统一 `operator` CLI 承载：

- 输入：
  - `task`
  - `target`
  - `model`
- 输出：
  - 最终完成结果
  - session transcript
  - tool trace

第一期运行边界：

- 单 session
- 单 target
- 单 agent loop
- 单任务运行到完成或失败

第一期 public CLI 入口：

- `operator agent <task>`
- `--model <gpt-5.4|doubao-seed>`
- `--max-steps <n>`
- `--json`
- `--target <target>`
- `--timeout-ms <ms>`

说明：

- 这里的 CLI 仅作为 northbound 入口来承载任务文本、模型选择和输出格式
- agent 内部执行工具时仍直接调用 `ToolRegistry`

## 核心组件

### 1. `AgentRunner`

对外唯一入口，负责：

- 建立 runtime session
- 初始化 `LoopState`
- 驱动 step loop
- 调用模型
- 调用工具
- 调度自动 observe
- 触发 finish gate
- 输出最终结果

建议接口：

```rust
pub struct AgentRunner {
    runtime: Arc<Runtime>,
    models: ModelRegistry,
    config: AgentConfig,
}

impl AgentRunner {
    pub async fn run(&self, req: AgentRunRequest) -> Result<AgentRunResult, AgentError>;
}
```

### 2. `LoopState`

热路径状态应是**内存增量状态对象**，不要求每步从 store 重新组装。

建议字段：

```rust
pub struct LoopState {
    pub session_id: SessionId,
    pub target: TargetId,
    pub task: String,
    pub status: AgentSessionStatus,
    pub turn_index: u32,
    pub step_index: u32,
    pub planner_attempt: u32,
    pub model_context: ModelContextBuffer,
    pub history: Vec<LoopHistoryItem>,
    pub tool_trace: Vec<ToolTraceEntry>,
    pub notes: Vec<String>,
    pub current_observation: Option<VisualObservationSummary>,
    pub visual_window: VecDeque<VisualFrame>,
    pub consecutive_error_count: u32,
    pub last_error_fingerprint: Option<String>,
}
```

说明：

- `model_context`：给模型消费的紧凑上下文缓存
- `history`：planner 可消费的增量执行历史
- `tool_trace`：结构化工具结果，供摘要和调试使用
- `current_observation`：最近一次 observe 的简短摘要
- `visual_window`：内存视觉窗口，**当前固定上限为 2**
- `notes`：来自 parse retry 或 finish gate 的短反馈

### 3. `ModelContextBuffer`

`ModelContextBuffer` 只服务模型，不服务审计。

建议职责：

- 保存最近若干轮紧凑 history
- 保存 planner feedback notes
- 保存当前/上一轮视觉引用
- 保存 compact tool result summaries

明确约束：

- 不保存完整 transcript
- 不保存 pretty JSON 工具结果
- 不承担回放职责

### 4. 模型抽象与封装

模型层直接参考 `/Users/gokwok/code/work/kernel_agent/crates/base` 的三层结构：

- `model::types`
- `model::provider`
- `model::event`

第一期支持两个模型名：

- `gpt-5.4`
- `doubao-seed`

建议保留统一 provider 接口：

```rust
pub trait ModelProvider: Send + Sync + 'static {
    fn stream(&self, req: ModelRequest) -> ModelStream;
}
```

`ModelRegistry` 负责把逻辑模型名解析成 `ModelConfig + Provider`。

第一期真正启用的能力可收敛为：

- 文本输出
- 视觉输入
- 可选 JSON mode
- 非 streaming 主流程

### 5. `PlannerRenderer`

`PlannerRenderer` 负责把 `LoopStateContextManager` 的结果渲染成模型请求。

关键约束：

- 不再把 planner request pretty JSON 整块塞进 prompt
- 不再把完整工具目录 schema 原样塞进 prompt
- 使用“任务语义 + 历史摘要 + notes + 前后视觉”的渲染方式

推荐结构：

- `system`
  - 决策规则
  - 输出契约
  - 工具使用原则
- `user`
  - task
  - compact history
  - notes
  - target summary
  - previous screenshot
  - current screenshot

### 6. `VisualContextLoader`

`VisualContextLoader` 负责把内存中的视觉引用变成真正的模型图片输入。

职责：

- 读取 `visual_window` 中的 `artifact_id`
- 加载图片字节或 data URL
- 组装 `ContentBlock::Image`
- 生成 `previous_visual_input` 和 `current_visual_input`

约束：

- 视觉窗口上限固定为 2
- 不在热路径保留更多历史截图
- 对加载失败的视觉输入允许降级为缺失，而不是阻塞整个 loop

### 7. `Planner`

Planner 负责消费以下输入：

- 用户任务
- 当前 target 与 capability 摘要
- 紧凑工具目录
- 当前视觉输入
- 上一轮视觉输入
- agent notes
- 最近若干轮工具结果摘要

Planner 不直接生成执行结果，只生成**下一步决策**。

### 8. `DecisionParser` / `DecisionValidator`

当前阶段不使用 provider-native tool calling。

模型输出统一解析成：

```rust
pub enum AgentDecision {
    CallTool {
        name: String,
        arguments: serde_json::Value,
        thought: Option<String>,
        summary: String,
    },
    Finish {
        summary: String,
    },
    Fail {
        reason: String,
    },
}
```

parse / validate 失败后，不直接终止，而是将失败原因注入 `notes` 并重试，最多 `MAX_PARSE_ATTEMPTS` 次。

当前阶段**不重写** parser / validator 协议。

### 9. `FinishGate`

当前阶段保留“防止模型假完成”的目标，但不把它作为本轮优化重点。

当前要求：

1. deterministic gate
   - 最近一步如果是 side-effect action，则必须已经拿到 fresh observe
   - 必须存在最近一张可用视觉输入
2. 轻量 reflection（可选）
   - 仅在 deterministic gate 通过但仍不确定时触发
   - 输入只包含：task、最近历史、notes、current visual、previous visual

当前阶段不继续扩大 finish gate 的工作范围。

### 10. `ToolExecutor`

设计要求：

- 不通过 CLI
- 不通过 MCP
- 直接调用 `ToolRegistry`

调用路径：

```text
AgentRunner
  -> ToolExecutor
    -> ToolRegistry.invoke(name, json)
      -> RuntimeCore.observe/query/act
        -> PlatformDriver
```

### 11. `ObservationCache`

`ObservationCache` 负责管理热路径视觉状态：

- 当前截图
- 上一轮截图
- 当前 observe 摘要

当前硬约束：

- 只保留最近 2 张图
- 新图进入时，最旧的图立即丢弃
- 不在内存中保留完整截图历史

### 12. `SessionJournal`

`SessionJournal` 只负责审计与回放，不负责模型上下文。

职责：

- 在内存中累积完整事件
- 在 `turn-end`、`run-end`、失败退出前 flush
- 保持 transcript 可回放

明确约束：

- 不把完整 transcript 直接回灌给模型
- 不承担 planner context 组装职责

## 工具目录设计

Agent 内部继续使用 runtime 的 flat tool names：

- `observe`
- `snapshot-get`
- `artifact-get`
- `get-focus`
- `list-apps`
- `list-windows`
- `permissions-status`
- `capabilities`
- 所有 action 工具

当前阶段说明：

- `observe` 仍保留为显式工具
- 但 agent loop 默认不依赖模型主动调用 `observe`
- 显式 `observe(include_elements=true)` 作为冷路径能力保留

## 多平台支持策略

agent 模块不认识 macOS / Windows / Harmony 的实现细节。

它只依赖：

- `TargetDescriptor`
- `CapabilitySet`
- `ToolCatalog`
- 工具返回结果

## 多模型支持策略

当前阶段只保留两个逻辑模型名：

- `gpt-5.4`
- `doubao-seed`

公共层统一使用 `Context` / `Message` / `ContentBlock` / `ToolSpec`。

## 观察与内存上下文

Operator 的底层仍是 snapshot-first，但当前 agent loop 不应把“完整 snapshot 读取 + 树摘要”放在热路径。

当前上下文装配策略：

- 默认使用 **screenshot-only observe**
- `include_screenshot = true`
- `include_elements = false`
- `observe` 结果进入内存 `visual_window`
- planner 默认只消费：
  - `current_visual_input`
  - `previous_visual_input`
  - `current_observation.surface`
  - 简短工具结果摘要
- 不把完整 AX tree 直接注入 prompt
- 不在每步规划前从 snapshot store 重新加载最近 snapshot

建议上下文对象：

```rust
pub struct PlannerContext {
    pub task: String,
    pub target_summary: String,
    pub tools: Vec<PlannerToolSummary>,
    pub recent_history: Vec<LoopHistoryItem>,
    pub recent_tool_results: Vec<ToolResultSummary>,
    pub current_observation: Option<VisualObservationSummary>,
    pub current_visual_ref: Option<VisualFrameRef>,
    pub previous_visual_ref: Option<VisualFrameRef>,
    pub notes: Vec<String>,
}
```

说明：

- `PlannerContext` 只保留**引用和摘要**
- 真正的 `ContentBlock::Image` 由 `VisualContextLoader` 在渲染阶段装入模型请求

## 自动 observe 规则

当前阶段 agent loop 改成**流程自动采样**：

- 任务启动后，`AgentRunner` 自动执行一次 `observe`（screenshot-only）
- 任意 side-effect action 执行成功后，`AgentRunner` 自动再执行一次 `observe`（screenshot-only）
- 只读工具执行后，不强制 observe

这样做的目标是：

- 让 planner 每轮天然拥有 fresh visual
- 避免模型忘记 observe 导致的错误推理
- 让 loop 更贴近 `agent_v3`

## 当前阶段 loop

建议 loop：

1. `AgentRunner::run(...)`
2. 建立 runtime `Session`
3. 初始化 `LoopState`
4. 自动执行一次 `observe`（screenshot-only）
5. 进入 step loop
6. 每轮：
   - 基于 `LoopState` 增量构造 `PlannerContext`
   - 使用 `VisualContextLoader` 组装前后截图图片输入
   - 使用 `PlannerRenderer` 生成模型请求
   - 调模型
   - parse + validate
   - 若 `CallTool`：
     - 记录 `ToolCall` 到内存 journal
     - 执行工具
     - 记录 `ToolResult` 到内存 journal
     - 仅将紧凑摘要写入 `ModelContextBuffer`
     - 更新 `LoopState`
     - 若工具有 side effect 且执行成功：
       - 自动执行一次 `observe`（screenshot-only）
       - 更新 `visual_window`
   - 若 `Finish`：
     - 调 `FinishGate`
   - 若 `Fail`：
     - 标记失败并退出
   - 每轮结束后 flush 一次 session journal
7. run 结束或失败时执行最终 flush

## 与现有 `SessionStore` 的关系

当前阶段仍复用 runtime 的 `SessionStore` / `SessionEvent`，但 flush 策略调整为：

- turn-end flush
- run-end flush
- fail-fast 退出前强制 flush

## 配置设计

```toml
[agent]
default_model   = "gpt-5.4"
max_steps       = 40
step_timeout_ms = 30000
planner_format  = "json"
visual_window_size = 2
auto_observe_initial = true
auto_observe_after_side_effect = true
session_flush_policy = "turn_end"
```

## 错误处理与停止条件

当前阶段停止条件：

- `Finish`
- `Fail`
- 超过 `max_steps`
- 单步模型超时
- 单步工具执行超时
- 不可恢复的 parser / validator 错误

恢复策略：

- parse 失败注入 planner feedback 并重试
- 工具错误默认反馈给 planner
- 连续相同错误超过阈值时强制失败退出

## 测试策略

### 单元测试

- `ModelContextBuffer`
- `PlannerRenderer`
- `VisualContextLoader`
- `LoopStateContextManager`
- `ObservationCache`
  - 视觉窗口固定上限为 2
- `SessionJournal`
  - turn-end flush
  - run-end flush

### 集成测试

基于 `operator-testkit` 的 `MockPlatformDriver`：

- 启动后自动 screenshot-only observe
- side-effect action 后自动 screenshot-only observe
- 前后两张截图被组装为模型图片输入
- planner prompt 不再包含 pretty JSON planner request
- 模型上下文只消费紧凑 history 与工具结果摘要
- 热路径 observe 默认 `include_elements = false`

### 人工验证

需要增加 human-assisted 验证来确认：

- `gpt-5.4` 和 `doubao-seed` 都能稳定消费前后截图
- prompt 改写后 step latency 明显下降
- audit transcript 与模型上下文分层后，完成率不回退

## 推荐的 crate 内部分层

```text
crates/operator-agent/
  src/
    lib.rs
    config.rs
    error.rs
    runner.rs
    session.rs
    journal.rs
    planner/
      mod.rs
      prompts.rs
      context.rs         # LoopStateContextManager
      renderer.rs        # PlannerRenderer
      parser.rs
      validator.rs
      finish_gate.rs
    model/
      mod.rs
      types.rs
      provider.rs
      event.rs
      registry.rs
      openai.rs
      doubao.rs
      visual_loader.rs   # VisualContextLoader
    tools/
      mod.rs
      executor.rs
      catalog.rs
      observe_cache.rs
      kv_store.rs
```

## 实施建议

建议拆分顺序：

1. `ModelContextBuffer` 与 `SessionJournal` 分层
2. `VisualContextLoader`，让前后截图真正进入模型
3. `PlannerRenderer`，改成任务语义渲染
4. `PlannerToolSummary` 与 compact history / tool result summaries
5. 集成测试与 human-assisted 验证

## 最终结论

当前阶段的 `operator-agent` loop 应采用一条更接近 `agent_v3` 的热路径：

- 状态保留在内存
- 视觉由流程自动刷新
- 前后截图真正进入模型
- prompt 采用任务语义渲染
- 审计日志与模型上下文严格分层
- AX tree 不进入默认热路径
- session 持久化改为边界 flush

这样既能保持 Operator 现有的多平台、多模型和 typed tool 边界，也能显著降低当前 loop 的固定等待成本。
