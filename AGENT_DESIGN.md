# Operator Agent Module Design

日期：2026-03-24

## 目的

本文档定义 `operator-agent` 的总体设计。

第一期目标是提供一个**本地单 session、单 target、单 agent loop** 的执行器，并在统一 `operator` CLI 中提供自然语言入口：

- 支持多平台运行时
- 支持多模型接入
- Agent 内部不通过 CLI 调用操作
- 直接复用 `operator-runtime` 的 `ToolRegistry`

本文档不实现 A2A northbound protocol，也不引入多会话调度。

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
- Agent 直接调用内建工具，不经由 CLI tool bridge，也不经由 MCP
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

## 总体架构

`operator-agent` 采用 **独立 crate + 内建工具直调** 的结构。

推荐结构：

```text
operator-agent
  ├── AgentRunner
  ├── AgentSessionState
  ├── ModelRegistry / ModelProvider
  ├── Planner
  ├── DecisionParser / DecisionValidator
  ├── TaskReflector
  ├── ToolExecutor
  ├── ContextAssembler
  └── AgentConfig
```

核心原则：

- Agent 是 entry 层，不是 runtime 内核的一部分
- Tool execution 统一走 `ToolRegistry`
- 模型只负责“生成下一步决策”，不直接接触平台或 CLI
- 平台差异通过已有 `CapabilitySet + ToolCatalog` 下沉

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
- 不支持 pause/resume
- 不支持 northbound `input_required`

第一期 public CLI 入口：

- `operator agent <task>`
- `--model <gpt-5.4|doubao-seed>`
- `--max-steps <n>`
- `--json`
- `--target <target>`
- `--timeout-ms <ms>`

说明：

- 这里的 CLI 仅作为 northbound 入口来承载任务文本、模型选择和输出格式
- Agent 内部执行工具时仍直接调用 `ToolRegistry`
- 因此“Agent 不通过 CLI 调用操作”仍然成立；CLI 不参与工具选择和工具执行链

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
    pub parse_attempts: u32,             // 当前 step 的 parse retry 计数
    pub messages: Vec<AgentMessage>,
    pub tool_trace: Vec<ToolTraceEntry>,
    pub notes: Vec<String>,              // 由 TaskReflector 写入，任务开始时清空
    pub latest_snapshot: Option<SnapshotId>,
    pub previous_snapshot_visual: Option<ArtifactId>, // 上一轮截图，供前后对比
    pub latest_artifacts: Vec<ArtifactId>,
    pub ui_state_stale: bool,
    pub consecutive_error_count: u32,    // 连续相同错误计数
    pub last_error_fingerprint: Option<String>, // 最近错误的指纹（tool_name + error_kind）
}
```

说明：

- `messages`：planner 可消费的对话/决策历史
- `tool_trace`：结构化工具执行历史
- `latest_snapshot` / `latest_artifacts`：保存最近观察结果的引用
- `previous_snapshot_visual`：上一轮截图引用，由 ContextAssembler 注入 `PlannerContext.previous_visual_input`，每次成功 observe 后更新
- `ui_state_stale`：标记界面是否因动作而失真
- `consecutive_error_count` / `last_error_fingerprint`：用于检测连续相同错误并触发终止

### 3. 模型抽象与封装

模型层直接参考 `/Users/gokwok/code/work/kernel_agent/crates/base` 的分层方式，但在 `operator-agent` 内按当前需求做裁剪。

推荐沿用它的三层结构：

- `model::types`
- `model::provider`
- `model::event`

其中最值得直接参考的边界是：

- `types`
  - `ModelId`
  - `ProviderKind`
  - `ModelConfig`
  - `Context`
  - `Message`
  - `ContentBlock`
  - `ToolSpec`
  - `Usage`
  - `CallOptions`
- `provider`
  - `ModelRequest`
  - `ModelError`
  - `ModelProvider`
- `event`
  - `ModelEvent`
  - `ModelStream`

第一期支持两个模型名：

- `gpt-5.4`
- `doubao-seed`

在 Operator 中，planner 不直接依赖具体 provider，而是只依赖统一的 `Context` / `Message` / `ContentBlock`。

建议模型模块内部结构如下：

```text
operator-agent::model
  ├── types.rs
  ├── provider.rs
  ├── event.rs
  ├── registry.rs
  ├── openai.rs
  └── doubao.rs
