# AgentSpace（VTS）最小可调试版本开发路线图

目标：在当前工程中实现 **1 个 AgentSpace + 1 个虚拟屏(VTSBackend)** 的最小闭环，并能在 `entry/src/main/ets/pages/Index.ets` 中通过按钮/日志完成调试验证。

约束：
- 不做“多 Space / 多 Backend / Tool 抽象层”；本地单例管理。
- Viewer 仅用于**前台可视化**（可选启动）；后台执行不依赖 UI（但仍需验证是否必须绑定 surface 才能渲染/截图）。
- 不使用 hvigor 在本环境编译验证（编译由其他环境完成）。

---

## 完成标准（Definition of Done）

在 `Index.ets` 中可以完成以下验证：
- 创建 VTS（拿到 `virtualScreenId/screenId` 与可用的 `displayId`）。
- 在 VTS 上启动一个目标 App（调试期可硬编码一个已知应用）。
- 可选：启动 Viewer 并看到 VTS 画面（XComponent surface 绑定成功）。
- 注入 `click`（归一化 `0..1000` -> px 映射）并在目标 App 上产生可观察变化（例如点击按钮/滚动列表）。
- 截图接口返回 **raw base64**（不带 `data:image/...;base64,` 前缀），且内容随操作变化。
- 结束时 best-effort 将 VTS 上窗口转移回主屏（displayId=0），并销毁 VTS/清理状态。

---

## 最小模块边界（建议）

> 只保留两块：`AgentSpace`（单例、业务入口）+ `VTSBackend`（HarmonyOS 系统调用封装）。

- `AgentSpace`：管理生命周期/状态、动作执行、截图、可选的 Viewer 绑定状态。
- `VTSBackend`：薄封装 `screen/screenshot/AppController/startAbility` 等系统 API，不做额外抽象。
- `Viewer(Page)`：仅作为“可视化 + surface 提供者”；**不承载业务逻辑**。

---

## 目录与文件规划（建议）

根据现有工程结构，建议新增：
- `entry/src/main/ets/agentspace/AgentSpace.ets`：单例 + 对外 API
- `entry/src/main/ets/agentspace/VTSBackend.ets`：虚拟屏/截图/注入/转屏等系统能力封装
- `entry/src/main/ets/pages/AgentSpaceViewer.ets`：XComponent(SURFACE) + 调试 overlay + surface 回调

并修改：
- `entry/src/main/resources/base/profile/main_pages.json`：加入 `pages/AgentSpaceViewer`
- `entry/src/main/module.json5`：增加 VTS 相关权限（至少 `ohos.permission.ACCESS_VIRTUAL_SCREEN`、`ohos.permission.CAPTURE_SCREEN`；若使用注入/IME 需按实际补齐）
- `entry/src/main/ets/entryability/EntryAbility.ets`：允许通过 Want 参数切换加载 `Index`/`AgentSpaceViewer`（为未来 ServiceExt 启动 Viewer 做准备）

---

## 里程碑拆解（每步都可在 Index.ets 手动验收）

### M0：前置准备（依赖与权限）

任务：
- 确认可用依赖：`@kit.ArkUI` 的 `screen/screenshot/display`；`@hms.ai.appController`（若用于触控注入与转屏）。
- 在 `entry/src/main/module.json5` 声明 VTS/截图所需权限。
- 设计调试期使用的目标 App：先固定一个 `{bundleName, abilityName}`，避免引入“应用名映射层”。

验收：
- App 安装后具备创建虚拟屏与截图权限（至少不会在运行时直接报权限拒绝）。

---

### M1：VTSBackend POC（最小系统能力闭环）

任务（只做最小子集）：
1) `createVirtualScreen()`：用默认屏幕宽高创建 VTS，记录 `screenId`，尽可能读取 `displayId`（若系统返回）。
2) `startAbilityOnDisplay()`：在 `displayId ?? screenId` 上启动目标 App（调试期先不做“present viewer”）。
3) `screenshot()`：对 `displayId ?? screenId` 进行截图，转成 **raw base64**（无 data 前缀）。
4) `injectClick()`：用 `AppController.processPointEvent` 组合 DOWN/UP（x/y 先用 px 直传）。
5) `transferAllWindowsToMain()`：结束时将 VTS 上 window 转回主屏（displayId=0）。
6) `destroyVirtualScreen()`：销毁 VTS。

