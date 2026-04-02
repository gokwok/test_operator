# Operator Task Feature Design

日期：2026-04-02

## 1. 目的

本文档定义 Operator 中 `task` 特性的设计边界、数据模型、控制面、数据面和 northbound 交互方式。

`task` 的目标是把一次成功的 `operator-agent` 执行，沉淀为一个**可参数化、可重复、成功率高、执行期不依赖 LLM** 的 GUI 自动化资产，并允许用户通过统一 `operator` CLI 直接调用，例如：

- 新建备忘录
- 创建日历事件
- 在固定应用中填写表单
- 在稳定 GUI 流程中完成重复录入

本文档只覆盖以下两条能力线：

1. **确定性 task substrate**
   - task manifest / schema / store
   - task runner
   - anchor 重绑定
   - certify
2. **transcript -> task authoring**
   - deterministic scaffold
   - optional LLM-assisted compile / repair

本文档**不覆盖**：

- 把成熟 task 提升为 `ToolRegistry` 内的复合 tool
- 动态把 task 暴露给 agent / MCP 作为一级工具
- 在执行期让 LLM 参与 task 决策
- 多 session 调度、pause / resume、远程 orchestration

一句话：**agent 负责探索，task 负责固化；LLM 只负责离线生成和修补，runner 永远确定性。**

## 2. 设计输入

本设计基于以下已实现前提：

1. Operator 已具备统一 typed runtime 和 `ToolRegistry`。
2. CLI / MCP / Agent 已共享 runtime 和命名 target 体系。
3. `operator-agent` 已具备持久化 session transcript：
   - session metadata：`~/.operator/sessions/<id>.json`
   - replayable transcript：`~/.operator/sessions/<id>.jsonl`
4. transcript 已持久化 `UserInput` / `ToolCall` / `ToolResult` / `ModelResponse` / `Completed` / `Error`。
5. runtime 已具备：
   - `observe/query/action` typed 请求响应
   - `ToolRegistry.invoke()`
   - action verification
   - `Snapshot` / `Artifact` / `Locator`
6. agent 已具备：
   - screenshot 原生视觉输入
   - compact planner context
   - `include_elements` 录制能力

当前代码中的关键事实：

- compile 的事实输入应该来自 persisted session transcript，而不是 `ModelContextBuffer`
- 当前 replayable transcript schema 与 loader 物理上归属 `operator-agent::journal`；若采用本路线，应先迁移到 `operator-runtime` 成为共享基础设施
- 当前 `Locator::SnapshotElement` / `Snapshot*Coords` 绑定历史 `snapshot`
- selector locator 是否暴露给 planner，依赖当前 session 是否有 usable element observation
- 当前 agent 默认 `include_elements = false`；compile-friendly recording profile 必须显式开启
- runtime action 执行链已经统一，不需要再发明一套新执行协议
- 当前 `CLI_DESIGN.md` / `docs/COMMAND.md` 尚未纳入 `operator task`；本设计落地时必须同步两份 CLI authority 文档

## 3. 设计目标

- 定义 `task` feature line 的 northbound shell surface，并要求落地时与 CLI authority 文档同步
- 让 task 执行期完全脱离 LLM，降低成本、波动和延迟
- 让 agent 成功 run 可以沉淀为可复用资产，而不是一次性 transcript
- 保持与现有 `operator-runtime` / `ToolRegistry` / `SessionStore` 的边界一致
- 用 task manifest 明确参数、步骤、锚点、守卫和认证结果
- 让 scaffold / compile / repair / certify 成为明确的控制面生命周期
- 优先追求高成功率，而不是一次性全自动生成率

## 4. 非目标

- 不为每个 task 生成动态 clap flag，例如 `operator task run notes.create --title ...`
- 不在第一阶段改变 runtime 的公共 tool schema
- 不在第一阶段修改 `operator-core` 的 `Action` / `Locator` 公共模型来直接容纳 task 语义
- 不要求所有 task 都可以纯文本无视觉地运行
- 不保证 transcript compile 零人工 review
- 不把 task 设计成“录屏回放器”或“坐标宏”

这里尤其需要明确两点：

1. **不做动态 task-specific CLI flags**
   - 当前 CLI 是静态 clap 命令树
   - 动态 `--title` / `--content` 会引入 help、补全、冲突和稳定 shell contract 问题
   - 第一阶段只支持稳定参数输入面，例如 `--set` / `--set-file` / `--params-json`
2. **不复用历史 snapshot locator**
   - 历史 `SnapshotElement` 不能跨会话直接执行
   - task 必须引入自己的 `AnchorSpec`

## 5. 核心概念

### 5.1 Task

Task 是一个版本化的自动化资产，定义：

- 任务标识与元数据
- 参数 schema
- 执行步骤
- 脚本级锚点
- 守卫与重试策略
- compile provenance
- certification 结果

### 5.2 Task Manifest

Task 的权威定义文件。建议使用 TOML。

### 5.3 Scaffold

只使用 deterministic transcript 提取与启发式规则生成的 task 草稿，不依赖 LLM。

