# 当前项目架构（ArkTS 端 · 现状说明）

本文档面向**新加入项目的开发/Agent**，用于快速建立对当前鸿蒙（ArkTS）工程的整体心智模型（不仅限 Accessibility）。

> 说明：`docs/ARCHITECTURE_DESIGN.md` 是“整改/对齐”的目标架构规范；本文档描述**当前代码实际实现**与关键链路。

补充（vNext 已落地的部分）：当前主链路已切换为 **A2A-first + MCP Tools**（事件驱动展示、MCP 下发工具调用），`TaskManager(/invoke loop)` 仅作为遗留模块保留（不再作为默认链路）。

---

## 1. 项目在做什么（高层概览）

一句话：这是一个“**云端编排 + 本地执行**”的操作型智能体 App，通过 **ServiceExtensionAbility 常驻后台**驱动任务循环，在 **虚拟屏（AgentSpace / VTS）**里启动并操作目标应用，同时支持：

- **浮窗输入**：用户随时发起任务/补充信息（`FloatWindowAbility` + `FloatChatPage`）。
- **Viewer 展示与接管**：前台展示虚拟屏画面，支持手动触控接管（`AgentSpaceViewerAbility` + `AgentSpaceViewer`）。
- **Debug 页调测**：提供大量 IPC 调试按钮、历史与配置（`EntryAbility` + `Index`）。
- **无障碍树获取**：通过 `AccessibilityExtensionAbility` 获取指定 display/窗口的可访问树，并提供树/窗口/列表/属性调试输出（`AgentAccessibilityExtAbility`）。

核心边界：**UI 负责交互与展示；后台 Service 负责任务执行；虚拟屏承载被操作应用；IPC 负责跨 Ability 通信；StateCollector 负责把“设备状态”拼给云端。**

---

## 2. 目录结构与模块边界

入口代码主要集中在：

```
entry/src/main/ets/
  abilities/                 # 系统入口（UIAbility/ServiceExtension/AccessibilityExtension）
  ui/
    pages/                   # 页面容器（Index / FloatChatPage / AgentSpaceViewer）
    views/                   # 可复用组件（XComponent、浮层、输入条等）
  features/
    agent_controller/        # 核心域：任务控制、虚拟屏、IPC、状态采集、LiveView
    persistence/             # 偏业务无关：settings/history
    voice_kit/               # ASR/TTS
```

系统/权限声明在 `entry/src/main/module.json5`，包括 abilities/extensionAbilities、权限申请与 metadata（如无障碍配置）。

---

## 3. Abilities（系统入口）一览

### 3.1 UIAbility

- `entry/src/main/ets/abilities/EntryAbility.ets`
  - App 主入口，加载 `ui/pages/Index`（配置/历史/Debug）。
- `entry/src/main/ets/abilities/AgentSpaceViewerAbility.ets`
  - Viewer 入口，加载 `ui/pages/AgentSpaceViewer`。
  - 在生命周期中通过 IPC 调用 `setViewerActive(false)`，用于 Viewer 退出时通知后台释放/降级展示。

### 3.2 ServiceExtensionAbility

- `entry/src/main/ets/abilities/AgentServiceExtAbility.ets`
  - 后台“执行引擎”入口：创建并初始化 `AgentController` 单例，并通过 `AgentControllerStub` 暴露 IPC。
  - 支持 `onRequest(action=start_task)` 的兜底启动方式（浮窗若 RPC 连接失败会 fallback 到这里）。
- `entry/src/main/ets/abilities/FloatWindowAbility.ets`
  - 浮窗能力：创建 `TYPE_FLOAT` Window 并加载 `ui/pages/FloatChatPage`。
  - 支持 `show/hide/toggle` 控制浮窗展示。

### 3.3 AccessibilityExtensionAbility

- `entry/src/main/ets/abilities/AgentAccessibilityExtAbility.ets`
  - 无障碍扩展：监听 CommonEvent 的请求事件，按 request 中的 displayId/bundleName 等参数抓取窗口/树，组装并回传。
  - 同时支持输出“树 JSON / 窗口 JSON / 列表 JSON / 节点属性调试文本”，并在 payload 大小超限时做截断。

### 3.4 BackupExtensionAbility