关键验证点（决定 Viewer 是否“后台必需”）：
- **验证 A**：不绑定 surface（不启动 Viewer）的情况下，`screenshot()` 是否能得到有效图像且随操作变化。
  - 若能：Viewer 真正可选（只用于前台观看）。
  - 若不能：需要一个“surface provider”策略（可选方案：启动 Viewer 但立即最小化/不展示给用户；或寻找系统提供的 headless surface 能力）。

验收：
- 在 Index.ets 通过按钮顺序调用：create -> launchApp -> screenshot -> click -> screenshot -> transfer -> destroy，全流程无崩溃。

---

### M2：AgentSpace 单例（状态机 + 最小 API）

任务：
- 设计并实现最小状态：
  - VTS：`virtualScreenId/screenId`、`displayId?`、`widthPx/heightPx/density`
  - Viewer：`viewerActive:boolean`、`surfaceId?`、`surfaceAttached:boolean`
  - 并发：序列化 `start/stop/execute`（避免并行导致状态错乱）
- 对外 API（最小集合）：
  - `updateContext(ctx)`：只保存“能 startAbility 的上下文”，未来从 EntryAbility 换到 ServiceExt 只需更新 ctx
  - `ensureVts()`：只保证 VTS 已创建
  - `showViewer()` / `closeViewerBestEffort()`：可选
  - `onViewerSurfaceReady(surfaceId,w,h)`：viewer 回调后 `setVirtualScreenSurface`
  - `launchApp(target)`
  - `captureScreenshotBase64()`
  - `executeAction({name, parameters})`：先支持 `click / swipe / wait / launch_app / finish` 子集（对齐 `/docs/api.md`）
  - `stop({transferToMain, destroyVts})`

验收：
- Index.ets 不再直接调用 VTSBackend，而是只调 AgentSpace API；截图/点击仍有效。

---

### M3：Viewer 页面（仅做 surface + 可视化）

任务：
- 新增 `pages/AgentSpaceViewer.ets`：
  - `XComponent(SURFACE)` 获取 `surfaceId`
  - 调用 `AgentSpace.onViewerSurfaceReady(surfaceId, widthPx, heightPx)`
  - 可选 overlay：展示 `virtualScreenId`、`surfaceAttached`、最近一次 action 描述
- 支持“关闭 viewer”：
  - `EntryAbility`/`Viewer` 读 Want 参数：`closeViewer=true` 时自终止/回到 Index

验收：
- Index.ets 中点击“Show Viewer”后可看到 VTS 画面（若 M1 验证表明需要 surface，则此步完成后截图应从空变为可用）。

---

### M4：Index.ets 调试面板（快速回归工具）

任务（建议提供按钮与最小日志输出）：
- `Start VTS`（ensureVts）
- `Launch App`（固定 target）
- `Show Viewer` / `Close Viewer`
- `Click (x,y)`：提供几个预置点（中心点/左上角/右下角）
- `Swipe`：预置从下往上滑
- `Screenshot`：显示截图长度/时间；可选在 UI 中预览（预览时可以临时加 `data:image/jpeg;base64,` 前缀）
- `Stop (transfer+destroy)`

验收：
- 在不改代码的情况下，靠 Index 面板能重复跑通主流程，并快速定位“VTS/Viewer/注入/截图”问题。

---

### M5：动作补齐（按云端 Action Space）

按优先级逐步补齐 `executeAction`：
1) `double_click`：两次 click（间隔 60~120ms）
2) `long_click`：DOWN -> sleep(duration_ms) -> UP
3) `press_back/press_enter`：评估可用系统 API（若无稳定注入接口，先降级为“交互提示 + SKIPPED/FAILED”）
4) `type`：评估是否必须启用 `AppController.connectInputMethod`（IME 注册/反注册生命周期）；调试期先支持“当前焦点输入”

验收：
- 能在 Index.ets 触发上述动作且可观察到效果；失败时返回清晰错误信息而非 silent no-op。

---

## 风险与决策点

- **是否必须绑定 surface 才能渲染/截图**：这是设计分叉点（M1 验证 A）。
- **后台运行时的上下文能力**：未来迁移到 `ServiceExtensionAbility` 后，仍需要一个 UIAbility 来承载 Viewer（但 AgentSpace 本体不应依赖 router/UI）。
- **权限与系统版本差异**：虚拟屏/截图/输入注入在不同设备与系统版本上行为可能不同，需以 POC 结果为准。