```

其中：

- `gpt-5.4` 对应 `ProviderKind::OpenAi`
- `doubao-seed` 优先按 OpenAI-compatible 方式适配，放在 `doubao.rs`

建议保留统一 provider 接口：

```rust
pub trait ModelProvider: Send + Sync + 'static {
    fn stream(&self, req: ModelRequest) -> ModelStream;
}
```

`ModelRegistry` 负责把逻辑模型名解析成 `ModelConfig + Provider`：

```rust
pub struct ModelRegistry {
    configs: HashMap<Arc<str>, ModelConfig>,
    providers: HashMap<ProviderKind, Arc<dyn ModelProvider>>,
}
```

`ModelRequest` 和 `Context` 也建议直接参考 `kernel_agent/base` 的结构：

```rust
pub struct ModelRequest {
    pub config: ModelConfig,
    pub context: Context,
    pub options: CallOptions,
    pub stream: bool,
    pub timeout: Option<Duration>,
    pub request_id: Option<Arc<str>>,
    pub max_retry_delay_ms: Option<u64>,
}
```

```rust
pub struct Context {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
}
```

这样做的意义是：

- 保持模型层与业务层解耦
- 未来若需要 streaming、thinking、tool call delta，不必推翻抽象
- `gpt-5.4` 和 `doubao-seed` 可共享同一套 message/tool context 结构

第一期真正启用的能力可收敛为：

- 文本输出
- 可选视觉输入
- 可选 JSON mode
- 非 streaming 主流程

即使主流程先不消费 `ModelStream`，也建议保留 `event` 分层，为后续 A2A/streaming 做前向兼容。

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

说明：

- `thought` 是模型的推理痕迹，不执行但保留在 session transcript 中，便于 debug

设计要求：

- 默认要求模型输出 JSON object
- 若 provider 支持 schema/json mode，则走约束输出
- 若 provider 输出格式漂移，先做一次就地恢复解析；恢复失败则走 **parse retry 回路**

**Parse retry 回路：**

parse 失败或 validator 校验不通过后，不直接终止，而是将失败原因注入上下文，退回 planner 重新生成决策，最多重试 `MAX_PARSE_ATTEMPTS`（建议 3）次。超过上限仍失败则标记 `Fail`。

```text
planner -> parser/validator
  若失败 && attempts < MAX_PARSE_ATTEMPTS:
    构造错误反馈 message -> 重新进入 planner
  若失败 && attempts >= MAX_PARSE_ATTEMPTS:
    -> Fail
```

validator 负责检查：

- 工具名是否存在
- 参数是否符合 schema
- 是否调用了当前 target 不支持的工具
- 是否违反 side-effect 策略

这部分借鉴 `agent_v3` 的 parser/validator/retry 思路，但不绑定其文本格式。

### 6. `TaskReflector`

**这是防止模型"假完成"的关键机制。**

当 planner 输出 `Finish` 决策时，不直接退出 loop，而是先经过 `TaskReflector` 做闭环验证：

- 将完整 session transcript、原始 task 和 finish 摘要传给模型
- 模型返回 `OK` 或 `NOT_OK`，以及原因

若验证结果为 `NOT_OK`：

- 回滚本次 `Finish` 决策（不计入最终结果）
- 将失败原因追加到 `AgentSessionState.notes`
- 重新进入 planner，利用 notes 引导下一步

若验证结果为 `OK`：

- 正式退出 loop，输出最终结果

```text
planner -> Finish
  -> TaskReflector
    -> OK  -> 退出 loop，输出结果
    -> NOT_OK -> 追加 note -> 重新进入 planner
```

`notes` 字段生命周期规则：

- **写入时机**：仅在 `TaskReflector` 判定 `NOT_OK` 时写入失败原因
- **清空时机**：任务开始时（新任务 bootstrap 阶段）清空；同一任务内 notes 持续累积
- **用途**：注入 planner context，作为"上次失败的教训"引导后续决策

### 7. `ToolExecutor`

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

- 公共层直接采用 `Context` / `Message` / `ContentBlock` / `ToolSpec`
- provider 差异只放在 `ModelProvider` 实现层
- `ModelRegistry` 负责逻辑模型名到 provider config 的解析
- planner 只消费统一的 `Context`，不依赖具体 SDK
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
    pub previous_visual_input: Option<ModelImageInput>,  // 上一轮截图，用于前后对比
    pub notes: Vec<String>,
}
```

说明：

- `previous_visual_input`：上一步动作执行前的截图（或上一轮 observe 的截图）。模型同时持有前后两张截图，可感知"操作是否生效"，避免重复操作或误判。来源是 `AgentSessionState.previous_snapshot_visual`。
- `recent_tool_results` 的窗口建议保留最近 **5 轮**。

**Long context KV store：**

工具返回结果可能包含大量文本（如 AX tree dump、长列表），若直接注入 prompt 会迅速超出上下文窗口。引入进程内 KV store 管理长内容：

- 工具结果长度超过阈值（建议 400 字符）时，将内容存入 KV store，生成 handle token（格式：`[KV:session:key]`）
- `ToolResultSummary` 中只保留 handle token + 简短摘要，不保留原始内容
- planner prompt 中引用 handle token
- 执行工具时，executor 在组装参数前先展开 token（若参数中含有 handle 引用）
- KV store 按 session 隔离，任务结束时释放

## `ui_state_stale` 规则

动作工具执行后，UI 可能已改变。为避免 planner 对过时 snapshot 继续推理，引入 stale 机制。

**当前设计（soft mechanism）：**

- 任何有副作用的 action 执行成功后，`ui_state_stale = true`
- ContextAssembler 将 stale 状态注入 prompt，提示 planner 应优先调用 `observe`
- `observe` 成功后，`ui_state_stale = false`，同时更新 `previous_snapshot_visual`