### 5.4 Compile

在 scaffold / compile IR 基础上，允许使用 LLM 做参数抽象、锚点综合、守卫补全和 manifest 生成。

### 5.5 Repair

在已有 task 基础上，基于失败 session 生成**局部补丁**，只允许修 anchor / guard / retry / assert 等局部区域，不做整份 task 重生成。

### 5.6 Certify

对 task 在一组 case 上执行重复运行，生成可审计报告和认证状态。

### 5.7 Anchor

Anchor 是 task 级别的跨会话元素定位描述，不直接引用历史 `snapshot_id` / `element_id`。

执行时：

1. 先 fresh observe
2. 用 anchor 在当前 observation 中重绑定
3. 重绑定成功后，再转换成当次运行的 `Locator`

### 5.8 Step Output Binding

Step 执行后，runner 会将 tool result 存入一个 step-scoped binding map。后续 step 可以通过 `${steps.<step_id>.result.<leaf_path>}` 语法引用前序 step 的输出。

引用规则：

- 第一阶段只支持 `${steps.<step_id>.result.<leaf_path>}`
- `<leaf_path>` 使用点号分隔的简单路径
- 路径必须落到标量叶节点（string / number / bool）
- 不支持 `${steps.<step_id>.result}` 整个对象绑定
- 不支持引用数组或对象
- 只允许引用已执行的前序 step，不允许前向引用
- 引用未执行、路径不存在或值不是标量，统一视为 `parameter_error`

## 6. 数据面与控制面

## 6.1 数据面

Task 数据面指真正执行 GUI 自动化的热路径：

```text
task manifest
  + runtime target
  + param bindings
  + current observations
  -> TaskRunner
  -> AnchorResolver
  -> ToolRegistry.invoke()
  -> RuntimeCore
  -> PlatformDriver
```

数据面职责：

- 参数绑定
- step 调度
- fresh observe
- anchor 重绑定
- deterministic tool invoke
- postcondition / verification
- retry / fallback
- 写入 session transcript 与 run report

数据面原则：

- 不调用模型
- 不读取 planner context
- 不回放 `ModelResponse`
- 不依赖“上一次成功运行的 snapshot_id”

数据面输入：

- task manifest
- 运行参数
- 命名 target
- 当前桌面 / 设备状态

数据面输出：

- runtime session
- step 级工具调用与结果
- 任务级 run report
- 可选 certify case 结果

## 6.2 控制面

Task 控制面指 task 资产的生产、修订、认证和治理链路：

```text
persisted sessions
  + compile IR
  + optional model
  -> scaffold / compile / repair / certify
  -> task store
  -> reports
```

控制面职责：

- 读取 persisted session transcript
- 生成 scaffold 或 compile IR
- 运行 LLM compile / repair pass
- 保存 task manifest
- 执行 certify
- 更新 draft / certified 等状态
- 管理 provenance 和报告

控制面允许使用模型，但只限离线 authoring：

- `task compile`
- `task repair`

控制面不负责：

- 在线做下一步 GUI 决策
- 在执行期兜底替代 runner

## 6.3 数据面 / 控制面边界

必须保持以下边界：

- runner 属于数据面，不依赖 model/provider
- compile / repair 属于控制面，可以依赖 model/provider
- task manifest 是数据面和控制面的共享契约，不是某一方私有格式
- certify 是控制面命令，但底层复用数据面 runner

## 7. 总体架构

本路线包含两部分结构调整：

1. 在 `operator-runtime` 中引入共享 transcript 基础设施
2. 引入 `crates/operator-task`

### 7.1 `operator-runtime` 侧新增共享 transcript 层

目标：

- 让 replayable transcript 成为 CLI / Agent / task 共用的运行时基础设施
- 让 deterministic scaffold 不再依赖 `operator-agent` 私有 loader
- 保持 transcript payload entry-neutral，而不是绑定 agent 私有结构

建议新增：

```text
crates/operator-runtime/
  src/
    transcript.rs
```

建议由 `operator-runtime::transcript` 持有：

- `ReplayableTranscriptEvent`
- `PersistedSessionTranscript`
- `PersistedToolCall`
- `PersistedToolResult`
- `load_persisted_transcript(...)`

约束：

- 共享 transcript 建立在现有 `SessionStore` / `FileSessionStore` 之上，不新增第二套 session 存储
- transcript schema 必须保持 entry-neutral，不直接暴露 `AgentToolResult` 这类 agent 私有类型
- task run 与 agent run 都应能通过同一组 runtime helper 读取 replayable transcript

### 7.2 `operator-task`

推荐引入一个新库 crate：

- `crates/operator-task`

职责：

- task manifest schema
- task store
- task runner
- anchor resolver
- deterministic scaffold heuristics
- compile IR
- certify
- run / compile / repair / certify reports
- task 侧 authoring contracts，例如 `TaskCompiler` / `TaskRepairEngine`

建议结构：

```text
crates/operator-task/
  src/
    lib.rs
    error.rs
    schema.rs
    store.rs
    runner.rs
    bind.rs
    anchor.rs
    scaffold.rs
    compile_ir.rs
    compiler.rs
    repair.rs
    certify.rs
    report.rs
```

