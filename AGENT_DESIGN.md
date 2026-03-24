# Operator Agent Module Design

日期：2026-03-24

## 目的

本文档定义 `operator-agent` 的总体设计。

第一期目标是提供一个**本地单 session、单 target、单 agent loop** 的执行器：

- 支持多平台运行时
- 支持多模型接入
- 不通过 CLI 调用操作
- 直接复用 `operator-runtime` 的 `ToolRegistry`

本文档不实现 A2A，也不引入多会话调度。

## 设计输入

本设计基于两类现状：

- 当前 Operator 已具备：
  - `operator-core` 的 typed 领域模型
  - `operator-runtime` 的 `RuntimeCore` / `ToolRegistry`
  - `operator-platform-macos` 的平台实现
  - `operator mcp serve` 的统一协议入口
- 参考 `operation_agent/agent_v3` 的实现经验：
  - 模型规划循环在进程内
  - 工具执行走统一工具通道
  - planner / parser / validator / executor 职责分离
  - 会话状态和长上下文由 agent 自己持有，而不是由 northbound 协议拼装

## 设计目标

- `operator-agent` 作为独立 crate，不让 runtime 反向依赖模型/provider
- Agent 直接调用内建工具，不经过 CLI，也不经由 MCP
- Agent 不感知具体平台实现，只依赖 `Target`、`CapabilitySet` 和工具 schema
- Agent 不依赖某个模型厂商的 tool-calling 能力
- `gpt-5.4` 和 `doubao-seed` 使用统一的 planner contract
- 未来可自然扩展到 A2A / pause-resume / 多会话调度

## 非目标

- 第一期开 A2A northbound surface
- 第一期开多 session scheduler
- 通过 CLI 作为 agent 的工具桥
- 让 runtime 内核依赖模型/provider SDK
- 为每个模型单独设计 planner 协议

## 总体结论

采用 **独立 `operator-agent` crate + 内建工具直调** 的方案。

推荐结构：

```text
operator-agent
  ├── AgentRunner
  ├── AgentSessionState
  ├── ModelRegistry / ModelClient
  ├── Planner
  ├── DecisionParser / DecisionValidator
  ├── ToolExecutor
  ├── PromptAssembler
  └── AgentConfig
```

核心原则：

- Agent 是 entry 层，不是 runtime 内核的一部分
- Tool execution 统一走 `ToolRegistry`
- 模型只负责“生成下一步决策”，不直接接触平台或 CLI
- 平台差异通过已有 `CapabilitySet + ToolCatalog` 下沉

## 方案比较

### 方案 1：独立 `operator-agent` crate，直接调用 `ToolRegistry`

优点：

- 与当前 `operator-core` / `operator-runtime` 分层最一致
- 不走 CLI，不走 MCP，执行链最短
- 工具 schema、side-effect gate、capability check、verification 全部沿用 runtime 现有能力
- 后续 A2A 只需在外层加协议适配，不需要重写 agent core

缺点：

- 需要自己定义 planner output contract
- 需要自己实现 provider adapter 和 session state

### 方案 2：复刻 `agent_v3` 的 LangGraph 风格状态机

优点：

- planner / validator / executor 边界显式
- 容易映射已有参考实现

缺点：

- 会把 Thinkflow / LangGraph 形态一并带入 Rust 代码库
- 前期复杂度偏高
- 不符合当前 Operator “少量清晰 crate”的约束

### 方案 3：完全依赖 provider-native tool calling

优点：

- 最少的 agent orchestration 代码

缺点：

- `gpt-5.4` 和 `doubao-seed` 的行为不一定一致
- 很难维持跨模型稳定 contract
- debug、回放、回归测试都更难

结论：**采用方案 1**。

## 与 `operation_agent/agent_v3` 的关系

要复用的是设计模式，不是技术栈。

保留的思想：

- planner / parser / validator / executor 分层
- 工具执行是统一子系统
- 长文本和截图引用不直接堆进 prompt
- 会话状态在 agent 内聚

不引入的东西：

- Thinkflow runtime 依赖
- A2A ingress 约束
- LangGraph 本身
- 通过 MCP 再转一次工具调用

在 `agent_v3` 中，executor 通过 `tools/call` 走远程工具端点；在 Operator 中，这一层改成**直接调用本地 `ToolRegistry`**。

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

设计原因：

