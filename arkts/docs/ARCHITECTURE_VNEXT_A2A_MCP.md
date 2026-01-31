# 架构整改设计（vNext）：A2A-first + MCP Tools（端侧 ArkTS）

> 目标读者：ArkTS 端 Operation Agent 开发者（Service/IPC/UI），用于指导“云侧 agent_v3（Thinkflow Runtime）”对接整改。
>
> 依据：`agent_v3/docs/device_side_integration.md`、`agent_v3/docs/vnext_a2a_mcp_architecture.md`。
>
> 本文聚焦 **端侧架构**：拆分 “发消息 / 收事件 / 执行工具”，并将 **UI 呈现（event 驱动）** 与 **动作执行（tool_call 驱动）** 解耦。

---

## 0. 背景与整改动机

当前 ArkTS 端实现以 `TaskManager` 的“截图→invoke→执行 action→再截图→invoke(action_result)”循环为主（`/invoke` 协议），这会导致：

- **耦合**：UI 主要依赖 snapshot/step/action 来展示；动作执行与 UI 状态推进绑在一起。
- **协议不对齐**：agent_v3 vNext 已经切换到 **A2A 事件流 + MCP Tools**，不再输出 `output.action` 给端侧执行。
- **扩展性受限**：云侧需要流式事件（Thought/Description/tool.trace/input-required），端侧需要单独承载工具执行的可靠性与观测。

整改目标：把端侧从“云端输出动作→端侧执行动作”的旧模式，切换为：

- 北向交互：**A2A message:stream（SSE）**（发起/续跑任务 + 收到事件流）
- 工具执行：**MCP Tools via harmony_mcp_proxy（WebSocket）**（收 tool_call → 执行 → 回 tool_result，必须带 screenshot_b64）
- UI：完全基于 **event** 呈现（而不是 action/snapshot 的 step loop）

---

## 1. 云侧契约（必须对齐的事实）

### 1.1 A2A（北向）

端侧需要实现（至少）：

- `POST /a2a/v1/message:send`（blocking=true）：拿终态 Task（可做辅助/兜底）
- `POST /a2a/v1/message:stream`（SSE）：**推荐主入口**，边执行边吐：
  - `statusUpdate`：`working` / `input-required` / `completed` / `failed`
  - `artifactUpdate`：`assistant.text` / `thinkflow.delta` / `tool.trace` / `thinkflow.output`
- `POST /a2a/v1/tasks/<TASK_ID>:subscribe`（SSE）：断线重连/观测面板可用（可作为 Phase2）

关键语义：

- `contextId` → Thinkflow `session_id`
- `taskId` → Thinkflow `run_id`
- `input-required` 必须靠 `run.input_required` 投影（`statusUpdate(state=input-required, final=true)`），提示文案在 `statusUpdate.message.parts[0].text`，交互动作可放在 `statusUpdate.message.metadata.action`。

### 1.2 MCP Tools（南向）

端侧 **DeviceToolsEndpoint（端侧工具入口）** 通过 WebSocket 连接 `harmony_mcp_proxy`：

- 连接：`ws://<proxy_host>:<ws_port>/ws?device_id=<DEVICE_ID>&session_id=<SESSION_ID>`
- 建连后可发送：`{"type":"hello","device_id":"...","session_id":"..."}`
- 接收：`{"type":"tool_call","call_id":"...","name":"click","arguments":{...}}`
- 回传：`{"type":"tool_result","call_id":"...","result":{...}}`

硬约束：

- **每个 tool_result 必须携带 `result.screenshot_b64`（base64 图片字节，不带 data URL 前缀）**。
- 坐标体系：`0..1000` 归一化。

---

## 2. 端侧目标架构（分三条逻辑线，互不耦合）

### 2.1 组件总览

```
UI (FloatChatPage / AgentSpaceViewer / Index Debug)
    │ IPC (events + control)
    ▼
AgentController (Service Facade)
    └─ AgentProxy (vNext)       # 端侧对 agent_v3 的统一代理入口（A2A + MCP Tools）
        ├─ AgentEventHub        # 统一事件总线（给 UI 的唯一输入）
        ├─ A2AClient            # A2A 交互：发消息 + 收 SSE 事件流
        ├─ DeviceToolsEndpoint  # WS：端侧工具入口（收 tool_call，执行，回 tool_result（含截图））
        └─ ToolCallDispatcher   # tool_call → 本地执行（复用 ToolKit/AgentSpace）

Cloud (Thinkflow Runtime / agent_v3)  ──(MCP JSON-RPC)──> harmony_mcp_proxy ──(WS tool_call/result)──> DeviceToolsEndpoint
Cloud (A2A) <──(HTTP+SSE)── A2AClient
```