- `entry/src/main/ets/abilities/EntryBackupAbility.ets`
  - 目前为占位实现。

---

## 4. 核心域：AgentController（features/agent_controller）

`AgentController` 是整个系统的**门面（Facade）**与编排中心：对外提供 IPC 接口，对内组织任务循环、虚拟屏、工具执行、状态采集、LiveView 等子模块。

### 4.1 门面入口

- `entry/src/main/ets/features/agent_controller/AgentController.ets`
  - 单例：在 `AgentServiceExtAbility.onCreate()` 注入 context 与 `startAbilityInvoker`。
  - 关键职责：
    - **任务生命周期**：`startTask/startTaskWithDefaults/stopTask`。
    - **AgentSpace 生命周期**：创建、销毁、Viewer 绑定、切回主屏等。
    - **对外 IPC 方法**：截图分片、启动应用、Viewer 事件、无障碍树 dump 等。

### 4.2 任务循环与云端编排

- vNext 主链路（A2A-first + MCP Tools）：
  - `entry/src/main/ets/features/agent_controller/a2a/A2AClient.ets`、`entry/src/main/ets/features/agent_controller/a2a/A2AStreamSession.ets`
    - 负责 `message:stream` SSE：发起任务/续跑、接收事件流（statusUpdate / artifactUpdate）。
  - `entry/src/main/ets/features/agent_controller/mcp/DeviceToolsEndpoint.ets`
    - 通过 WebSocket 连接 `harmony_mcp_proxy`，接收 `tool_call`，执行后回 `tool_result`（必须携带 `screenshot_b64`）。
  - `entry/src/main/ets/features/agent_controller/events/AgentEventHub.ets`
    - 端侧统一事件总线，供 UI 拉取并驱动渲染（description/tool.trace/output 等）。

- Legacy（不再是默认链路，仅保留）：
  - `entry/src/main/ets/features/agent_controller/task_manager/TaskManager.ets`
    - 旧状态机与 `/invoke` loop：截图 → invoke → 执行动作 → 再截图 → invoke(action_result) → …
  - `entry/src/main/ets/features/agent_controller/AgentProxy.ets`
    - HTTP `/invoke` 客户端（CloudAction → AgentAction）。

### 4.3 虚拟屏执行（AgentSpace / VTS）

- `entry/src/main/ets/features/agent_controller/agentspace/AgentSpace.ets`
  - 封装虚拟屏运行时：`ensureVts/attachViewerSurface/startApp/click/swipe/screenshot` 等。
  - 维护 Viewer 状态（active/mode/surfaceAttached），并维护“动效队列”（tap/long/swipe）供 Viewer 拉取渲染。
- `entry/src/main/ets/features/agent_controller/agentspace/VTSBackend.ets`
  - 具体实现：`screen.createVirtualScreen` 创建虚拟屏、`setVirtualScreenSurface` 绑定渲染 surface、在指定 displayId 启动应用。
  - 注入输入：点击/长按/滑动、IME 输入（依赖 `@hms.ai.appController`）。

### 4.4 动作执行（ToolKit）

- `entry/src/main/ets/features/agent_controller/toolkit/ToolKit.ets`
  - AgentAction 执行适配层：处理 `launch_app`（展示名 → AppTarget）、坐标归一化→像素等，再委托 `AgentSpace` 执行。
  - 维护“AgentSpace 当前前台应用 bundleName”（用于 a11y / state 上报）。

### 4.5 状态采集（StateCollector）

- `entry/src/main/ets/features/agent_controller/state_collector/StateCollector.ets`
  - 组装云端需要的 `DeviceState`：截图、系统前台包名、AgentSpace 前台包名、installed apps、user_note 等。
  - 同时提供 debug/a11y dump：面向 AgentSpace 的 displayId + bundleName 拉取无障碍树/列表/窗口/属性。

### 4.6 LiveView（系统通知）

- `entry/src/main/ets/features/agent_controller/live_view/LiveViewController.ets`
  - 负责 System LiveView 的 subscribe/publish/update/stop，以及 cancel 回调监听。

### 4.7 IPC（UI <-> Service）

- `entry/src/main/ets/features/agent_controller/ipc/AgentControllerProtocol.ets`
  - RequestCode 枚举（新增 IPC 接口时需同步更新）。