- `operator-runtime` 应继续保持“无模型依赖”的纯执行内核
- `operator-agent` 只消费 runtime 暴露的执行能力
- CLI / MCP / Agent 都是 runtime 的不同 entry surface

## 第一期开箱能力

第一期 `operator-agent` 提供一个库级本地执行器：

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
- 不支持 pause/resume
- 不支持 northbound `input_required`

## 核心组件

### 1. `AgentRunner`

对外唯一入口，负责：

- 建立 agent session
- 构造 planner 上下文
- 驱动 step loop
- 调用模型
- 调用工具
- 记录 session event
- 输出最终结果

建议接口：

```rust
pub struct AgentRunner {
    runtime: Arc<Runtime>,
    models: ModelRegistry,
    prompts: PromptSet,
    config: AgentConfig,
}

impl AgentRunner {
    pub async fn run(&self, req: AgentRunRequest) -> Result<AgentRunResult, AgentError>;
}
```

### 2. `AgentSessionState`

单次任务的内存状态，不等同于 runtime 的 `Session` 持久化结构。

建议字段：

```rust
pub struct AgentSessionState {
    pub session_id: SessionId,
    pub target: TargetId,
    pub task: String,
    pub status: AgentSessionStatus,
    pub turn_index: u32,
    pub step_index: u32,
    pub messages: Vec<AgentMessage>,
    pub tool_trace: Vec<ToolTraceEntry>,
    pub notes: Vec<String>,
    pub latest_snapshot: Option<SnapshotId>,
    pub latest_artifacts: Vec<ArtifactId>,
    pub ui_state_stale: bool,
}
```

说明：

- `messages` 是 planner 可消费的对话/决策历史
- `tool_trace` 是结构化工具执行历史
- `latest_snapshot` / `latest_artifacts` 保存最近观察结果的引用
- `ui_state_stale` 用于标记界面是否因动作而失真

### 3. `ModelRegistry` 与 `ModelClient`

第一期支持两个模型名：

- `gpt-5.4`
- `doubao-seed`

模型层不直接暴露 provider 概念给 planner。统一接口：

```rust
pub trait ModelClient: Send + Sync {
    fn profile(&self) -> &ModelProfile;

    async fn generate(
        &self,
        request: ModelRequest,
    ) -> Result<ModelResponse, AgentError>;
}
```

建议 profile：

```rust
pub struct ModelProfile {
    pub name: String,
    pub supports_vision: bool,
    pub supports_json_mode: bool,
    pub max_output_tokens: u32,
}
```

第一期实现：

- `OpenAIModelClient` for `gpt-5.4`
- `DoubaoModelClient` for `doubao-seed`

### 4. `Planner`

Planner 负责把这些输入组装给模型：

- 用户任务
- 当前 target 与 capability 摘要
- 可用工具目录
- 最近 snapshot / focus / app/window 状态摘要
- agent notes
- 最近若干轮工具结果

Planner 不直接生成执行结果，只生成**下一步决策**。

### 5. `DecisionParser` / `DecisionValidator`

第一期不使用 provider-native tool calling。

模型输出统一解析成：

```rust
pub enum AgentDecision {
    CallTool {
        name: String,
        arguments: serde_json::Value,
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

设计要求：

- 默认要求模型输出 JSON object
- 若 provider 支持 schema/json mode，则走约束输出
- 若 provider 输出格式漂移，parser 允许做一次恢复解析
- validator 负责检查：
  - 工具名是否存在
  - 参数是否符合 schema
  - 是否调用了当前 target 不支持的工具
  - 是否违反 side-effect 策略

这部分借鉴 `agent_v3` 的 parser/validator 思路，但不绑定其文本格式。

### 6. `ToolExecutor`

这是第一期最关键的组件。

设计要求：

- 不通过 CLI
- 不通过 MCP
- 直接调用 `ToolRegistry`

建议接口：

```rust
pub struct ToolExecutor {
    runtime: Arc<Runtime>,
}

impl ToolExecutor {
    pub fn catalog(&self, target: &TargetId) -> Result<Vec<AgentToolSpec>, AgentError>;

    pub async fn call(
        &self,
        session_id: &SessionId,
        target: &TargetId,
        name: &str,
        arguments: serde_json::Value,
        timeout_ms: Option<u64>,
    ) -> Result<AgentToolResult, AgentError>;
}
```

调用路径：

```text
AgentRunner
  -> ToolExecutor
    -> ToolRegistry.invoke(name, json)
      -> ToolHandler
        -> RuntimeCore.observe/query/act
          -> PlatformDriver