### 2.2 职责边界（强约束）

- `AgentProxy`：端侧 “vNext 对接层” 的边界；对 `AgentController` 暴露少量稳定接口（start/resume/stop + events）。
- `A2AClient`：只处理 **消息/事件**（message:stream SSE）；不执行 click/type。
- `DeviceToolsEndpoint`：只处理 **工具调用**（tool_call/result）；不理解 Thought/Description，也不驱动 UI。
- UI：只消费 `AgentEventHub` 的事件来渲染；UI 不参与“动作循环”的推进。

---

## 3. 模块设计细化

### 3.0 命名与兼容策略（建议）

- 保留 `AgentProxy` 这个名字，但将其语义升级为 **vNext 对接总入口（A2A + MCP Tools）**。
- 当前工程里用于 `/invoke` 的 `AgentProxy.ets` 建议迁移为 `legacy/LegacyInvokeClient`（或类似名字），避免新旧语义冲突。
- `TaskManager(/invoke loop)` 建议保留为 legacy fallback（Debug 开关可启），等 vNext 稳定后再下线。

### 3.1 AgentEventHub（统一事件总线）

目的：让 UI 只依赖一个稳定契约，屏蔽 A2A/MCP 的细节差异。

建议复用现有 `TaskEventQueue` 的形态（`seq/type/message/payloadJson`），扩展 `type` 集合：

- `session`：started/resumed/finished/failed（payload: {contextId, taskId}）
- `status`：working/input-required/completed/failed（payload: {text?, updatedAt?}）
- `assistant.delta`：流式文本（payload: {textChunk}）
- `delta`：结构化 delta（payload: thinkflow.delta 的 data，含 kind=agent.thought/agent.description/...）
- `final`：最终输出（payload: thinkflow.output data）
- `tool.trace`：云侧观测（payload: tool.trace 记录）
- `tool.exec`：本地工具执行观测（payload: {phase: started|completed|failed, callId, name, ms?, error?}）
- `net`：连接/重连/错误（payload: {scope: a2a|mcp, state, error?}）

API（建议）：

- `push(type, message, payloadJson?)`
- `pull(lastSeq) -> batch`
- `reset()`（仅 debug 用）

### 3.2 A2AClient（发消息 + 收 SSE 事件流）

#### 3.2.1 发送（首轮/续跑统一）

端侧统一调用 `message:stream`（推荐主流程）：

- 首轮：不带 `taskId`
- 续跑：带 `taskId`（resume）

消息 parts 建议：

- 首轮：`TextPart` + `FilePart(screenshot.jpeg bytes=b64)`
- 续跑（input-required）：使用 `TextPart`（与 agent_v3 当前实现对齐），同时仍带 `FilePart`（最新截图）
  - call_user：文本回答
  - interact：文本选项
  - take_over：文本备注/确认

安装应用列表：放在请求最外层 `metadata.installed_apps`（首轮带一次即可）。

#### 3.2.2 SSE 解析与事件映射

SSE payload（每条 `data:` 都是一个 JSON dict）只需处理两类：

1) `statusUpdate`
- `state=working` → `status(working)`
- `state=input-required` → `status(input-required)` + `input_required`（从 `statusUpdate.message` 提取）
- `state=completed/failed` → `session(finished/failed)` + `status(...)`

2) `artifactUpdate`
- `artifactId="assistant.text"` → `assistant.delta`
- `artifactId="thinkflow.delta"` → `delta`（UI 通常优先消费 `kind=agent.description`）
- `artifactId="thinkflow.output"` → `final`
- `artifactId="tool.trace"` → `tool.trace`

> 注意：`tool.trace` 的 args/result 可能被打码；端侧工具执行不依赖它，仅用于 UI/调试观测。

#### 3.2.3 会话状态（端侧）

端侧必须持有：

