# 项目架构设计（整改对齐文档）

本文档用于规范化与模块化当前项目架构，目标是在**保留现有功能**的前提下，为后续“目录结构整改 + 模块边界收敛 + MVVM 结合”提供统一标准与对齐依据。

---

## 0. 目标与约束

### 0.1 目标
- 明确分层（系统入口 / UI / 控制逻辑 Feature），减少跨层依赖与耦合。
- 支持 **ServiceExtensionAbility** 后台全链路执行（云端编排 + 虚拟屏执行 + LiveView 更新），Viewer 仅在前台展示/交互时参与。
- 支持 **每个任务从干净环境开始**：任务与 `AgentSpace(VTS)` 一一对应，任务结束释放资源。
- 保留现有能力：云端动作执行、Viewer 展示/交互、LiveView、配置持久化、历史任务 KV 存储、ASR/TTS 能力等。

### 0.2 约束
- **最简化实现**：避免引入过多抽象层；以“门面（Facade）+ 领域模块”实现为主。
- ArkTS 编译约束：避免结构化类型、utility types、spread、catch 类型标注等。
- Index 页面：仅作为配置与历史入口（不作为对外服务入口）。

---

## 1. 分层与目录规范（标准化）

目标目录结构（仅规范，不立即改动代码）：

```
entry/src/main/ets/
  abilities/                 # 系统入口与生命周期承载（UIAbility/ServiceExtAbility）
  ui/
    pages/                   # 页面容器（渲染+交互编排）
    views/                   # 可复用 UI 组件
  features/
    agent_controller/        # 任务控制域（核心）
    voice_kit/               # 语音域（ASR/TTS）
    persistence/             # 可选：settings/history 归档（短期可不动）
    agent_controller/
      state_collector/       # 设备/应用状态数据源（如 AppState/StateCollector）
```

> 现阶段项目已包含 `abilities/`, `pages/`, `view/`, `features/`；后续整改建议把 `pages/view` 合并到 `ui/pages` 与 `ui/views`，并把 `agent_service` 重命名/迁移到 `agent_controller`。

---

## 2. abilities 层规范

### 2.1 abilities 的职责
- 只负责系统回调（onCreate/onRequest/onConnect/onDestroy 等）、Window 创建/销毁、把系统 `Context` 注入业务门面。
- 不写任务业务逻辑，不写云端协议处理，不写动作注入，不写历史落库。

### 2.2 abilities 列表（现状）
- `EntryAbility`：加载 `ui/pages/Index`
- `AgentServiceExtAbility`：后台服务入口（ServiceExtensionAbility），创建/持有任务控制域的门面对象（服务端）
- `FloatWindowAbility`：浮窗入口（ServiceExtensionAbility），创建 float window 并加载 `ui/pages/FloatChatPage`
- `AgentSpaceViewerAbility`：Viewer UIAbility，加载 `ui/pages/AgentSpaceViewer`
- `EntryBackupAbility`：备份扩展（占位）

---

## 3. UI 层（ui/pages + ui/views）规范

### 3.1 pages（页面容器）
pages 只做：
- 页面状态与交互编排（可通过 ViewModel 统一收敛）
- 调用 IPC Client（与 ServiceExtensionAbility 中的服务通信）
- 组合 views 并渲染

pages 不做：
- 不直接调用 Cloud API
- 不直接执行动作注入
- 不直接写 KV-Store 或 Preferences（通过 features/persistence）

页面清单（现状）：
- `Index`：配置 + 历史任务浏览（非对外入口）
- `FloatChatPage`：浮窗输入（含 ASR），发起任务
- `AgentSpaceViewer`：虚拟屏展示、交互呈现、接管/补充流程、动效展示

### 3.2 views（可复用 UI 组件）
views 只做：
- 可复用组件渲染（支持 props/controller/callback）

views 不做：
- 不发起 RPC、不访问 KV、不跑任务循环

组件清单（现状）：
- `AgentSpaceDisplay`（XComponent 承载虚拟屏）
- `StatusFloatWindow`、`FloatingOrb`、`GlowOverlay`、`InteractionEffectOverlay` 等

---

## 4. 控制逻辑 Feature：AgentController 域（核心）