- `entry/src/main/ets/features/agent_controller/ipc/AgentControllerStub.ets`
  - Service 侧实现：把 request code 分发到 `AgentController` 对应方法。
- `entry/src/main/ets/features/agent_controller/ipc/AgentControllerClient.ets`
  - UI 侧调用封装：负责 connectServiceExtensionAbility，并提供强类型方法调用（如 `screenshot()`、`dumpAccessibilityTreeList()`）。

---

## 5. 关键链路（端到端）

### 5.1 发起任务（浮窗）→ 后台执行

1) `FloatWindowAbility` 创建浮窗，加载 `ui/pages/FloatChatPage`  
2) `FloatChatPage` 通过 `AgentControllerClient` 连接 `AgentServiceExtAbility`  
3) IPC 调用 `START_TASK_TEXT` → `AgentController.startTaskWithDefaults()`  
4) `AgentController`：
   - 加载 `AgentSettings`
   - reset `AgentSpace`（虚拟屏环境）
   - `ensureMcpConnected()`：建立 MCP WS（后续云侧 tool_call 会通过该连接下发）
   - `A2A message:stream`：携带“用户指令 + 当前截图（base64）+ installed_apps”发起任务
5) 运行推进（vNext）：
   - A2A SSE（`statusUpdate/artifactUpdate`）→ `AgentEventHub` → Viewer/Debug 渲染
   - 云侧触发工具时：`harmony_mcp_proxy` → MCP WS `tool_call` → `DeviceToolsEndpoint/ToolCallDispatcher` 执行动作并回 `tool_result`（含 screenshot_b64）

> Legacy（保留但非默认）：旧 `/invoke` loop 仍在 `TaskManager` 中，可用于对照排障。

### 5.2 Viewer 展示与手动接管

1) 打开 `AgentSpaceViewerAbility`（UIAbility）加载 `ui/pages/AgentSpaceViewer`  
2) `AgentSpaceViewer`：
   - IPC 调用 `setViewerActive(true)`，并通过 `AgentSpaceDisplay(XComponent)` 上报 surfaceId
   - IPC 调用 `attachViewerSurface(surfaceId)` 把虚拟屏画面渲染到 XComponent
3) 触控事件：
   - `AgentSpaceDisplay.onTouch` 归一化坐标 → IPC `sendViewerEvent`
   - Service 侧注入到 `AgentSpace/VTSBackend`，并产生动效（tap/long/swipe）供 Viewer overlay 渲染

### 5.3 Debug 页（Index）

- `EntryAbility` 加载 `ui/pages/Index`
- Index 的 debug tab 通过 `AgentControllerClient` 直接调用 Service 侧方法：
  - VTS 初始化/销毁、打开 Viewer、按应用名启动、截图、前台应用、定位、无障碍树/窗口/列表/属性等
- 截图采用“长度 + 分片”方式（避免单次 IPC payload 过大）：`SCREENSHOT` / `SCREENSHOT_CHUNK`

### 5.4 无障碍树获取（虚拟屏）

目标：获取 **虚拟屏 displayId** 中 **AgentSpace 前台应用 bundleName** 对应窗口的可访问树/列表。

- 调用侧（Service）：
  - `StateCollector.dumpAccessibilityTree*ForAgentSpace()` 计算 displayId（优先 `runtime.displayId`，否则 fallback `screenId`）
  - 通过 `AccessibilityTreeClient` 发布请求 CommonEvent（见 `entry/src/main/ets/features/agent_controller/state_collector/accessibility/AccessibilityTreeClient.ets`）
- 扩展侧（AccessibilityExtension）：
  - `AgentAccessibilityExtAbility` 订阅请求事件，调用 `this.context.getWindows(displayId)` / `getWindowRootElement(...)` 遍历树
  - 根据请求参数返回：
    - `DUMP_ACCESSIBILITY_TREE`：树结构 JSON
    - `DUMP_ACCESSIBILITY_WINDOWS`：窗口元数据 JSON
    - `DUMP_ACCESSIBILITY_TREE_LIST`：扁平列表 JSON（更适合给智能体/调试展示）
    - `DUMP_ACCESSIBILITY_NODE_ATTRS`：调试文本（每节点可读属性键值）
