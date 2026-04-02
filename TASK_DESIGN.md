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
- 当前 `Locator::SnapshotElement` / `Snapshot*Coords` 绑定历史 `snapshot`
- selector locator 是否暴露给 planner，依赖当前 session 是否有 usable element observation
- runtime action 执行链已经统一，不需要再发明一套新执行协议

## 3. 设计目标

- 为 Operator 增加一个稳定的 `task` northbound shell surface
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

推荐引入一个新库 crate：

- `crates/operator-task`

职责：

- task manifest schema
- task store
- task runner
- anchor resolver
- scaffold heuristics / compile IR
- certify
- run / compile / certify reports

建议结构：

```text
crates/operator-task/
  src/
    lib.rs
    error.rs
    schema.rs
    store.rs
    runner.rs
    anchor.rs
    scaffold.rs
    compile_ir.rs
    certify.rs
    report.rs
```

同时定义一个**逻辑上的** compile subsystem：

- `TaskCompiler`
- `TaskRepair`

为了避免 runtime 反向依赖模型，建议采用以下物理布局：

1. `operator-task`
   - 不依赖模型/provider
   - 提供 manifest、IR、runner、certify
2. `operator-agent`
   - 增加 `task_compile` 模块
   - 复用现有 `ModelRegistry` / `ResolvedModel`
   - 输入 `CompileIR`，输出 `TaskManifestDraft` 或 `TaskPatch`

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
  └── may depend on operator-task (for compile IR / manifest draft types)
```

关键点：

- `operator-runtime` 不依赖 `operator-task`
- `operator-runtime` 不依赖模型/provider
- task 不是 entry 层，而是可被 CLI 和 compile path 复用的领域库

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
anchor = "title_field"
verifications = ["Focus"]

[[steps]]
id = "type_title"
tool = "type"
kind = "action"
args = { text = "${title}", clear_before = true }

[[steps]]
id = "focus_body"
tool = "click"
kind = "action"
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

匹配顺序建议：

1. 限定 app / window
2. fresh observe，强制 `include_elements=true`
3. 用 `role + label + value + bounds` 综合打分
4. 找不到时退回 text 匹配
5. 再退回 role/index
6. 最后才退回 normalized bounds

默认禁止：

- 直接回放绝对坐标
- 直接复用历史 `SnapshotElement`

## 9.4 Task Step

Task step 本质上是“对现有 runtime tool 的封装调用”，而不是另一套动作协议。

建议字段：

- `id`
- `kind`
  - `observe`
  - `action`
  - `assert`
- `tool`
- `args`
- `anchor`
- `guard`
- `retry`
- `postconditions`
- `on_failure`

约束：

- `tool` 名直接复用 runtime tool 名
- step 级参数替换只允许引用 task params 或前序 step 输出摘要
- side-effect step 默认要求 fresh observe discipline

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
- case 列表
- repeats / passes / failures
- 失败 session id
- 失败 step id
- 失败分类
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

Task run 继续写入共享 `SessionStore`，不发明新的 audit store。

第一阶段可复用现有 session 事件模型：

- `UserInput`
  - 记录 task name 和绑定参数摘要
- `ToolCall`
- `ToolResult`
- `Completed`
- `Error`

第一阶段不要求新增 task 专属 `SessionEvent` 变体。

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

原因：

- 没有 elements 的 transcript 更容易退化成坐标脚本
- 没有 fresh observation 的 transcript 难以提炼锚点和 postcondition

## 11.2 Scaffold

`task scaffold` 不依赖 LLM，只做 deterministic 提取：

- 读取 persisted session transcript
- 提取成功路径上的 `ToolCall` + `ToolResult`
- 删除失败分支、噪音 observe、无效探索
- 收集字面量候选
- 生成 anchor 候选
- 输出一个可人工修改的 draft manifest

scaffold 的目标不是完美，而是把人工固化工作从“从零写 manifest”降低到“review + 调整”。

## 11.3 Compile

`task compile` 在 scaffold / compile IR 基础上允许使用模型优化草稿。

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

compile 输入必须来自 persisted session，而不是 planner 的 model-facing context。

## 11.4 Repair

`task repair` 基于失败 session 做局部修复。

输入：

- 现有 task manifest
- 失败 run session
- 失败 step / 错误分类
- 前后 observation / artifacts

输出：

- `TaskPatch`
- 新的 draft manifest
- repair report

repair 原则：

- 只允许改动局部：
  - anchor
  - guard
  - retry policy
  - postcondition
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

## 12.1 根命令分组

建议把 `task` 放在 `AI` 分组，与 `agent` 并列：

```text
AI
  agent   Execute a natural-language task against a target
  task    Run and manage parameterized automation tasks