同时定义一个**逻辑上的** task authoring subsystem：

- deterministic scaffold
- optional LLM-assisted compile
- patch-based repair

为了避免 runtime 反向依赖模型，建议采用以下物理布局：

1. `operator-task`
   - 不依赖模型/provider
   - 依赖 `operator-runtime::transcript`
   - 提供 manifest、shared IR types、runner、store、scaffold、certify、report
   - 定义 `TaskCompiler` / `TaskRepairEngine` trait 或等价 authoring interface
2. `operator-agent`
   - 复用现有 `ModelRegistry` / `ResolvedModel`
   - 实现 optional model-backed compile / repair backend
   - 输入 `CompileIr`，输出 `TaskManifestDraft` 或 `TaskPatch`

这样可以保持依赖方向：

```text
operator-cli
  ├── depends on operator-task
  ├── depends on operator-agent
  └── depends on operator-bootstrap / operator-runtime / operator-core

operator-task
  ├── depends on operator-core
  └── depends on operator-runtime

operator-agent
  ├── depends on operator-core
  ├── depends on operator-runtime
  └── may depend on operator-task (for compile traits / manifest / CompileIr / TaskPatch / reports)
```

关键点：

- `operator-runtime` 不依赖 `operator-task`
- `operator-runtime` 不依赖模型/provider
- `operator-runtime` 负责共享 transcript 基础设施，但不负责 task 资产生命周期
- deterministic scaffold / compile IR 归 `operator-task`
- model-backed compile / repair backend 归 `operator-agent`

## 8. 存储设计

Task 建议存放在 operator home 下，与 session / snapshot / artifact 并列：

```text
~/.operator/
  config.toml
  sessions/
  snapshots/
  artifacts/
  tasks/
    notes.create/
      task.toml
      reports/
        compile-20260402T013000Z.json
        certify-20260402T020500Z.json
      cases/
        smoke.toml
        regression-existing-note.toml
```

第一阶段不新增 `.operator/config.toml` 的 task 专属配置字段。

约束：

- task store 默认跟随 operator home
- compile / certify report 和 task 资产共同存放
- run 产生的 session 仍写入现有 `sessions/`

## 9. 数据模型

## 9.1 Task Manifest

建议最小 manifest 结构：

```toml
name = "notes.create"
version = 1
description = "Create a note in the Notes app"
status = "draft"

[provenance]
mode = "compiled"
source_sessions = ["agent-20260402-abc123"]
compiled_at = "2026-04-02T01:30:00Z"
model = "openai"

[target]
default = "default"
platform_hint = "macos"
app_hint = "Notes"

[[params]]
name = "title"
type = "string"
required = true
description = "Note title"

[[params]]
name = "content"
type = "multiline_string"
required = true
description = "Note body"

[[anchors]]
name = "title_field"
app_hint = "Notes"
role = "TextField"
label_contains = "Title"
fallbacks = ["text", "role", "normalized_bounds"]

[[anchors]]
name = "body_area"
app_hint = "Notes"
role = "TextArea"
index = 0
fallbacks = ["role", "normalized_bounds"]

[[steps]]
id = "launch_notes"
tool = "launch-app"
kind = "action"
args = { bundle_id_or_name = "Notes" }

[[steps]]
id = "observe_editor"
tool = "observe"
kind = "observe"
args = { surface = { kind = "Frontmost" }, include_screenshot = true, include_elements = true }

[[steps]]
id = "new_note"
tool = "hotkey"
kind = "action"
args = { keys = ["Meta", "N"] }

[[steps]]
id = "focus_title"
tool = "click"
kind = "action"
args = {}
verifications = ["Focus"]

[[steps.anchor_bindings]]
arg_path = "locator"
anchor = "title_field"

[[steps]]
id = "type_title"
tool = "type"
kind = "action"
args = { text = "${title}", clear_before = true }

[[steps]]
id = "focus_body"
tool = "click"
kind = "action"
args = {}

[[steps.anchor_bindings]]
arg_path = "locator"
anchor = "body_area"

[[steps]]
id = "type_body"
tool = "type"
kind = "action"
args = { text = "${content}", clear_before = true }
```

## 9.2 参数模型

第一阶段参数类型限制为：

- `string`
- `multiline_string`
- `int`
- `bool`
- `enum`

不在第一阶段引入：

- 任意表达式
- 条件模板语言
- 复杂嵌套对象
- 自定义脚本执行

参数字段建议：

- `name`
- `type`
- `required`
- `default`
- `description`
- `sensitive`
- `examples`

## 9.3 AnchorSpec

Anchor 需要表达跨会话重绑定线索，而不是历史 snapshot 引用。

建议字段：

```rust
pub struct AnchorSpec {
    pub name: String,
    pub app_hint: Option<String>,
    pub window_hint: Option<String>,
    pub role: Option<String>,
    pub label_contains: Option<String>,
    pub value_contains: Option<String>,
    pub text_contains: Option<String>,
    pub index: Option<usize>,
    pub bounds_hint_norm_1000: Option<RectNorm1000>,
    pub required: bool,
    pub fallbacks: Vec<AnchorFallback>,
}
```