- 传输约束：
  - CommonEvent payload 有长度限制，`AgentAccessibilityExtAbility` 内部会按 `MAX_PAYLOAD_LENGTH` 截断并标记 `truncated=true`

相关协议与格式化：
- `entry/src/main/ets/features/agent_controller/state_collector/accessibility/AccessibilityTreeProtocol.ets`
- `entry/src/main/ets/features/agent_controller/state_collector/accessibility/AccessibilityTreeFormatter.ets`

---

## 6. 数据与持久化

- 配置（Preferences）
  - `entry/src/main/ets/features/persistence/settings/AgentSettings.ets`
  - 保存云端地址、wait_ms、LiveView/历史开关等（Index 页可编辑）
- 历史（distributedKVStore）
  - `entry/src/main/ets/features/persistence/history/TaskHistoryStore.ets`
  - 以 task/step/image chunk 形式存储，Index 页支持浏览 step 截图与动作/结果

---

## 7. 语音能力（voice_kit）

- ASR：`entry/src/main/ets/features/voice_kit/Asr.ets`
  - `FloatChatPage` 用于按住说话/自动识别，最终合并为输入文本并发起任务
- TTS：`entry/src/main/ets/features/voice_kit/Tts.ets`
  - 目前提供测试/示例封装

---

## 8. 常见坑与实现约束（务必先读）

1) **ArkTS 编译约束严格**  
避免 `any/unknown`、结构化类型、utility types、索引签名/按索引访问字段、`Function.call/apply` 等（项目中已有多次踩坑记录）。

2) **虚拟屏 vs 主屏**  
与 AgentSpace 相关的能力（截图、无障碍树、打开应用）必须使用 `VTSBackend` 的 `runtime.displayId/screenId`，不要误用系统默认 display。

3) **IPC/事件 payload 大小限制**  
截图使用分片；无障碍树使用“扩展侧截断 + 列表输出”；新增调试输出时优先考虑长度上限。

4) **Service 连接可靠性**  
UI 通过 `connectServiceExtensionAbility` 拿 RemoteObject；浮窗发起任务有 fallback：`AgentServiceExtAbility.onRequest(action=start_task)`。

---

## 9. 新功能应该加在哪里（落点指南）

### 9.1 新增“云端动作”支持

1) 云侧返回 action → `AgentProxy.normalizeAction()` 能解析到 `AgentAction`  
2) 本地执行：优先在 `ToolKit.executeAction()` 做参数适配（尤其 app 名映射/坐标换算），再交给 `AgentSpace.executeAction()` / `VTSBackend` 落地

### 9.2 新增 Debug 按钮 / IPC 接口

1) `AgentControllerProtocol.ets` 增加 request code  
2) `AgentControllerStub.ets` 增加分发分支  
3) `AgentControllerClient.ets` 增加调用封装  
4) `AgentController.ets` 增加真正实现  
5) `ui/pages/Index.ets` debug tab 增加 UI 与结果展示

### 9.3 新增“状态字段”上报给智能体

- 在 `features/agent_controller/types.ets` 扩展 `DeviceState`
- 在 `StateCollector.buildState()` 填充字段

### 9.4 Viewer 新的展示/动效

- Service 侧：`AgentSpace` 产出 effect batch（sequence + effects）
- UI 侧：`AgentSpaceViewer` 轮询 `PULL_VIEWER_EFFECTS` 并渲染 overlay

---

## 10. 建议的阅读顺序（5 分钟入门）

1) `entry/src/main/module.json5`（能力与权限声明）  
2) `features/agent_controller/AgentController.ets`（总控入口）  
3) `features/agent_controller/task_manager/TaskManager.ets`（任务循环）  
4) `features/agent_controller/agentspace/AgentSpace.ets` + `agentspace/VTSBackend.ets`（虚拟屏执行）  
5) `ui/pages/FloatChatPage.ets` + `ui/pages/AgentSpaceViewer.ets`（发起任务/展示接管）  
6) `ui/pages/Index.ets`（调试与历史）  
7) `abilities/AgentAccessibilityExtAbility.ets` + `state_collector/accessibility/*`（无障碍链路）