### 4.1 AgentController 域的总体目标
把“云端编排 / 设备执行 / 虚拟屏 / 状态采集 / LiveView”纳入同一个任务域内管理，形成一个对外简单、对内边界清晰的模块化结构。

### 4.2 模块划分（标准化）
AgentController 域建议拆为：
- `AgentController`（门面 + 生命周期编排）
- `AgentProxy`（云端协议 + 网络）
- `TaskManager`（任务状态机 + 单步循环）
- `AgentSpace`（虚拟屏运行时 + Viewer桥接）
- `ToolKit`（动作执行器）
- `StateCollector`（设备状态采集）
- `live_view/LiveViewController`（系统 LiveView 适配器）
- `ipc/`（RPC 协议：Client/Stub/Protocol）

> 现状可映射为：`features/agent_service/AgentService ≈ AgentController`，`CloudAgentClient ≈ AgentProxy`，`TaskManager` 同名，`DeviceExecutor ≈ ToolKit`，`features/agentspace ≈ AgentSpace`，`features/live_view ≈ live_view`。

---

## 5. AgentController 域：模块职责与边界（详细规范）

### 5.1 AgentController（门面 + 生命周期编排）
只做：
- 维护 `TaskContext`（每任务一份资源集合）
- 对外提供统一接口（供 IPC / ViewModel 调用）
- 负责“干净环境策略”：任务开始前清理旧上下文，任务结束释放上下文
- 监听 TaskManager 状态变化，驱动 LiveView 更新（不直接写 UI）

不做：
- 不直接发 HTTP、不直接解析云端协议细节
- 不直接注入触控/IME（交给 ToolKit/AgentSpace）

建议对外接口（示例，不强制一次到位）：
- 任务：`startTask(...)`, `stopTask(reason)`, `getSnapshot()`, `pullEvents()`
- 交互：`submitUserReply(kind, content)`
- Viewer：`setViewerActive`, `setViewerMode`, `attachViewerSurface`, `sendViewerEvent`, `pullViewerEffects`

### 5.2 AgentProxy（云端协议与网络）
只做：
- `invoke(sessionId?, payload)`：网络请求、重试策略、超时
- 协议转换：`output.action -> AgentAction`，`ActionResult -> action_result`
- `session_id` 一致性策略：首轮可空，后续以回传 id 为准

不做：
- 不截图、不执行动作、不关心 Viewer/LiveView

### 5.3 TaskManager（任务状态机 + 单步循环）
只做：
- 维护 `TaskSnapshot` 与事件队列
- 运行固定 tick 流程（云侧约定）：
  1) 截图
  2) invoke
  3) 执行动作
  4) wait_ms
  5) 截图
  6) invoke(action_result)
- 交互动作进入 `waiting_user`，收到回复后恢复 loop
- 写入历史记录（task meta / step meta / step screenshot / step result / task final）

依赖：
- `AgentProxy`（云端）
- `StateCollector`（截图与设备状态）
- `ToolKit`（动作执行）

不做：
- 不直接操作 LiveView，不直接操作 UI

### 5.4 AgentSpace（虚拟屏运行时 + Viewer桥接）
只做：
- VTS 生命周期：`create/attachSurface/startApp/screenshot/injectPointer/destroy`
- Viewer 状态：`viewerActive/surfaceAttached/mode`
- 动效队列：提供给 Viewer 拉取（tap/long/swipe）

不做：
- 不做任务循环、不做云端 invoke、不做 installed apps 采集

### 5.5 ToolKit（动作执行器）
只做：
- 将标准 `AgentAction` 执行到设备：`launch_app/click/double/long/swipe/type/wait/press_enter/...`
- 参数解析与适配：
  - app 展示名 -> bundle/ability（resolveAppTarget）
  - 坐标归一化/像素换算
  - 默认 duration/wait_ms
- 最终通过 `AgentSpace/VTSBackend` 落地执行

不做：
- 不做云端 invoke、不驱动 UI、不写历史存储

### 5.6 StateCollector（设备状态采集）
只做：
- 产出云端所需 `DeviceState`：`screenshot`、`installed_apps`（首轮）、`user_note` 等
- 对接 `AppState` 获取应用列表/图标等（作为数据源）