Anchor 退化策略使用 `AnchorFallback` 枚举定义：

```rust
pub enum AnchorFallback {
    /// 用 role + label + value 综合匹配
    SemanticMatch,
    /// 退回纯文本匹配（label / value / text 任一包含目标串）
    TextMatch,
    /// 退回 role + index 定位（如"第 2 个 TextField"）
    RoleIndex,
    /// 退回归一化坐标区域匹配（bounds_hint_norm_1000 ± tolerance）
    NormalizedBounds { tolerance: u32 },
}
```

匹配流程：

1. **限定 scope**：按 `app_hint` / `window_hint` 筛选目标窗口
2. **fresh observe**：强制 `include_elements=true`
3. **语义打分**（SemanticMatch）：
   - `role` 精确匹配：+40 分
   - `label_contains` 子串命中：+30 分
   - `value_contains` 子串命中：+15 分
   - `bounds_hint_norm_1000` 在 tolerance 内：+15 分
   - 总分 ≥ 40 且为最高分者胜出；平局取 tree order 最先者
   - 低于 40 分视为未命中，进入 fallback 链
4. **按 `fallbacks` 顺序依次尝试**：
   - `TextMatch`：遍历 elements，`text_contains` 子串命中即选中
   - `RoleIndex`：按 role 过滤后取第 `index` 个
   - `NormalizedBounds`：将归一化坐标中心距离 ≤ tolerance 的元素视为命中
5. 所有 fallback 均未命中，且 `required = true`，则报 `anchor_unresolved`

默认禁止：

- 直接回放绝对坐标
- 直接复用历史 `SnapshotElement`

## 9.4 Task Step

对 `Observe` / `Action` 而言，task step 本质上是“对现有 runtime tool 的封装调用”；`Assert` 是 runner 内建 pseudo-step，不直接调用平台。

建议字段：

```rust
pub struct TaskStep {
    pub id: String,
    pub kind: StepKind,
    pub tool: Option<String>,
    pub args: serde_json::Value,
    pub anchor_bindings: Vec<AnchorBinding>,
    pub guard: Option<StepGuard>,
    pub retry: Option<RetryPolicy>,
    pub verifications: Vec<ActionVerification>,
    pub postconditions: Vec<PostCondition>,
    pub on_failure: OnFailure,
}

pub enum StepKind {
    Observe,
    Action,
    Assert,
}

pub struct AnchorBinding {
    /// 要把哪个 tool 参数位置替换成 fresh locator，例如 `locator` / `from` / `to`
    pub arg_path: String,
    /// 使用哪个 task-level anchor 做重绑定
    pub anchor: String,
}

pub struct StepGuard {
    /// 要求指定 app 在前台
    pub app_frontmost: Option<String>,
    /// 要求指定 anchor 在当前 observation 中可解析
    pub anchor_resolvable: Option<String>,
    /// 要求前序 step 的 output 某字段满足预期值
    pub step_output_eq: Option<StepOutputCheck>,
}

pub struct StepOutputCheck {
    pub step_id: String,
    pub path: String,
    pub expected: serde_json::Value,
}

pub enum PostCondition {
    /// 焦点已切换到指定 anchor
    FocusOn(String),
    /// 指定 anchor 存在于当前 observation
    AnchorExists(String),
    /// 自定义 step output 检查
    OutputCheck(StepOutputCheck),
}

pub struct RetryPolicy {
    pub max_retries: u32,
    pub delay_ms: u64,
    /// 重试前是否重新 observe
    pub re_observe: bool,
}

pub enum OnFailure {
    /// 终止整个 task（默认）
    Fail,
    /// 跳过当前 step 继续执行
    Skip,
    /// 记录警告但继续执行
    Warn,
}
```

TOML 表示示例：

```toml
[[steps]]
id = "focus_title"
tool = "click"
kind = "action"
args = {}
on_failure = "fail"
verifications = ["Focus"]

[[steps.anchor_bindings]]
arg_path = "locator"
anchor = "title_field"

[steps.guard]
app_frontmost = "Notes"
anchor_resolvable = "title_field"

[steps.retry]
max_retries = 2
delay_ms = 500
re_observe = true

[[steps.postconditions]]
type = "FocusOn"
anchor = "title_field"
```

约束：

- `Observe` / `Action` step 必须设置 `tool`；`Assert` step 必须省略 `tool`
- `tool` 名直接复用 runtime tool 名
- `args` 直接复用对应 runtime tool 的输入形状，但不写入 `target` / `session_id` / `timeout_ms` 这类 exec context 字段
- `anchor_bindings.arg_path` 指向一个 locator 槽位，例如 `locator` / `from` / `to`
- `target_selector` / `focus_policy` / 其他非 locator 的 tool-specific 参数，继续显式放在 `args`
- `verifications` 直接复用现有 `ActionVerification` 枚举，只对 action step 生效
- step 级参数替换只允许引用 task params（`${param_name}`）或前序 step 输出（`${steps.<step_id>.result.<path>}`）
- side-effect step 默认要求 fresh observe discipline
- `on_failure` 默认为 `Fail`