- `contextId`：设备级稳定标识（建议与 WS session_id 一致）
- `taskId`：从 SSE 首次出现的 `task.id` 捕获并保存（续跑必用）

并处理错误：

- `HTTP 409 SESSION_BUSY`：向 UI 投递 `net/error`（提示“当前会话忙”）
- 断线：投递 `net` 事件；如已拿到 taskId，可选 Phase2 增加 `tasks:subscribe` 观测恢复

### 3.3 DeviceToolsEndpoint（WS 工具执行入口）

> 注：本文统一称为 `DeviceToolsEndpoint`（端侧工具入口），避免与系统 Ability 的 “Service” 概念混淆。

职责：

1) 连接/保持 WS：`harmony_mcp_proxy`
2) 处理 `tool_call`：执行 → 截图 → 回 `tool_result`
3) 将本地执行观测投递到 `AgentEventHub`（tool.exec）

并发策略：

- 同一 session 内 **串行执行** tool_call（避免 VTS 输入并发）

#### 3.3.1 ToolCallDispatcher（tool → 本地执行映射）

输入：`{call_id, name, arguments}`  
输出：`{status, error?, screenshot_b64, data?}`

推荐复用现有执行栈（最小改动）：

- 将 tool_call 规范化为端侧 `AgentAction`：
  - `click/double_click/long_click/type/wait/launch_app/press_enter/press_back`：参数直映射
  - `swipe`：云侧参数是 `start/end`，端侧执行层更偏 `start_point/end_point`，dispatcher 负责转换
- 调用 `ToolKit.executeAction(action)` 执行
- **无论成功失败都调用 `ToolKit.screenshotBase64()`** 采集最新截图
- 拼装 `tool_result` 回传（必须含 screenshot_b64）

特殊工具：

- `screenshot`：不要走“保存相册”的旧逻辑；直接返回 `ToolKit.screenshotBase64()`（SUCCESS）

已知缺口：

- `press_back` 当前端侧未实现真实注入，短期返回 `FAILED + error=press_back_not_implemented`，但仍需携带 screenshot_b64。

---

## 4. AgentController / IPC 调整建议

### 4.1 Service 内部结构

`AgentController` 保持门面角色，但内部从 “TaskManager(/invoke loop)” 切换为 “AgentProxy(vNext)”：

- `AgentProxy`（vNext，A2A + MCP Tools）
  - `A2AClient`（北向）
  - `DeviceToolsEndpoint`（南向）
  - `AgentEventHub`（统一输出）
- `SessionSnapshot`（兜底摘要；必要时可从 events 归并生成）

旧的 `TaskManager + legacy/LegacyInvokeClient` 可保留为 fallback（Debug 开关），便于回滚与对照。

### 4.2 IPC 面向 UI（尽量复用现有接口）

建议语义升级：

- `START_TASK_TEXT`：从“启动旧 task loop” → “首轮 message:stream（Text+Screenshot）”
- `SUBMIT_USER_REPLY`：从“action_result 回执” → “input-required resume（TextPart+Screenshot）”
- `PULL_EVENTS`：继续使用，成为 UI 的主驱动
- `GET_SNAPSHOT`：继续保留，但只做兜底（UI 不应以 snapshot 推进流程）

可新增（推荐用于 Debug Tab）：

- `SET_A2A_BASE_URL`（原 cloudEndpoint 可复用语义）
- `SET_MCP_WS_URL / SET_DEVICE_ID / SET_CONTEXT_ID`
- `MCP_CONNECT / MCP_DISCONNECT / MCP_STATUS`
- `A2A_STREAM_START / A2A_STREAM_STOP / A2A_STATUS`
- `DEBUG_SIMULATE_TOOL_CALL(name, argumentsJson)`（只测 dispatcher，不依赖 WS）

---

## 5. Debug Tab：非耦合子模块的手动调试方案（推荐）

结论：**可以**。Index 的 Debug Tab 是最合适的“端侧验收面板”，因为它：

- 已经具备 IPC Client
- 可以在不影响 Float/Viewer 主链路的前提下，单独拉起/停止 A2A/WS
- 能把“模块状态 + 最近事件 + 最近错误”可视化

建议在 Debug Tab 新增 4 组区域（每组只依赖对应模块）：

