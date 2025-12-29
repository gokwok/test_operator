# AgentService（ServiceExtAbility）设计与里程碑（R0–R6）

目标：把“云侧 Agent（/invoke 单步决策）↔ 端侧执行（VTS/注入/截图）↔ 回传 action_result + 最新截图”的全流程，迁移到 **ServiceExtAbility** 中长期运行；前台 UI（Index / Viewer）只负责调试、可视化与用户交互（call_user/interact/task_over、权限引导）。

约束（沿用当前项目原则）：
- 最简实现：不引入不必要的抽象层；本地以**单例**方式管理，`AgentSpace` 只有 1 个。
- Viewer 仅用于**前台可视化/接管**；应用后台执行任务时，不依赖 Viewer。
- ArkTS 代码风格：避免 `any/unknown`、避免“对象字面量作为类型”、避免索引签名（`Record<string, ...>`/`[k: string]`）。

参考协议：`/Users/gokwok/code/agent_v3/docs/api.md`（Thinkflow 信封 + Action Space）。

---

## 1. 总览架构

### 1.1 核心模块与职责（最简分层）

1) **AgentServiceExtAbility（入口/生命周期）**
- 创建并持有 `AgentService`（单例）。
- 提供 IPC（RemoteObject）给 UIAbility/ViewerAbility 调用。
- 不承载“循环逻辑”，只做路由与生命周期托管。

2) **AgentService（单例聚合点）**
- 持有：`TaskManager`、`AgentSpace`、`TaskStore`、（可选）`CloudAgentClient`。
- 对外提供：开始/停止任务、查询状态、拉取事件、提交用户回复、Viewer surface/事件转发等。

3) **TaskManager（单任务状态机，建议需要）**
- 只管理“一个任务/一个 session_id”的全流程：`/invoke → execute → screenshot → /invoke ...`。
- 维护任务状态（running/waiting_user/finished/failed 等）、最近一次 action 与结果、重试/退避。
- 输出“任务事件队列”，供 UI 拉取展示。

4) **DeviceExecutor（端侧动作执行器，保持很薄）**
- 把云侧 `action` 映射为端侧执行：调用 `AgentSpace.executeAction(...)` 或更底层能力。
- 统一默认参数、坐标处理、错误转换（返回 ActionResult）。

5) **CloudAgentClient（云侧调用器）**
- 实现 `POST /invoke`、（可选）`GET /events` 断线恢复。
- 不关心端侧执行细节，只收发协议字段。

6) **TaskStore（最小持久化）**
- 只存：`session_id/run_id/cursor/installed_apps_sent/...` 等可恢复关键字段。
- 不存 base64 截图（太大）；截图只在内存/文件临时持有用于 debug。

7) **IPC（UI ↔ Service）**
- UIAbility/ViewerAbility 通过 `connectServiceExtensionAbility` 获取 RemoteObject。
- UI 通过 `pullEvents(lastSeq)` 轮询增量事件（沿用你现有 viewer effect 的设计风格）。

### 1.2 运行时依赖与边界

- **后台执行不依赖 Viewer**：任务循环在 ServiceExtAbility 运行；Viewer 只在前台需要观看/接管时启动并 attach surface。
- **权限/交互由 UI 承接**：user_grant 权限弹窗在 Service 内不可靠（你已验证）。因此 Service 只发出 “need_permission/need_user_input” 事件；UI 去打开设置页/收集输入，再回传到 Service。
- **Context 类型注意**：当前 `AgentSpace.init(ctx: common.UIAbilityContext)` 未来迁入 ServiceExtAbility 时，需要允许 `UIExtensionContext`（建议改为 `common.Context` 或 `common.UIAbilityContext | common.UIExtensionContext`，保持最小改动）。

---

## 2. 目录规划（建议）

建议新增一个独立 feature：`features/agent_service`，把 ServiceExtAbility + 云侧闭环集中管理：

```
entry/src/main/ets/
  serviceextability/
    AgentServiceExtAbility.ets

  features/
    agent_service/
      AgentService.ets
      TaskManager.ets
      DeviceExecutor.ets
      CloudAgentClient.ets
      TaskStore.ets
      types.ets
      ipc/
        AgentServiceProtocol.ets
        AgentServiceStub.ets
        AgentServiceClient.ets

    agentspace/              # 复用现有（VTS/注入/截图/Viewer effects）
      AgentSpace.ets
      VTSBackend.ets

  pages/
    Index.ets                # 调试按钮（连接/启动任务/停止/状态/交互回传）
    AgentSpaceViewer.ets      # surface + 可视化/控制（通过 IPC）
```

---

## 3. 数据模型（建议最小集合）

> 目标：结构稳定、字段显式、便于 IPC 与 UI 展示；不跨进程传 screenshot base64。