## 9.5 Compile IR

Compile IR 是控制面中间产物，不直接用于执行。

建议结构：

```rust
pub struct CompileIr {
    pub session_id: String,
    pub task_text: String,
    pub target: String,
    pub successful_path: Vec<CompiledStepCandidate>,
    pub observations: Vec<ObservationDigest>,
    pub literals: Vec<LiteralCandidate>,
    pub anchor_candidates: Vec<AnchorCandidate>,
}

/// 从 transcript 提取的单步候选
pub struct CompiledStepCandidate {
    /// 在 transcript 中的序号
    pub transcript_index: usize,
    /// runtime tool 名
    pub tool: String,
    /// 原始 tool call input
    pub input: serde_json::Value,
    /// tool result（用于判断成功/失败）
    pub output: serde_json::Value,
    /// 此步是否被判定为成功路径的一部分
    pub on_success_path: bool,
    /// 此步使用的 locator 类型（用于 anchor 提取）
    pub locator_info: Option<LocatorDigest>,
}

/// Observation 摘要，不含完整像素数据
pub struct ObservationDigest {
    /// 对应的 transcript step index
    pub after_step_index: usize,
    /// 前台 app
    pub frontmost_app: Option<String>,
    /// 窗口标题
    pub window_title: Option<String>,
    /// 元素摘要列表（role + label + bounds，不含完整树）
    pub element_summaries: Vec<ElementSummary>,
    /// 是否包含 screenshot
    pub has_screenshot: bool,
}

pub struct ElementSummary {
    pub role: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub bounds_norm_1000: Option<RectNorm1000>,
}

/// 可能需要参数化的字面量候选
pub struct LiteralCandidate {
    pub step_index: usize,
    pub json_path: String,
    pub value: serde_json::Value,
    /// 启发式判断：是否像用户输入（非系统生成）
    pub likely_user_input: bool,
}

/// Locator 信息摘要
pub struct LocatorDigest {
    pub kind: String,
    pub element_role: Option<String>,
    pub element_label: Option<String>,
    pub element_bounds_norm_1000: Option<RectNorm1000>,
}

/// 从 observation 和 locator 推断的 anchor 候选
pub struct AnchorCandidate {
    pub name_hint: String,
    pub source_step_index: usize,
    pub role: Option<String>,
    pub label_contains: Option<String>,
    pub value_contains: Option<String>,
    pub bounds_hint_norm_1000: Option<RectNorm1000>,
    /// 该候选在多少个 observation 中出现过
    pub occurrence_count: usize,
}
```

作用：

- deterministic scaffold
- 供 LLM compile / repair 使用
- 保留 evidence，不直接暴露给 runner

## 9.6 Certification Case / Report

建议 case 文件也使用 TOML：

```toml
name = "smoke"
repeats = 3
required_passes = 3

[params]
title = "周会纪要"
content = "1. 项目进展\n2. 风险\n3. 决议"
```

report 建议记录：

- task name / version
- run target
- linked session id
- case 列表
- repeats / passes / failures
- 失败 session id
- 失败 step id
- 失败分类
- step 级摘要与关键 observation / artifact 引用
- 总结结论

## 10. Task Runner 设计

## 10.1 执行原则

runner 必须完全确定性：

- 不调用模型
- 不做自由规划
- 不根据自然语言再推理下一步
- 只执行 manifest 已定义步骤

runner 的核心循环：

```text
bind params
  -> load manifest
  -> create runtime session
  -> for step in steps
       -> guard check
       -> if anchor needed: fresh observe + resolve anchor
       -> invoke tool
       -> verify / assert
       -> if side effect: mark UI stale and refresh as required
  -> emit Completed / Error
```

## 10.2 与 runtime 的关系

runner 不能直接调用平台 driver，而应继续走：

- `ToolRegistry.invoke()`
- `RuntimeCore.observe/query/act()`

理由：

- 复用已有 capability 检查
- 复用 side-effect policy
- 复用 action verification
- 复用统一 audit / timeout / target 解析

## 10.3 守卫与断言

为了高成功率，每个关键 side-effect step 前后都应允许显式 guard：

- app 在前台
- window 焦点正确
- 当前 observation 可用
- 目标 anchor 已解析

关键动作后的 postcondition：

- focus 已切换
- window state 符合预期
- geometry 已落到预期
- 当前 UI 已进入下一状态

## 10.4 Fresh Observe Discipline

Task runner 应继承 agent 当前的“副作用后 refresh visual state”纪律。

建议默认规则：

- 进入有 anchor 的 action step 前，若当前 UI state stale，则自动 fresh observe
- side-effect step 成功后，将 UI state 标记为 stale
- 下一个依赖 anchor / assert 的 step 前，必须重新 observe

## 10.5 Session 与审计

Task run 继续写入共享 `SessionStore`，不发明新的 audit store。采用本路线后，`SessionStore` 之上还需要由 `operator-runtime` 提供共享 transcript schema 与 loader。