```

这保证：

- CLI / MCP / Agent 复用同一份工具语义
- capability check、verification、audit 逻辑不重复
- 平台 driver 不需要为 agent 再开一套 API

## 工具目录设计

Agent 内部直接使用 runtime 的 flat tool names。

第一期不把 grouped CLI command surface 暴露给 agent。对 agent 来说，更合适的是稳定的内部工具目录，例如：

- `observe`
- `snapshot-get`
- `artifact-get`
- `get-focus`
- `list-apps`
- `list-windows`
- `permissions-status`
- `capabilities`
- 所有 action 工具

对模型暴露的工具目录项应包含：

```rust
pub struct AgentToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub read_only: bool,
}
```

目录生成规则：

- 基于 `ToolRegistry`
- 再叠加 target capability 过滤
- 再叠加安全策略过滤

这样 agent 从第一步就只会看到**当前 target 可执行**的工具。

## 多平台支持策略

Agent 模块不认识 macOS / Windows / Harmony 的实现细节。

它只依赖这些平台无关输入：

- `TargetDescriptor`
- `CapabilitySet`
- `ToolCatalog`
- 工具返回结果

多平台适配规则：

- 平台差异继续由 `PlatformDriver` 承担
- tool availability 由 `ToolExecutor::catalog()` 动态裁剪
- prompt 中只描述“当前 target 支持的能力与工具”，而不内置平台特化 prompt

这使得：

- 将来接入 `operator-platform-windows`
- 或 `operator-platform-harmony`

agent core 不必改架构，只需换 target 和 driver。

## 多模型支持策略

第一期只保留两个逻辑模型名：

- `gpt-5.4`
- `doubao-seed`

Agent 使用统一 planner contract，不做模型分叉工作流。

策略如下：

- 公共层：
  - `ModelRequest`
  - `ModelResponse`
  - `ModelProfile`
- provider 差异只放在 adapter
- parser/validator 兜底跨模型输出一致性

具体要求：

- 若模型支持 JSON schema，则启用严格结构化输出
- 若模型只支持普通文本，则要求输出首个 JSON object
- 若模型支持视觉输入，则可附带最近 screenshot artifact
- 若模型不支持视觉，则只附文本摘要和 snapshot 元数据

## 观察与上下文装配

Operator 是 snapshot-first 设计，Agent 必须体现这一点。

第一期上下文装配策略：

- 最近 snapshot 只保留：
  - `snapshot_id`
  - `surface`
  - `root element count`
  - 若存在则附最新 screenshot artifact 引用
- 不把完整 AX 树直接注入 prompt
- 对长文本工具结果只保留摘要 + 引用

建议新增 `ContextAssembler`：

```rust
pub struct PlannerContext {
    pub task: String,
    pub target_summary: String,
    pub tools: Vec<AgentToolSpec>,
    pub recent_messages: Vec<AgentMessage>,
    pub recent_tool_results: Vec<ToolResultSummary>,
    pub latest_snapshot: Option<SnapshotSummary>,
    pub latest_visual_input: Option<ModelImageInput>,
}
```

## `ui_state_stale` 规则

动作工具执行后，UI 可能已改变。

为避免 planner 对过时 snapshot 继续推理：

- 任何有副作用的 action 执行成功后，`ui_state_stale = true`
- 当 planner 看到：
  - `ui_state_stale = true`
  - 且接下来需要基于界面定位
  - 应优先调用 `observe`
- `observe` 成功后，`ui_state_stale = false`

这条规则能把 `Operator` 的 snapshot-first 哲学真正落到 agent loop。

## 第一期开机流程

建议 loop：

1. `AgentRunner::run(...)`
2. 建立 runtime `Session`
3. bootstrap：
   - `capabilities`
   - `permissions-status`
   - 可选 `get-focus`
4. 进入 step loop
5. 每轮：
   - 组装 planner context
   - 调模型
   - parse + validate
   - 若 `CallTool`：
     - 记录 `ToolCall`
     - 执行工具
     - 记录 `ToolResult`
     - 更新 session state
   - 若 `Finish`：
     - 输出完成结果并退出
   - 若 `Fail`：
     - 标记失败并退出
6. 超过 `max_steps` 或超时则失败

## 与现有 `SessionStore` 的关系

第一期不重新设计 runtime session persistence。

复用现有 `SessionStore` / `SessionEvent` 做：

- 会话创建
- `UserInput`
- `ToolCall`
- `ToolResult`
- `ModelResponse`
- `Completed` / `Error`

但 `AgentSessionState` 仍是 agent 内部状态对象。

理由：

- runtime 现有 `Session` 足以承载审计/回放最小信息
- planner memory、latest snapshot、notes 这些 agent 内部状态不适合强塞回 runtime 通用 session 结构

## 配置设计

当前 `DESIGN.md` 中的旧配置项：

- `model.anthropic`
- `agent.model = "claude-opus-4-6"`

都应废弃。

建议改成：

```toml
[agent]
default_model   = "gpt-5.4"
max_steps       = 40
step_timeout_ms = 30000
planner_format  = "json"