### 5.1 A2A（消息/事件）
- 配置：`baseUrl`、`contextId`
- 按钮：
  - `Start Stream (text)`：用当前截图发起首轮 `message:stream`
  - `Resume (data)`：对当前 taskId 续跑（answer/choice/done）
  - `Stop Stream`（本地停止读取）
  - `Fetch Task (GET /tasks/{id})`（辅助）
- 展示：
  - 当前 `taskId`、当前 `status.state`、最近 `assistant.delta/description` 文案

### 5.2 MCP WS（DeviceToolsEndpoint 工具入口）
- 配置：`wsUrl`、`deviceId`、`sessionId(contextId)`
- 按钮：
  - `Connect / Disconnect`
  - `Simulate tool_call`（走 dispatcher，验证 click/type/swipe 等参数解析与截图回传）
- 展示：
  - WS 状态、最后一个 call_id、最后一次 tool_result 的 status/error/screenshot_len

### 5.3 EventHub（统一事件）
- 按钮：`Pull Events`、`Clear Events`（仅 debug）
- 展示：最近 N 条事件（type/message + payloadJson 截断）

### 5.4 AgentSpace/ToolKit（本地执行兜底）
- 按钮：`Ensure VTS`、`Launch App`、`Screenshot`（已有可复用）
- 目的：当 MCP/A2A 未接通时，仍可单测本地执行链路

---

## 6. 开发节奏（分阶段交付 + 每阶段可手动验收）

> 原则：每阶段都要“可跑、可看、可回滚”；优先通过 Debug Tab 验收，必要时辅以仓库已有脚本。
> 本节采用 **Option A** 顺序：先打通工具链路（Dispatcher → WS），再打通 A2A（send → stream），最后切主 UI 链路。

### Phase 0：骨架与开关（1–2 天）
交付：
- 引入 `AgentEventHub` 新事件类型（不影响旧链路）
- 新增 settings：A2A baseUrl、MCP wsUrl、deviceId、contextId
- Debug Tab 增加“配置展示 + 事件列表框架”

手动验收：
- 打开 Index Debug：能保存/读取新配置；能看到空事件队列；旧功能不回归。

### Phase 1：ToolCallDispatcher（不接 WS，先把本地工具闭环打通）（1–2 天）
交付：
- 实现 `ToolCallDispatcher`：name/args → 执行 → screenshot_b64 → result
- 支持最小工具集：`screenshot/click/type/swipe/launch_app/wait/press_enter`（press_back 可先失败）
- Debug Tab 增加按钮：`Simulate tool_call`

手动验收：
- 在 Debug Tab 点 `Simulate tool_call: screenshot`：返回 SUCCESS 且 screenshot_len>0
- 点 `Simulate tool_call: click(point=[500,500])`：返回 SUCCESS 且 screenshot_len>0

### Phase 2：DeviceToolsEndpoint（接入 WS，打通 proxy→device→proxy）（1–2 天）
交付：
- `McpWsClient` + `DeviceToolsEndpoint`：connect/hello/recv tool_call/send tool_result
- 串行执行 tool_call；错误也回 screenshot_b64
- Debug Tab 增加：`Connect/Disconnect/MCP Status`

手动验收（两种任选其一）：
1) Debug Tab 验收（最优先）
   - 配置 wsUrl/deviceId/sessionId，点 Connect，看到 connected 状态与心跳日志。
2) 脚本验收（端到端更强）
   - 启动 `harmony_mcp_proxy`
   - 端侧连接 WS 后，在电脑运行：
     - `python harmony_mcp_proxy/validate_proxy.py --tool screenshot`
   - 期望：命令输出 `tools_call_ok=true` 且 screenshot_len>0（表明 WS 通路 + screenshot 工具都可用）

### Phase 3：A2A message:send（blocking）先跑通任务首轮/续跑（1–2 天）
交付：
- A2A HTTP client：支持 `message:send`（首轮 + 带 taskId 的 resume）
- 组装 parts：Text + File(screenshot bytes b64)；metadata.installed_apps（首轮）
- Debug Tab 增加：`Send (blocking)` / `Resume (blocking)` 按钮

手动验收：
- 启动 mock A2A（本地替身）：
  - `python scripts/a2a_mock_server.py --host 0.0.0.0 --port 8080`