第一阶段职责划分：

- `operator-runtime::transcript`：提供共享 replayable transcript 的 schema、loader、writer helper
- scaffold / compile：以 runtime 持有的共享 transcript 为权威输入
- task-origin repair / certify：以 task run report + 关联 transcript 为权威输入
- 共享 `SessionStore`：继续提供统一 session 检索与 JSONL 落盘 backend

共享 transcript 的最低要求：

- 继续可由现有 `SessionEvent` 线性投影得到
- `ToolResult` payload 必须是 entry-neutral 的共享结构，不直接绑定 `AgentToolResult`
- agent run 与 task run 都能通过同一 loader 读回 `ToolCall` / `ToolResult` / `Completed` / `Error`

Task run 仍可复用现有 session 事件模型：

- `UserInput`
  - 记录 task name 和绑定参数摘要
- `ToolCall`
- `ToolResult`
- `Completed`
- `Error`

第一阶段不要求新增 task 专属 `SessionEvent` 变体；task-specific 语义继续放在 `TaskRunReport`，不要反向污染共享 transcript schema。

## 10.6 错误分类

runner 需要把失败原因结构化分类，至少包括：

- `parameter_error`
- `guard_failed`
- `observe_failed`
- `anchor_unresolved`
- `tool_failed`
- `verification_failed`
- `assertion_failed`
- `timeout`
- `capability_denied`

这些分类既服务人类排障，也服务后续 `task repair`。

## 11. Scaffold / Compile / Repair 设计

## 11.1 录制前提

准备沉淀为 task 的 agent run，必须使用 compile-friendly recording profile。

推荐规则：

- `include_elements = true`
- 保留 screenshot
- 保留副作用后的 auto-observe
- session 持久化必须完整

补充事实：

- 当前 `AgentConfig` 默认 `include_elements = false`
- 因此 compile-friendly recording profile 必须显式传入 `--include-elements` 或等效配置

原因：

- 没有 elements 的 transcript 更容易退化成坐标脚本
- 没有 fresh observation 的 transcript 难以提炼锚点和 postcondition

## 11.2 Scaffold

`task scaffold` 不依赖 LLM，只做 deterministic 提取。

物理归属建议：

- scaffold 放在 `operator-task`
- 输入通过 `operator-runtime::transcript::load_persisted_transcript(...)` 读取
- scaffold 输出的 `CompileIr` / draft manifest 类型由 `operator-task` 自身定义

执行内容：

- 读取 `operator-runtime` 持有的 persisted transcript
- 提取成功路径上的 `ToolCall` + `ToolResult`
- 删除失败分支、噪音 observe、无效探索
- 收集字面量候选
- 生成 anchor 候选
- 输出一个可人工修改的 draft manifest

scaffold 的目标不是完美，而是把人工固化工作从”从零写 manifest”降低到”review + 调整”。

#### 成功路径识别

当前共享 transcript 仍只在最终有 `Completed`/`Error` 标记，缺乏 per-step 成功标记。scaffold 使用以下启发式规则识别成功路径：

1. **session 必须为 `Completed` 状态**：`Failed`/`Interrupted` session 不适合 scaffold
2. **反向标记法**：从最后一个 `ToolCall`/`ToolResult` 对开始，反向遍历 transcript
3. **失败 tool call 识别**：
   - `ReplayableTranscriptEvent::ToolResult.result.is_error == true` → 标记为失败
   - 同一个 tool + 相似 input 连续出现多次，只保留最后一次成功的 → 前面的视为重试噪音
4. **噪音 observe 识别**：
   - 两个相邻 observe 之间没有 action step → 删除前一个
   - observe 后紧跟的 action 被标记为失败 → 该 observe 一并删除
5. **成功路径**：删除所有被标记为失败和噪音的步骤后，剩余的有序 `ToolCall`/`ToolResult` 对即为成功路径

此启发式不保证 100% 准确，scaffold 输出始终为 `draft` 状态，需要人工 review。

## 11.3 Compile

`task compile` 在 scaffold / compile IR 基础上允许使用模型优化草稿。

职责划分建议：

- `operator-task`：定义 compile IR、pass 边界、输出 schema
- `operator-agent`：提供 model-backed compiler implementation
- `operator-cli`：装配 model selector、调用 compile backend、落盘 manifest / report

推荐拆成多个 pass，每个 pass 必须输出严格 JSON：

1. `trajectory_distill`
   - 保留成功骨架
2. `parameterize`
   - 把稳定字面量提升成参数
3. `anchor_synthesize`
   - 结合 observation digest、截图、历史 locator 生成 `AnchorSpec`
4. `guards_and_asserts`
   - 补守卫、重试、断言
5. `manifest_emit`
   - 产出完整 draft

compile 输入必须来自 `operator-runtime` 持有的 persisted transcript，而不是 planner 的 model-facing context。

## 11.4 Repair

`task repair` 基于失败运行证据做局部修复。

输入：