[agent.models.gpt_5_4]
provider    = "openai"
model       = "gpt-5.4"
api_key_env = "OPENAI_API_KEY"
base_url    = "https://api.openai.com/v1"

[agent.models.doubao_seed]
provider    = "doubao"
model       = "doubao-seed"
api_key_env = "DOUBAO_API_KEY"
base_url    = ""
```

说明：

- 模型名是逻辑别名，不直接把 provider SDK 写进业务层
- 具体 provider 需要的额外参数放在 adapter 内部消费

## 错误处理与停止条件

第一期停止条件：

- `Finish`
- `Fail`
- 超过 `max_steps`
- 单步模型超时
- 单步工具执行超时
- 不可恢复的 parser / validator 错误

恢复策略：

- parser 允许一次恢复提取 JSON object
- 工具错误默认反馈给 planner，允许 planner 决定是否换路径
- 连续相同错误超过阈值则失败退出

## 测试策略

### 单元测试

- `DecisionParser`：
  - 正常 JSON
  - 多余文本包裹
  - 非法工具名
- `ModelRegistry`：
  - `gpt-5.4`
  - `doubao-seed`
- `ContextAssembler`：
  - screenshot / no screenshot
  - stale / fresh UI state

### 集成测试

基于 `operator-testkit` 的 `MockPlatformDriver`：

- 单轮 query -> finish
- observe -> click -> observe -> finish
- action 触发 `ui_state_stale`
- capability 不满足时工具目录正确裁剪

### 回归测试

- 工具 catalog 稳定性
- planner contract snapshot
- provider adapter JSON mode / fallback mode

## 未来扩展边界

第一期完成后，未来扩展顺序建议是：

1. pause / resume
2. 多 session 调度
3. A2A surface
4. 更细的 memory backend

关键原则：

- A2A 是 `operator-agent` 的**外层协议适配**
- 不是 Agent core 的前置条件
- 不应反向污染 `operator-runtime`

## 推荐的 crate 内部分层

```text
crates/operator-agent/
  src/
    lib.rs
    config.rs
    error.rs
    runner.rs
    session.rs
    planner/
      mod.rs
      prompts.rs
      context.rs
      parser.rs
      validator.rs
    model/
      mod.rs
      registry.rs
      openai.rs
      doubao.rs
    tools/
      mod.rs
      executor.rs
      catalog.rs
```

## 实施建议

建议把第一期拆成以下顺序：

1. `operator-agent` crate skeleton
2. `ModelClient` / `ModelRegistry`
3. `ToolExecutor` 直接接 `ToolRegistry`
4. `AgentSessionState` 与基础 loop
5. planner contract / parser / validator
6. `gpt-5.4` adapter
7. `doubao-seed` adapter
8. session event / transcript / integration tests

## 最终结论

`operator-agent` 应该是一个**独立 entry 层执行器**：

- 上接 `gpt-5.4` 与 `doubao-seed`
- 中间用模型无关的 planner contract
- 下接 `ToolRegistry`
- 通过 `RuntimeCore` 和 `PlatformDriver` 支撑多平台

第一期只做：

- 单 session
- 单 target
- 单 loop
- 本地执行

不做：

- CLI 工具桥
- MCP 工具桥
- A2A northbound
- 多会话调度

这样可以在不破坏当前 `Operator` 内核分层的前提下，把 Agent 能力平滑接进现有架构。