**已知局限：** 这是 prompt 约定，模型可能忽略 stale 提示而直接采取 action，导致基于过时界面推理。

**备选强制策略（后续迭代可考虑）：**

action 执行成功后，AgentRunner **强制插入一次 observe**，再进入下一轮 planner，不依赖模型自觉。代价是增加每轮工具调用开销，适合高精度要求场景。

第一期采用 soft 策略，同时在 planner prompt 中明确要求："每次 action 执行后必须先 observe 再做下一步决策"，以 prompt 约束替代强制路由。

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
   - 组装 planner context（含前后截图、notes、tool results 摘要）
   - 调模型
   - parse + validate
     - 失败时：注入错误反馈，重新调模型（最多 `MAX_PARSE_ATTEMPTS` 次）
     - 超过上限：标记失败并退出
   - 若 `CallTool`：
     - 记录 `ToolCall`
     - 执行工具
     - 记录 `ToolResult`（长结果存 KV store，注入 handle token）
     - 更新 session state（`ui_state_stale`、`previous_snapshot_visual`、error 计数）
   - 若 `Finish`：
     - 调 `TaskReflector` 做闭环验证
     - 验证 OK：输出完成结果并退出
     - 验证 NOT_OK：追加 note，重置 step，重新进入 planner
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

当前 `DESIGN.md` 中遗留的旧模型配置需要替换为 agent 自己的模型配置分组。

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
reasoning_level = "medium"

[agent.models.doubao_seed]
provider    = "openai_compatible"
model       = "doubao-seed"
api_key_env = "DOUBAO_API_KEY"
base_url    = "..."
reasoning_level = "medium"
```

说明：

- 模型名是逻辑别名，不直接把 provider SDK 写进业务层
- 配置项语义尽量对齐 `kernel_agent/base` 的 `ModelConfig + CallOptions`
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

- parser 先做一次就地恢复（提取首个 JSON object）；恢复失败则进入 parse retry 回路（最多 `MAX_PARSE_ATTEMPTS = 3` 次，每次将错误描述注入 planner context）
- 工具错误默认反馈给 planner，允许 planner 决定是否换路径
- **连续相同错误阈值**：连续 3 次触发相同错误指纹（`tool_name + error_kind`）则强制失败退出，避免无效循环。错误指纹由 executor 在写入 `ToolResult` 时计算，与 `consecutive_error_count` 配合使用；执行成功时重置计数。

## 测试策略

### 单元测试

- `DecisionParser`：
  - 正常 JSON
  - 多余文本包裹
  - 非法工具名
  - parse retry 回路触发（3 次失败后 Fail）
- `ModelRegistry`：
  - `gpt-5.4`
  - `doubao-seed`
- `ContextAssembler`：
  - screenshot + previous screenshot 均存在
  - 无截图 fallback
  - stale / fresh UI state
  - notes 注入
- `KvStore`：
  - 短内容直接返回，不存 KV
  - 长内容存 KV，返回 handle token
  - handle token 正确展开
- `TaskReflector`：
  - OK → 退出 loop
  - NOT_OK → 追加 note，重新进入 planner

### 集成测试

基于 `operator-testkit` 的 `MockPlatformDriver`：

- 单轮 query -> finish（TaskReflector OK）
- observe -> click -> observe -> finish
- finish NOT_OK -> 重新规划 -> finish OK
- action 触发 `ui_state_stale`
- parse 失败 3 次后任务 Fail
- 连续相同错误 3 次后任务 Fail
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
      context.rs        # ContextAssembler
      parser.rs
      validator.rs
      reflector.rs      # TaskReflector
    model/
      mod.rs
      types.rs
      provider.rs
      event.rs
      registry.rs
      openai.rs
      doubao.rs
    tools/
      mod.rs
      executor.rs
      catalog.rs
      kv_store.rs       # Long context KV store（handle token 管理）
```

## 实施建议

建议把第一期拆成以下顺序：

1. `operator-agent` crate skeleton
2. 模型抽象层（对齐 `kernel_agent/base` 的 `types/provider/event`）
3. `ToolExecutor` 直接接 `ToolRegistry`
4. `AgentSessionState` 与基础 loop
5. planner contract / parser / validator（含 parse retry 回路）
6. `ContextAssembler`（含前后截图、KV store handle 注入）
7. `TaskReflector`（completion verification）
8. `gpt-5.4` adapter
9. `doubao-seed` adapter
10. session event / transcript / integration tests

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

核心可靠性机制（第一期必须落地）：

- `TaskReflector`：防止模型假完成
- Parse retry 回路：防止格式漂移导致任务失败
- Long context KV store：防止 prompt 膨胀
- 前后截图对比：支撑 GUI 动作感知
- 连续错误指纹检测：防止无效死循环

不做：

- CLI 工具桥
- MCP 工具桥
- A2A northbound
- 多会话调度

这样可以在不破坏当前 `Operator` 内核分层的前提下，把 Agent 能力平滑接进现有架构。