- 现有 task manifest
- 失败 task run report
- 关联 `session_id`
- 关联 runtime transcript
- 失败 step / 错误分类
- 前后 observation / artifacts

输出：

- `TaskPatch`
- 新的 draft manifest
- repair report

CLI 入口可以继续使用 `--session <failed-session-id>`，但底层应先按 `session_id` 同时解析到对应的 task run report 与关联 transcript，再进入 repair 流程。

repair 原则：

- 只允许改动局部：
  - anchor
  - guard
  - retry policy
  - postcondition
- 关联 transcript 是 repair 的共享基础证据，task run report 提供 task-specific 语义定位
- 不默认重写整份 task
- repair 后必须重新 certify

## 11.5 Compile Provenance

manifest 需要记录 provenance：

- `mode = scaffolded | compiled | repaired`
- `source_sessions`
- `compiled_at`
- `model`
- `base_task_version`

这样后续排障时可以知道 task 来自哪里、是否经过模型处理。

## 12. CLI / UX 设计

## 12.1 与当前 CLI authority 的关系

本节定义的是 **task feature line 的命令提案**，不单独覆写当前根 help authority。

约束：

- 当前根 help / 分组 / help 文案仍以 `CLI_DESIGN.md` 与 `docs/COMMAND.md` 为准
- 第一次把 `operator task` 落地到 northbound shell surface 时，必须在同一 issue 中同步更新两份 CLI authority 文档
- 在完成同步之前，本节只能视为 task feature 自身的命令设计，不视为已经生效的稳定 CLI 契约

推荐集成方式：

- 把 `task` 放在 `AI` 分组，与 `agent` 并列

```text
AI
  agent   Execute a natural-language task against a target
  task    Run and manage parameterized automation tasks
```

这样做的原因：

- task 的来源通常是 agent transcript
- compile / repair 需要模型
- 对用户心智来说，`agent` 是一次性探索，`task` 是沉淀后的复用资产

## 12.2 建议命令树

建议 task feature line 第一阶段 northbound shell surface：

```text
operator task
  list
  show <name>
  run <name>
  scaffold --session <id> --name <task-name>
  compile --session <id>... --name <task-name>
  certify <name>
  repair <name> --session <failed-session-id>
```

## 12.3 参数输入面

第一阶段参数输入使用稳定、静态、可文档化的输入方式：

- `--set <name=value>`
- `--set-file <name=path>`
- `--params-json <json>`

示例：

```bash
operator task run notes.create \
  --set title='周会纪要' \
  --set-file content=./body.md
```

或者：

```bash
operator task run notes.create \
  --params-json '{"title":"周会纪要","content":"1. 项目进展\n2. 风险"}'
```

第一阶段**不支持**：

```bash
operator task run notes.create --title ... --content ...
```

原因：

- task-specific 参数名运行时才知道
- 会破坏静态 help / completion / error message 契约
- 与当前 CLI 设计的稳定 shell surface 不一致

## 12.4 输出设计

### `task list`

至少显示：

- task name
- version
- status
- source mode
- certified summary
- last updated

### `task show`

至少显示：

- manifest metadata
- params
- anchors 摘要
- steps 摘要
- provenance
- latest certification summary

### `task run`

human-readable 输出建议包含：

- task name / version
- target
- session id
- resolved params summary
- final result
- failed step（若失败）

`--json` 输出建议包含：

- `task`
- `version`
- `target`
- `session_id`
- `status`
- `summary`
- `failed_step`
- `error_kind`

### `task scaffold` / `compile` / `repair`

输出建议包含：

- 输出 task 路径
- draft 状态
- source sessions
- 推断出的 params
- 推断出的 anchors 数
- review notes / warnings

### `task certify`

输出建议包含：

- executed cases
- total runs
- pass / fail
- failed session ids
- certification result

## 12.5 便捷录制 UX

可选便捷入口：

```bash
operator agent "新建一个备忘录，标题是 X，内容是 Y" --record-task notes.create
```

语义建议为：

1. 用 compile-friendly recording profile 跑 agent
2. 成功后自动触发 `task scaffold` 或 `task compile`
3. 生成 `notes.create` draft

但这只是便利入口，不应成为 task 体系的唯一入口。属于 **Phase C** 范围，在 compile 能力就绪后再提供。

核心入口仍应是：

- `task scaffold --session ...`
- `task compile --session ...`

## 13. 版本管理与并发

### 13.1 版本策略

Task manifest 使用单调递增整数版本号：

- scaffold / compile 首次生成：`version = 1`
- 每次 repair 成功后：`version += 1`
- 手动编辑 manifest 后建议手动递增版本
- certify report 绑定执行时的 version
- `certified` 状态绑定到具体 version，version 变更后自动回退为 `draft`

第一阶段不要求版本历史存储或回滚能力。旧版本可通过 git 追溯。

### 13.2 并发控制

Task 操作 GUI 桌面，天然不适合同一 target 并行执行：