### 3.1 TaskSnapshot（UI 可展示）
- `sessionId: string`
- `status: 'idle' | 'running' | 'waiting_user' | 'stopping' | 'finished' | 'failed'`
- `stepIndex: number`
- `lastDescription: string`
- `lastActionName: string`
- `lastActionJson: string`（可选：action.parameters 的 JSON；避免复杂 Parcel）
- `lastResultStatus: 'SUCCESS' | 'FAILED' | 'SKIPPED' | ''`
- `lastError: string`
- `installedAppsSent: boolean`
- `viewerActive: boolean`（从 AgentSpace runtime 拉取）

### 3.2 TaskEvent（增量事件）
- `seq: number`
- `type: 'status' | 'log' | 'need_user' | 'need_permission' | 'action' | 'result' | 'error'`
- `message: string`
- `payloadJson?: string`（可选）

### 3.3 UserReply（UI → Service 回传）
- `kind: 'call_user' | 'interact' | 'task_over' | 'permission_granted'`
- `content: string`（用户文本/选项/说明）

---

## 4. IPC 设计（最简且稳定）

原则：
- IPC 只传小数据：**不传截图 base64**、不传大数组（installed apps 可在 UI 获取后只传一次摘要或 JSON）。
- 为了避免 ArkTS “对象字面量类型/索引签名”问题：**复杂结构统一用 JSON string** 传输。
- Service 为“权威状态源”，UI 通过 `getSnapshot/pullEvents` 驱动展示。

### 4.1 建议暴露的方法（第一版）

基础连接：
- `ping(): string`（返回版本/时间戳）
- `getSnapshot(): string`（TaskSnapshot JSON）
- `pullEvents(lastSeq: number): string`（EventBatch JSON；为空返回 `''`）

任务控制：
- `startTask(sessionId: string, taskText: string, optionsJson: string): void`
- `stopTask(reason: string): void`
- `submitUserReply(replyJson: string): void`

Viewer（R3 开始启用）：
- `setViewerActive(active: boolean, mode: 'display' | 'control'): void`
- `attachViewerSurface(surfaceId: string): void`
- `sendViewerPointerEvent(eventJson: string): void`
- `pullViewerEffects(lastSeq: number): string`（可复用你现有 AgentSpace viewer effects 结构，返回 JSON）

> 说明：这套接口既能支持 Index 调试，也能支持独立 ViewerAbility。后续迁移到 ServiceExtAbility 不需要改 UI 侧调用形态。

---

## 5. 任务闭环（与 api.md 对齐）

### 5.1 单步循环（TaskManager.tick）
1. 采集 `DeviceState`：
   - `screenshot`：来自 `AgentSpace.captureScreenshotBase64()`（仅在内存）
   - `installed_apps`：首轮（或变化时）上传；建议由 UI 获取并通过 `startTask/optionsJson` 传入
   - `user_note`：由 UI 提交到 Service（submitUserReply）后注入下一轮
2. 调用云侧：`CloudAgentClient.invoke(session_id, payload)`
3. 得到 `action + description`
4. `DeviceExecutor.execute(action)`：
   - click/double/long/swipe/type/wait/launch_app（按你 api.md）
5. 形成 `action_result`
6. 进入下一轮 `/invoke`（携带 `action_result + 最新 screenshot`）
7. 遇到 `finish`：置 `finished` 并停止循环

### 5.2 人机交互动作（call_user/interact/task_over）
当云侧返回这些 action：
- TaskManager 不执行端侧动作，改为：
  - 置状态 `waiting_user`
  - 发出事件 `need_user`（payloadJson 包含提示文案/选项）
- UI 展示控件收集输入后调用 `submitUserReply(...)`
- TaskManager 收到回复后：
  - 组装下一轮 `action_result.status = 'SKIPPED'`，`output = user_reply`
  - 继续 tick

### 5.3 错误与恢复（最小策略）
- `409 SESSION_BUSY`：指数退避后重试；或走 `/events` 获取 `output.final`（可在 R4 之后补）
- `408/502`：退避重试 N 次；超过阈值进入 `failed` 并发事件
- 端侧注入失败：`action_result.status='FAILED'`，写入 `error`，继续下一轮让云侧自我纠偏

---

## 6. 本地持久化（R6）

推荐用 Preferences 做最小持久化：
- `sessionId`
- `lastRunId`（用于 /events 恢复）
- `eventsCursor`（用于 /events 增量恢复）
- `installedAppsSent`
- `lastActionName/lastActionJson/lastResultStatus/lastError`（可选，便于调试与续跑）

不持久化：
- `screenshot base64`（太大）
- `viewer surfaceId`（进程内资源，重启无意义）