不做：
- 不执行动作、不处理云端协议、不更新 UI

### 5.7 LiveView（系统 UI 适配器）
位置：`features/agent_controller/live_view/`

建议拆分：
- `LiveViewController`：只做 NotificationKit 的 publish/update/stop + cancel 监听（纯系统 UI 渲染）
- `LiveViewPresenter`（可选）：把 `TaskSnapshot` 映射为 `LiveViewContents`

原则：
- 必须在 ServiceExtAbility 后台可用
- 仅依赖 `TaskSnapshot`/任务状态，不依赖 pages/views

---

## 6. 与 MVVM 的结合方式（标准化）

### 6.1 View（ui/pages + ui/views）
- 只渲染与收集用户输入
- 不直接依赖 AgentController 的具体实现（实现运行在 ServiceExtAbility）
- 通过 IPC Client 与服务通信

### 6.2 ViewModel（建议引入，短期可先概念对齐）
- `IndexVM`：配置读写、历史任务加载/详情/截图预览
- `FloatChatVM`：输入与 ASR 状态、发起任务
- `ViewerVM`：轮询 snapshot/effects、管理交互形态机（call_user/interact/take_over/补充/接管）、回传用户结果

### 6.3 Model（领域/数据）
- `features/agent_controller/*`、`features/voice_kit/*`、`features/persistence/*`

---

## 7. 关键运行链路（对齐描述）

### 7.1 任务启动链路（浮窗）
`FloatWindowAbility` → `ui/pages/FloatChatPage` → IPC Client → `AgentController.startTask(...)`

### 7.2 任务执行链路（后台）
`TaskManager.loop`：
1) `StateCollector.screenshot()`
2) `AgentProxy.invoke(...)`
3) `ToolKit.execute(action)`（落地到 `AgentSpace/VTSBackend`）
4) wait_ms
5) screenshot
6) invoke(action_result)

### 7.3 Viewer 链路（前台可选）
`AgentSpaceViewerAbility` → `ui/pages/AgentSpaceViewer`
- `attachViewerSurface(surfaceId)` 绑定虚拟屏 surface
- `pullViewerEffects` 拉取动效数据并播放
- `getSnapshot` 拉取任务状态并切换交互形态（call_user/interact/take_over 等）

### 7.4 “每任务干净环境”策略
- `startTask` 前：销毁旧 `TaskContext`（包含 `AgentSpace/VTS`）
- 任务进入终态（failed/idle）：立即释放 `TaskContext`
- 任务进入终态（finished）：为保证 `transferToMain` 复用现有应用实例，允许延迟释放；
  典型策略是 Viewer 收起后再释放 `AgentSpace`

---

## 8. 现有代码与设计映射（用于整改迁移）
现状：
- `features/agent_service/AgentService.ets`：≈ `AgentController`
- `features/agent_service/CloudAgentClient.ets`：≈ `AgentProxy`
- `features/agent_service/TaskManager.ets`：`TaskManager`
- `features/agent_service/DeviceExecutor.ets`：≈ `ToolKit`
- `features/agentspace/*`：`AgentSpace`
- `features/live_view/LiveViewController.ets`：`LiveViewController`
- `features/settings/*`：配置持久化（Preferences）
- `features/history/*`：历史任务 KV-Store（FULL 截图）
- `features/agent_controller/state_collector/AppState.ets`：应用列表/图标数据源
- `features/Asr.ets`、`features/Tts.ets`：≈ `VoiceKit`
- `pages/*` 与 `view/*`：≈ `ui/pages` 与 `ui/views`

整改建议（保持功能不变）：
1) 目录迁移与 import 修复（不动逻辑）
2) 命名统一：agent_service → agent_controller；view → ui/views；pages → ui/pages
3) 职责收敛：逐步引入/完善 `StateCollector` 与 `ToolKit`，让 TaskManager 仅保留 loop 与状态机
4) 视图逻辑下沉：引入 ViewModel（可选，按页面渐进）

---

## 9. 非目标（本次不做）
- 不改变云端协议、不更换任务循环策略
- 不引入多 AgentSpace 实例并存（当前策略是每任务一份）
- 不引入复杂 DI 框架/事件总线