- 锁粒度应与 resolved target 对齐，而不是全局 operator home
- 同一时刻只允许一个 `task run` 实例持有某个 target 的 GUI 执行权
- 建议使用文件锁 `~/.operator/tasks/.locks/<target>.run.lock` 实现 per-target 互斥
- 同一 target 获取锁失败时立即报错 `concurrent_run_denied`，不排队等待
- 不同 target 的 task run 允许并发，保持与当前 runtime / MCP 的 target 级串行原则一致
- `task certify` 对同一 target 的多 case 串行运行，共享该 target 的锁
- scaffold / compile / repair 不操作 GUI，不受此锁限制

## 14. 高成功率设计原则

这条特性的价值建立在成功率上，因此以下约束必须硬性执行：

1. **执行期不使用 LLM**
2. **录制期优先 `include_elements=true`**
3. **不复用历史 `SnapshotElement`**
4. **关键 action 前后必须有 guard / verify / observe discipline**
5. **默认优先语义 anchor，不优先绝对坐标**
6. **repair 使用 patch，不使用整份重编译**
7. **task 可信度来自 certify，不来自 compile 成功**
8. **version 变更后 certified 状态自动失效**
9. **并发 task run 通过文件锁互斥，不允许竞态**

## 15. Certification 设计

高成功率不能只靠设计假设，必须有认证流程。

建议 `task certify` 支持：

- 多 case
- 每个 case 多次重复
- 记录 pass rate
- 记录失败 step / failure kind / session id
- 生成可回溯报告

建议 task 状态：

- `draft`
- `draft_with_failed_cert`
- `certified`

状态规则：

- scaffold / compile / repair 后进入 `draft`
- certify 全部通过后可标记为 `certified`
- certify 失败则保留 draft，并附带失败报告

第一阶段不要求新增单独的 `approve` 命令。

## 16. 与现有模块的边界

### 16.1 `operator-core`

第一阶段不修改公共 `Action` / `Locator` 语义去直接承载 task。

task 专属模型：

- `TaskManifest`
- `AnchorSpec`
- `TaskStep`
- `CompileIR`

都放在 `operator-task`。

### 16.2 `operator-runtime`

runtime 继续只负责 typed execution 与共享运行时基础设施，不关心 task 资产生命周期。

本路线下，runtime 额外负责：

- 共享 `SessionStore` backend
- replayable transcript schema / loader / writer helper
- 让 agent / task / CLI 通过同一组 helper 读取 persisted transcript

runner 通过 `ToolRegistry.invoke()` 复用 runtime；scaffold 通过 runtime transcript helper 读取持久化轨迹。

### 16.3 `operator-agent`

agent 继续负责：

- 自然语言探索
- session transcript 产出
- 模型/provider 抽象
- model-backed task compile / repair backend

agent 不负责在线执行 task。

agent 不再拥有 transcript schema 本身；它只是共享 transcript 基础设施的一个 producer。

### 16.4 `operator-cli`

CLI 负责：

- `operator task ...` northbound shell surface
- 参数输入解析
- compile / repair / run / certify orchestration

CLI 不直接实现 task 核心逻辑。

## 17. 分阶段落地

推荐实施顺序：

### Phase A

- `operator-runtime` 共享 transcript 层
- `operator-task` crate
- manifest schema
- file-backed task store
- deterministic runner
- `operator task list/show/run`

### Phase B

- deterministic scaffold in `operator-task`
- compile-friendly recording profile
- `operator task scaffold`

### Phase C

- LLM-assisted compile
- `operator-agent` 中的 model-backed compile / repair backend
- `operator task compile`
- provenance / draft report
- `operator agent --record-task` 便捷入口

### Phase D

- certify case / report
- `operator task certify`

### Phase E

- patch-based repair
- `operator task repair`

这五个阶段都在本设计范围内；复合 tool promotion 不在本设计范围。

## 18. 示例用户路径

### 18.1 先录制再固化

```bash
operator agent "新建一个备忘录，标题是 周会纪要，内容是 1. 项目进展" \
  --include-elements

operator task scaffold --session agent-20260402-abc123 --name notes.create

operator task run notes.create \
  --set title='周会纪要' \
  --set-file content=./body.md
```

### 18.2 直接 compile

```bash
operator task compile \
  --session agent-20260402-abc123 \
  --name notes.create \
  --model openai
```

### 18.3 certify

```bash
operator task certify notes.create --case ~/.operator/tasks/notes.create/cases/smoke.toml
```

### 18.4 repair

```bash
operator task repair notes.create \
  --session taskrun-20260402-def456 \
  --model openai
```

## 19. 最终结论

Task 特性的核心不是“回放 agent”，而是：

- 以 `operator-runtime` 持有的共享 transcript / task run report 为事实输入
- 以 manifest 为权威资产
- 以 deterministic runner 为执行底座
- 以 scaffold / compile / repair / certify 为控制面生命周期

在这个设计下：

- `agent` 是一次性探索器
- `task` 是长期复用资产
- `compile` / `repair` 可以利用 LLM
- `run` 永远不依赖 LLM

这使得 Operator 可以同时拥有：

- 自然语言探索能力
- 高成功率的参数化 GUI 自动化能力
- 与当前 typed runtime 一致的架构边界