```

原因：

- task 的来源通常是 agent transcript
- compile / repair 需要模型
- 对用户心智来说，`agent` 是一次性探索，`task` 是沉淀后的复用资产

## 12.2 稳定命令树

建议第一阶段 northbound shell surface：

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

但这只是便利入口，不应成为 task 体系的唯一入口。

核心入口仍应是：

- `task scaffold --session ...`
- `task compile --session ...`

## 13. 高成功率设计原则

这条特性的价值建立在成功率上，因此以下约束必须硬性执行：

1. **执行期不使用 LLM**
2. **录制期优先 `include_elements=true`**
3. **不复用历史 `SnapshotElement`**
4. **关键 action 前后必须有 guard / verify / observe discipline**
5. **默认优先语义 anchor，不优先绝对坐标**
6. **repair 使用 patch，不使用整份重编译**
7. **task 可信度来自 certify，不来自 compile 成功**

## 14. Certification 设计

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

## 15. 与现有模块的边界

### 15.1 `operator-core`

第一阶段不修改公共 `Action` / `Locator` 语义去直接承载 task。

task 专属模型：

- `TaskManifest`
- `AnchorSpec`
- `TaskStep`
- `CompileIR`

都放在 `operator-task`。

### 15.2 `operator-runtime`

runtime 继续只负责 typed execution，不关心 task 资产生命周期。

runner 通过 `ToolRegistry.invoke()` 复用 runtime。

### 15.3 `operator-agent`

agent 继续负责：

- 自然语言探索
- session transcript 产出
- 模型/provider 抽象

agent 不负责在线执行 task。

但 compile / repair 的模型 pass 可以逻辑上属于 task feature line，并物理上暂存于 `operator-agent` 以复用模型抽象。

### 15.4 `operator-cli`

CLI 负责：

- `operator task ...` northbound shell surface
- 参数输入解析
- compile / repair / run / certify orchestration

CLI 不直接实现 task 核心逻辑。

## 16. 分阶段落地

推荐实施顺序：

### Phase A

- `operator-task` crate
- manifest schema
- file-backed task store
- deterministic runner
- `operator task list/show/run`

### Phase B

- deterministic scaffold
- compile-friendly recording profile
- `operator task scaffold`

### Phase C

- LLM-assisted compile
- `operator task compile`
- provenance / draft report

### Phase D

- certify case / report
- `operator task certify`

### Phase E

- patch-based repair
- `operator task repair`

这五个阶段都在本设计范围内；复合 tool promotion 不在本设计范围。

## 17. 示例用户路径

### 17.1 先录制再固化

```bash
operator agent "新建一个备忘录，标题是 周会纪要，内容是 1. 项目进展" \
  --include-elements

operator task scaffold --session agent-20260402-abc123 --name notes.create

operator task run notes.create \
  --set title='周会纪要' \
  --set-file content=./body.md
```

### 17.2 直接 compile

```bash
operator task compile \
  --session agent-20260402-abc123 \
  --name notes.create \
  --model openai
```

### 17.3 certify

```bash
operator task certify notes.create --case ~/.operator/tasks/notes.create/cases/smoke.toml
```

### 17.4 repair

```bash
operator task repair notes.create \
  --session taskrun-20260402-def456 \
  --model openai
```

## 18. 最终结论

Task 特性的核心不是“回放 agent”，而是：

- 以 persisted session 为事实输入
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