恢复策略（R6）：
- Service 启动时加载 store，恢复 `sessionId/installedAppsSent/...`
- UI 提供 “Resume” 调试按钮（调用 `startTask` 或 `resumeTask`，你可选是否单独提供）

---

## 7. 里程碑（R0–R6）与验证方式

> 验证原则：每个里程碑都能通过 `Index.ets` 的 debug 按钮 + 日志完成验收；你在具备权限的环境用 hvigor 编译与真机验证。

### R0：ServiceExtAbility 骨架 + IPC 可连接
实现：
- 新增 `AgentServiceExtAbility`，导出 RemoteObject。
- 新增 `AgentServiceClient`（UI 侧连接/调用）。
- Index 增加按钮：Connect / Ping / Snapshot。

验证：
- 点击 Connect 成功，Ping 返回非空字符串。
- Snapshot 返回默认 idle 状态。

### R1：TaskManager（单任务状态机）+ 事件队列
实现：
- `TaskManager.start/stop/getSnapshot/pullEvents`。
- 先用“假任务”推进 stepIndex（不接云，不执行注入）。

验证：
- Start 后 UI 可看到状态 running、stepIndex 递增。
- PullEvents 不重复（按 lastSeq 增量）。
- Stop 能进入 idle/stopping 并停止推进。

### R2：接入 AgentSpace + DeviceExecutor（不接云）
实现：
- Service 中初始化 `AgentSpace`（迁移初始化位置：从 EntryAbility → Service）。
- `DeviceExecutor` 支持：ensureVts/launch/click/screenshot/wait/stop 等最小子集。
- Index 按钮改为走 IPC：Create/Launch/Click/Screenshot/Stop。

验证：
- 与当前本地实现一致：Create→Launch→Click→Screenshot 可用。
- UI 切后台后（不打开 Viewer）仍可执行 click/screenshot（如果你设备允许 headless）。

### R3：Viewer 与 Service 通信（display/control）
实现：
- ViewerAbility/AgentSpaceViewer 页面连接 service。
- surfaceReady → IPC attach；control 模式把触摸事件 IPC 转发。
- Viewer effects 通过 `pullViewerEffects` 获取并渲染（保持你现在的动画体系）。

验证：
- Viewer 可看到 VTS 画面。
- control 模式触控能在 VTS 上生效，并出现 InteractionEffectOverlay 动画。

### R4：接入 CloudAgentClient（/invoke 单步闭环）
实现：
- `CloudAgentClient.invoke(sessionId, payload)` 对齐 `api.md`。
- TaskManager.tick：截图→invoke→execute→action_result→下一轮。

验证：
- Index 提供：Start Cloud Task（输入 taskText/sessionId）按钮。
- 日志能看到每轮 `description/action/action_result`。
- 遇到失败能回传 FAILED 并继续下一轮。

### R5：交互动作闭环（call_user/interact/task_over）
实现：
- TaskManager 遇到交互 action → `waiting_user` + 事件。
- Index 弹窗收集用户输入/选项，并 `submitUserReply` 回传。

验证：
- 云侧返回 `call_user/interact/task_over` 时，端侧暂停并弹出 UI。
- 提交后继续执行下一轮。

### R6：最小持久化 + 恢复
实现：
- `TaskStore` 写入关键字段；Service 重启读取恢复。
- 可选：支持 `/events` 恢复 output.final（对齐 `api.md` 5.5）。

验证：
- 强杀进程/重启应用后，Snapshot 仍包含上次 sessionId/stepIndex（或恢复到可续跑状态）。
- Resume 后不会重复执行已完成的动作（如引入 /events 则验证不重复规划）。

---

## 8. Debug 按钮建议（Index.ets）

最少保留这些入口（按里程碑逐步点亮）：
- Service：Connect / Ping / Snapshot / PullEvents
- Task：Start / Stop / SubmitUserReply（测试 waiting_user）
- AgentSpace：Create / Launch / Click preset / Swipe / Screenshot
- Viewer：Show Viewer / Close Viewer / Show Control Viewer（可选）

---

## 9. 关键注意事项（踩坑清单）

- **权限弹窗**：user_grant 权限在 Service 内常常不弹；按设计交给 UI 承接，Service 只发 `need_permission` 事件。
- **IPC 传输**：避免传 screenshot base64；复杂结构用 JSON string，减少 ArkTS 类型限制问题。
- **并发控制**：TaskManager 内部要有“运行令牌/互斥锁”，避免 tick 重入（比如 UI 连点 Start、或 timer 与回调交错）。
- **Context 类型**：ServiceExtAbility 的 context 与 UIAbilityContext 不同；迁移时优先把 AgentSpace/VTSBackend 的 context 类型放宽到 `common.Context`（保持最小改动）。