- 端侧 Setting：`A2A Base URL = http://<host>:8080`
- 用一个会触发 `input-required` 的 prompt：能拿到 Task.status.state=input-required、能从 status.message 拿到提示文案
- 输入 answer 后 Resume：能进入 completed/failed

### Phase 4：A2A message:stream（SSE）与事件映射（2–3 天）
交付：
- SSE 行级解析（只吃 `data:` JSON）
- 将 `statusUpdate/artifactUpdate` 映射为 `AgentEventHub` 事件
- Debug Tab 增加：`Start Stream/Stop Stream`，并可看到 delta/trace 逐条追加

手动验收：
- 不接 agent_v3 时，可用 mock A2A：
  - `python scripts/a2a_mock_server.py --host 0.0.0.0 --port 8080`
  - Debug Tab 填 `stream prompt`，点 `Start Stream`，再点 `拉取A2A事件`
  - 期望：出现 `assistant.delta` 与 `tool.trace` 事件；终态为 `completed` 或 `input_required`
- 使用 `agent_v3/scripts/e2e_phase4_acceptance.py` 的提示语作为测试 prompt（在 Debug Tab 发起 stream）
- 期望：能看到 `thinkflow.delta`（agent.description）与 `tool.trace`（如果云侧确实调用工具），并最终进入 completed 或 input-required。

### Phase 5：主链路切换（FloatChatPage/Viewer 事件驱动）（2–4 天）
交付：
- FloatChatPage：`START_TASK_TEXT` 走 A2A stream 首轮
- Viewer：不再依赖 snapshot.lastDescription；改为消费事件（status/assistant.delta/input_required）
- `SUBMIT_USER_REPLY`：转为 A2A resume（TextPart + Screenshot）

手动验收：
- 浮窗发起任务：Viewer 能实时显示状态文案（来自 event）
- 命中 input-required：Viewer 弹出交互（来自 statusUpdate.message/metadata.action），提交后能续跑并完成

### Phase 6：收尾与回滚策略（1–2 天）
交付：
- 主链路彻底 A2A-first：`startTaskWithDefaults()` 不再自动 fallback 到 `/invoke`
- A2A → `TaskSnapshot` 投影（用于 `getSnapshotJson()` / LiveView / AgentSpace 释放等旧能力复用）
- `stopTask()` 对 A2A 生效：停止读流 + 断开 MCP WS（阻断 tool_call）+ 释放资源 + 更新 LiveView
- LiveView 跟随 A2A 状态推进（working/input-required/completed/failed），并支持“关闭 LiveView → stopTask”
- MCP WS 生命周期明确：每次发起/续跑 A2A 前 `ensureMcpConnected()`（含 contextId override），Stop/销毁 AgentSpace 时自动断开
- 文档更新（本文件 + ARCHITECTURE_CURRENT 对齐说明）

手动验收：
- Setting 配置：
  - `A2A Base URL` 指向可用 runtime（mock 或 agent_v3）
  - `MCP WS` 指向 `harmony_mcp_proxy` 的 `/ws`
- 浮窗发起任务：
  - 观察 Debug 页 `MCP 日志` 出现 `ws_open/send_hello`
  - Viewer 过程中能逐条看到 description 与 tool.trace，完成后出现最终输出
  - LiveView：running →（命中 input-required 时）waiting → finished/failed（并在 30s 内自动关闭）
- 关闭 LiveView：任务应 stop（A2A stop + MCP 断开），且不再继续触发 tool_call
- Viewer 点“停止任务”：同上（stop 生效）

---

## 7. 已知风险与约束（提前写明，避免实现时走偏）

- **停止任务**：A2A v0.3.0 子集未定义 cancel；端侧 stop 只能停止本地读流/停止执行工具，云侧可能因工具超时而 failed（需要产品定义）。
- **并发限制**：同一 session 同时发起多条 run 会触发 `SESSION_BUSY(409)`；UI 必须提示并避免并发 start/resume。
- **tool.trace 打码**：端侧不要依赖 tool.trace 做状态闭环；仅用于展示与观测。
- **press_back**：本轮 Phase 6 不要求补齐，后续如要支持需补齐 VTS 注入能力（且仍需携带 screenshot_b64）。
