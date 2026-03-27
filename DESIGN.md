# Operator Design

> 跨平台自动化内核，Rust 实现；当前提供 CLI / MCP 入口，并为 Agent / A2A 保留扩展边界。

---

## 目录

1. [[#1. 背景|背景]]
2. [[#2. 设计目标|设计目标]]
3. [[#3. 设计原则|设计原则]]
4. [[#4. 总体架构|总体架构]]
5. [[#5. 分层设计|分层设计]]
6. [[#6. Workspace 结构|Workspace 结构]]
7. [[#7. 统一领域模型|统一领域模型]]
8. [[#8. 执行模型|执行模型]]
9. [[#9. 平台能力模型|平台能力模型]]
10. [[#10. 平台抽象|平台抽象]]
11. [[#11. Runtime 设计|Runtime 设计]]
12. [[#12. Snapshot 与 Session|Snapshot 与 Session]]
13. [[#13. Tool 设计|Tool 设计]]
14. [[#14. 配置系统|配置系统]]
15. [[#15. CLI 设计|CLI 设计]]
16. [[#16. MCP 设计|MCP 设计]]
17. [[#17. Agent 设计|Agent 设计]]
18. [[#18. 平台实现建议|平台实现建议]]
19. [[#19. 扩展机制|扩展机制]]
20. [[#20. 并发与超时模型|并发与超时模型]]
21. [[#21. 安全模型|安全模型]]
22. [[#22. 当前实现范围|当前实现范围]]
23. [[#23. 当前进度与推荐后续顺序|当前进度与推荐后续顺序]]
24. [[#24. 与 Peekaboo 风格架构的主要差异|与 Peekaboo 风格架构的主要差异]]
25. [[#25. 风险与后续关注点|风险与后续关注点]]
26. [[#26. 补充决策|补充决策]]

---

## 1. 背景

目标是重新实现一个类似 Peekaboo 的自动化工具，但约束更明确：

- 使用 **Rust**
- 面向**跨平台**自动化
- 提供统一的平台能力抽象
- 可扩展到 macOS、Windows、HarmonyOS
- 同时支持 CLI、Agent、MCP 三种入口
- **不**实现 App 壳层
- 架构尽量轻量、精简、可维护

本文档描述 Operator 的总体架构、抽象边界和实现路径。

> **实现状态说明（2026-03-27）：** 本文档中的核心分层、typed runtime、snapshot/capability 模型仍然有效；其中 macOS 平台、统一 `operator` CLI、`operator mcp serve`、`operator agent <task>` 已经实现。多平台接入的核心内核已具备雏形，但入口层装配仍然以 macOS 为主，Windows / Harmony driver 仍未落地。

---

## 2. 设计目标

### 2.1 功能目标

- 统一抽象多平台自动化能力
- 支持观察、查询、交互三类核心操作
- 使用同一套工具定义同时服务 CLI、Agent、MCP
- 允许按 crate 扩展新平台
- 支持会话恢复、快照复用、审计和调试

### 2.2 非目标

- 不实现桌面 GUI 或 menubar app
- 不追求对所有平台做完全一致的高级能力
- 不在第一阶段实现复杂分布式调度
- 不把 shell 执行作为核心能力暴露

---

## 3. 设计原则

| 原则 | 说明 |
|---|---|
| 能力优先 | 不以某个操作系统的 UI 概念为中心 |
| 平台差异下沉 | driver 内部消化差异，不上浮到 CLI、Agent、MCP |
| 统一执行链 | 所有入口共用同一套工具定义和执行链路 |
| Typed Kernel | 核心执行链优先使用强类型请求/响应，JSON 只停留在入口边界 |
| MVP 平台优先 | 先用 macOS + CLI 验证骨架，再扩展 MCP、Agent 和其他平台 |
| 显式依赖注入 | 不依赖全局单例 |
| 状态文件化 | 先避免引入重型持久化 |
| 只抽象共性 | 不强行统一平台特有能力，用 capability 驱动 |
| northbound 稳定 | CLI / MCP / Agent 只暴露命名 target，不暴露 local / remote / bridge 等连接细节 |

---

## 4. 总体架构

```
┌─────────────────────────────────────────────────────┐
│                  User / LLM Client                  │
└───────────┬─────────────────┬───────────────────────┘
            │                 │                 │
          CLI              MCP Server       Agent Runner
            │                 │                 │
            └────────────┬────┘                 │
                         │                      │
                    Tool Registry  ◄─────────────┘
                         │
                    RuntimeCore
                    ┌────┴────────────────────────────┐
                    │         │            │           │
               Snapshot    Session      Event      Target
                Store       Store        Sink      Resolver
                                                    │
                              ┌──────────┬──────────┼──────────┐
                              │          │          │          │
                           macOS     Windows    Harmony    (future)
                           Driver     Driver     Driver

Agent Runner
  ├── ModelClient
  ├── Tool Registry
  └── Session Store
```

**核心思路：**

- `RuntimeCore` 是核心装配对象，不持有 `ToolRegistry`，避免循环引用
- `ToolRegistry` 独立持有，handler 持有 `Arc<RuntimeCore>` 而非完整 `Runtime`
- `PlatformDriver` 是平台执行边界，对上提供 typed `observe/query/act`
- `SnapshotStore` 和 `SessionStore` 提供可共享的状态协议，但不同入口不共享进程内实例

---

## 5. 分层设计

系统分成四层，依赖方向单向向下。

```
┌─────────────────────────────────┐
│          Entry Layer            │  CLI / MCP / Agent（独立 crate）
├─────────────────────────────────┤
│         Runtime Layer           │  装配、调度、工具注册、会话管理
├─────────────────────────────────┤
│         Platform Layer          │  各平台 driver crate
├─────────────────────────────────┤
│           Core Layer            │  领域模型、协议定义、错误类型
└─────────────────────────────────┘
```

### 5.1 Core 层

负责纯领域模型和协议，**不依赖**任何平台、CLI、MCP。

- 错误类型（`OperatorError`，见下方定义）
- `Observe` / `Query` / `Action` 请求与响应模型
- `Snapshot` / `UiElement` / `Surface` / `Locator`
- `Target` / `CapabilitySet` / `Capability`

**`OperatorError` 定义（使用 `thiserror`）：**

```rust
#[derive(Debug, thiserror::Error)]
pub enum OperatorError {
    #[error("capability not supported: {0:?}")]
    CapabilityNotSupported(Capability),

    #[error("target not found: {0}")]
    TargetNotFound(String),

    #[error("target is busy (queue timeout)")]
    TargetBusy,

    #[error("operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("element not found: {0:?}")]
    ElementNotFound(ElementId),

    #[error("snapshot not found: {0:?}")]
    SnapshotNotFound(SnapshotId),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    #[error("platform error: {0}")]
    Platform(String),

    #[error("tool error: {tool}, message: {message}")]
    Tool { tool: String, message: String },

    #[error("model error: {0}")]
    Model(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
```

### 5.2 Runtime 层

负责运行时装配和通用执行逻辑，**不直接调用平台 API**。

- `RuntimeCore` / `RuntimeBuilder`（核心装配，不持有 `ToolRegistry`）
- `Runtime`（外层聚合，持有 `Arc<RuntimeCore>` + `ToolRegistry`）
- `TargetResolver`
- `ToolRegistry`（注册、查找、执行工具）
- `SnapshotStore` / `SessionStore`（trait + 文件实现）

### 5.3 Platform 层

每个平台一个独立 crate，只实现 `PlatformDriver` trait：

- 当前已实现：`operator-platform-macos`
- 未来扩展：`operator-platform-windows`、`operator-platform-harmony`

### 5.4 Entry 层

不同入口共享同一套 runtime 构造方式、tool catalog 和状态协议：

- 当前已实现：
  - `operator-cli` — 统一用户入口，暴露 `operator` 二进制
  - `operator-mcp` — MCP 协议适配库，由 `operator mcp serve` 复用
- 当前已实现：
  - `operator-agent` — 本地单 session agent runner（独立 crate，可选依赖）
- 未来扩展：
  - A2A surface — 复用 `operator-agent` 能力向外提供 agent 协议入口

> **说明：** Agent 单独成 crate 而非内嵌于 runtime，原因是 Agent 需要 `ModelClient`（外部 LLM 依赖）。核心 runtime 不应反向依赖任何 LLM/provider 抽象，使得只需要 CLI / MCP 能力的用户无需引入该依赖。

---

## 6. Workspace 结构

长期控制在少量清晰的 crate 内；当前 workspace 已经包含 macOS、CLI、MCP、Agent 和测试支撑，其他平台仍保持规划态。

```
operator/
  Cargo.toml                   # workspace 根，声明 members 和共用依赖
  crates/
    operator-core/              # 自动化领域模型、typed 请求/响应、错误
    operator-runtime/           # RuntimeCore、ToolRegistry、存储 trait
    operator-platform-macos/    # 当前唯一平台实现
    operator-cli/               # 当前唯一用户二进制：operator
    operator-mcp/               # MCP 协议适配库（无独立 bin target）
    operator-agent/             # 单 session 本地 agent runner
    operator-testkit/           # 测试工具：MockPlatformDriver、fixture 等
```

### 6.1 当前实现与未来扩展

当前 workspace members 为：

- `operator-cli`
- `operator-agent`
- `operator-core`
- `operator-mcp`
- `operator-platform-macos`
- `operator-runtime`
- `operator-testkit`

未来若接入其他平台，可在 workspace 中新增：

- `operator-platform-windows`
- `operator-platform-harmony`

当前 `operator-cli` 与 `operator-agent` 仍直接依赖 `operator-platform-macos` 完成本地 runtime 装配；多平台最终形态需要额外引入“平台/driver 注册层”，把入口层从 macOS 直连装配中解耦出来。

### 6.2 `operator-testkit` 职责

提供可跨 crate 复用的测试基础设施，**不参与生产构建**：

- `MockPlatformDriver` — 实现 `PlatformDriver`，可预设返回值和错误
- `InMemorySnapshotStore` / `InMemorySessionStore` — 测试用内存存储
- 常用测试 fixture（`test_snapshot()`, `test_element()` 等）

---

## 7. 统一领域模型

跨平台自动化最容易踩坑的地方，是直接把窗口、菜单、Dock 这类桌面概念当作核心抽象。为了兼容 HarmonyOS，这些概念必须降级为可选能力。

| 概念 | 说明 |
|---|---|
| `Target` | 当前执行目标的命名引用，如 `macos`、`windows-lab`、`harmony-phone` |
| `Surface` | 一次观察的上下文，如整屏、前台应用、某个窗口、某个区域 |
| `Snapshot` | 一次观察结果，包含图像、元素树、元数据 |
| `UiElement` | 可交互、可观测或可识别的 UI 元素 |
| `Locator` | 元素定位方式，如 snapshot 内元素 ID、文本、角色、坐标 |
| `Action` | 点击、输入、滚动、按键、启动应用等动作 |
| `CapabilitySet` | 某平台支持的能力集合 |

### 7.1 Surface

```rust
pub struct Surface {
    pub kind: SurfaceKind,
}

pub enum SurfaceKind {
    /// 全屏截取，display_id 为 None 时取主屏
    Fullscreen { display_id: Option<u32> },
    /// 当前前台应用
    Frontmost,
    /// 指定窗口
    Window { id: WindowId },
    /// 指定区域
    Region { rect: Rect },
}
```

> **MVP 约束：** Core 只保留已被桌面平台验证的观察上下文。HarmonyOS / Web 风格的页面概念不进入首版 core，而是在对应平台扩展设计中定义。

### 7.2 Locator

定位策略按稳定性由高到低排列：

```rust
pub enum Locator {
    /// 最稳定：snapshot 内的元素 ID，需同时提供 snapshot 上下文
    SnapshotElement { snapshot: SnapshotId, element: ElementId },
    /// 次选：文本匹配
    Text(String),
    /// 次选：角色 + 序号匹配
    Role { role: String, index: usize },
    /// 最不稳定：裸坐标（明确标注，工具层应给出警告）
    Coords(Point),
}
```

> **说明：** 孤立的 `ElementId`（无 snapshot 上下文）不作为独立 `Locator` 变体，原因是平台 driver 无法在没有 snapshot 上下文时重新定位该元素。所有基于元素 ID 的定位必须携带 `SnapshotId`。平台特有 escape hatch 不进入 core，避免把不可移植能力固化成公共接口。

### 7.3 Snapshot 与 UiElement

```rust
pub struct Snapshot {
    pub id: SnapshotId,
    pub target: TargetId,
    pub surface: Surface,
    /// 图像的逻辑 ID，由 SnapshotStore 解析为实际路径或 URL
    pub image_artifact: Option<ArtifactId>,
    /// 元素树，使用 HashMap 保证 O(1) 查找
    pub elements: HashMap<ElementId, UiElement>,
    /// 根节点 ID 列表（顶层元素）
    pub root_ids: Vec<ElementId>,
    pub metadata: SnapshotMetadata,
    pub created_at: SystemTime,
    /// TTL，可选；None 表示使用全局默认值
    pub expires_at: Option<SystemTime>,
}

pub struct UiElement {
    pub id: ElementId,
    pub role: String,
    pub label: Option<String>,
    pub value: Option<String>,
    pub bounds: Option<Rect>,
    pub enabled: Option<bool>,
    /// 子元素 ID，引用 snapshot.elements 中的条目
    pub children: Vec<ElementId>,
    pub confidence: Option<f32>,
    pub source: ElementSource,
}

pub enum ElementSource {
    Native,   // 平台无障碍 API
    Ocr,      // OCR 识别
    Vision,   // 视觉模型
    Hybrid,   // 多源融合
}
```

> **说明：** `elements` 使用 `HashMap<ElementId, UiElement>` 而非 `Vec<UiElement>`，避免在大型 AX 树（数百至数千节点）中 O(n²) 的子节点查找开销。`root_ids` 保留根节点入口用于树遍历。

### 7.4 ID 类型定义

所有 ID 使用 newtype wrapper，禁止使用裸 `String` / `u64`，保证类型安全且可在 `HashMap` 中用作 key。

```rust
/// UUID v4 字符串，由 SnapshotStore 在 save() 时生成
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnapshotId(pub String);

/// 平台 driver 生成，同一 snapshot 内唯一；跨 snapshot 无语义
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ElementId(pub String);

/// UUID v4 字符串，AgentRunner 在创建 Session 时生成
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub String);

/// 命名 target，如 "macos"、"windows-lab"、"harmony-phone"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TargetId(pub String);

/// 平台原生窗口句柄；u64 可容纳 macOS CGWindowID（u32）和 Win32 HWND（usize）
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

/// UUID v4 字符串，对应 SnapshotStore artifacts/ 目录下的截图文件
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ArtifactId(pub String);
```

所有 ID 类型均实现 `Display`，格式即内部值（不加前缀），方便 CLI 输出和日志。

### 7.5 补充类型定义

以下类型在领域模型中引用，在此集中定义。

```rust
// ── QueryResult 相关 ─────────────────────────────────────

pub struct AppInfo {
    /// macOS bundle ID / Harmony 包名；桌面应用通常有，系统进程可能为 None
    pub bundle_id: Option<String>,
    pub name: String,
    pub pid: Option<u32>,
    pub is_running: bool,
}

pub struct WindowInfo {
    pub id: WindowId,
    pub title: Option<String>,
    pub app_name: Option<String>,
    pub bounds: Option<Rect>,
    pub is_focused: bool,
    pub is_minimized: bool,
}

/// GetFocus 返回类型；不绑定 snapshot 上下文，仅描述当前焦点元素基本属性
/// 与 UiElement 的区别：FocusInfo 是实时查询结果，无 children / confidence 等 snapshot 专有字段
pub struct FocusInfo {
    pub role: String,
    pub label: Option<String>,
    pub bounds: Option<Rect>,
    pub app_name: Option<String>,
}

// ── Snapshot 相关 ────────────────────────────────────────

pub struct SnapshotMetadata {
    /// 平台标识，如 "macos"、"windows"、"harmony"
    pub platform: String,
    /// 逻辑像素与物理像素比，如 Retina 屏为 2.0
    pub display_scale: Option<f32>,
    /// 本次 observe 调用的实际耗时
    pub capture_duration_ms: u64,
}

// ── 权限相关 ─────────────────────────────────────────────

pub struct PermissionsReport {
    pub accessibility: PermissionStatus,
    pub system_events: PermissionStatus,
    pub screen_recording: PermissionStatus,
}

pub enum PermissionStatus {
    Granted,
    Denied,
    /// 用户从未授权也从未拒绝（如 macOS 首次请求前）
    NotDetermined,
}

// ── 审计相关 ─────────────────────────────────────────────

pub struct AuditEvent {
    pub timestamp: SystemTime,
    pub session_id: Option<SessionId>,
    pub target_id: Option<TargetId>,
    pub kind: AuditEventKind,
}

pub enum AuditEventKind {
    ToolInvoked   { tool: String, input: serde_json::Value },
    ToolCompleted { tool: String, duration_ms: u64, success: bool },
    CapabilityDenied  { tool: String, capability: Capability },
    SideEffectBlocked { tool: String },
}

// ── RuntimeConfig ────────────────────────────────────────

pub struct RuntimeConfig {
    pub default_target: TargetId,
    pub snapshot_ttl_hours: u64,
    pub max_snapshots: usize,
    pub default_timeout_ms: u64,
    pub audit_enabled: bool,
    pub allow_side_effects: bool,
    pub redact_sensitive_fields: bool,
    pub artifact_ttl_hours: u64,
    /// 每隔 N 次 SnapshotStore::save() 懒触发一次 evict_expired()
    pub snapshot_evict_interval: u32,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            default_target: TargetId("macos".into()),
            snapshot_ttl_hours: 24,
            max_snapshots: 200,
            default_timeout_ms: 10_000,
            audit_enabled: true,
            allow_side_effects: true,
            redact_sensitive_fields: true,
            artifact_ttl_hours: 24,
            snapshot_evict_interval: 100,
        }
    }
}
```

---

## 8. 执行模型

核心执行链直接使用 typed 请求/响应，不再引入 `Operation` 总包装。JSON schema 只用于 CLI / MCP / Agent 入口边界。

| 类别 | 包含能力 | 副作用 |
|---|---|---|
| `Observe` | 截图、获取 UI 树、OCR、生成 snapshot | 无 |
| `Query` | 列应用、列窗口、查询权限、获取焦点、查询平台能力 | 无 |
| `Act` | 点击、输入、滚动、热键、拖拽、启动应用、聚焦窗口 | 有 |

### 8.2 请求结构

```rust
pub struct ObserveRequest {
    pub surface: Surface,
    pub include_screenshot: bool,
    pub include_elements: bool,
}

pub struct ObserveResult {
    pub snapshot: Snapshot,
}

pub enum QueryRequest {
    ListApps,
    ListWindows { app: Option<String> },
    GetFocus,
    PermissionsStatus,
    Capabilities,
}

pub struct ActionRequest {
    pub action: Action,
    pub locator: Option<Locator>,
    pub target_selector: Option<ActionTargetSelector>,
    pub focus_policy: ActionFocusPolicy,
    pub verifications: Vec<ActionVerification>,
}

pub enum Action {
    Click { mode: ClickMode },
    Move,
    Type {
        text: String,
        clear_before: bool,
        trailing_keys: Vec<TypeTrailingKey>,
    },
    Press { key: String, count: NonZeroU32 },
    Scroll { delta_x: f64, delta_y: f64 },
    Hotkey { keys: Vec<String> },
    Drag {
        from: Locator,
        to: Locator,
        motion: DragMotion,
    },
    Swipe {
        from: Locator,
        to: Locator,
        duration_ms: Option<u64>,
        steps: Option<NonZeroU32>,
    },
    LaunchApp { bundle_id_or_name: String },
    SwitchApp,
    QuitApp,
    RelaunchApp,
    HideApp,
    UnhideApp,
    CloseWindow,
    MinimizeWindow,
    MaximizeWindow,
    MoveWindow { x: f64, y: f64 },
    ResizeWindow { width: f64, height: f64 },
    SetWindowBounds { bounds: Rect },
    FocusWindow { id: WindowId },
}
```

### 8.3 查询与动作结果

```rust
pub enum QueryResult {
    Apps(Vec<AppInfo>),
    Windows(Vec<WindowInfo>),
    /// 使用 FocusInfo 而非 UiElement，原因：焦点查询是实时调用，
    /// 无 snapshot 上下文，不适合携带 children / confidence 等 snapshot 专有字段
    Focus(Option<FocusInfo>),
    Permissions(PermissionsReport),
    Capabilities(CapabilitySet),
}

pub struct ActionOutcome {
    pub success: bool,
    pub duration_ms: u64,
    pub detail: Option<String>,
    pub coordinates: Option<ActionCoordinates>,
    pub target_app: Option<AppInfo>,
    pub target_window: Option<WindowInfo>,
    pub side_effects: Vec<ActionSideEffect>,
    pub warnings: Vec<String>,
}
```

> **说明：** 当前 action 模型已经超出最初 MVP，只是仍保持 `Observe / Query / Act` 三分法不变。用户面通过 CLI 分组命令暴露这些能力，内部 tool/runtime 仍使用同一套 typed 请求结构。

### 8.4 执行上下文

```rust
pub struct ExecContext {
    pub target: TargetId,
    pub session: Option<SessionId>,
    /// 单次操作超时，None 时使用 RuntimeConfig 中的默认值
    pub timeout_ms: Option<u64>,
}
```

> **MVP 说明：** 重试策略（RetryPolicy）在 MVP 阶段不实现，需要时在 `ExecContext` 中扩展。当前超时只有单一层级，保持结构简单。

---

## 9. 平台能力模型

不要求所有平台支持完全相同的能力，运行时先检查 capability，再决定是否允许某个工具执行。

```rust
pub enum Capability {
    /// 屏幕截图
    Capture,
    /// 平台无障碍 API 获取元素树
    InspectTree,
    /// OCR 识别
    InspectText,
    /// 指针输入
    PointerInput,
    /// 键盘输入
    KeyboardInput,
    /// 窗口管理
    WindowManagement,
    /// 应用启动/关闭
    AppLifecycle,
    /// 剪贴板
    Clipboard,
    /// 权限查询
    Permissions,
    /// 设备信息
    DeviceInfo,
    /// 平台扩展能力（如 macOS menu、Harmony gesture）
    Extension(CapabilityId),
}

pub struct CapabilityId {
    pub namespace: &'static str,
    pub name: &'static str,
}

pub struct CapabilitySet(HashSet<Capability>);

impl CapabilitySet {
    pub fn supports(&self, cap: &Capability) -> bool { ... }
}
```

> **设计说明：** 不在核心枚举中硬编码平台特有能力（如原有的 `Menu`），而是使用结构化 `CapabilityId` 作为扩展点。这样 Core 层保持平台中立，又避免把平台能力做成裸字符串。

---

## 10. 平台抽象

为保持轻量，平台层对上只暴露一个统一 driver trait，但直接提供 typed `observe/query/act` 三个入口，避免把所有行为再包一层动态 `Operation`。这里的关键区别是：**平台（platform）是能力归属，driver 是具体执行实现**；同一平台允许存在多个 driver。

```rust
pub trait PlatformDriver: Send + Sync {
    fn platform_id(&self) -> &'static str;
    fn driver_id(&self) -> &'static str;
    fn capabilities(&self) -> CapabilitySet;
    async fn health_check(&self) -> Result<HealthStatus, OperatorError>;
    async fn observe(
        &self,
        req: ObserveRequest,
        ctx: &ExecContext,
    ) -> Result<ObserveResult, OperatorError>;
    async fn query(
        &self,
        req: QueryRequest,
        ctx: &ExecContext,
    ) -> Result<QueryResult, OperatorError>;
    async fn act(
        &self,
        req: ActionRequest,
        ctx: &ExecContext,
    ) -> Result<ActionOutcome, OperatorError>;
}

pub struct HealthStatus {
    pub healthy: bool,
    pub message: Option<String>,
    pub permissions: PermissionsReport,
}
```

上层 runtime 只做三件事：

1. 根据 `target` 选择 driver
2. 检查 `CapabilitySet`
3. 调用 typed `observe/query/act`

平台内部可以自行拆分为 capture / inspect / input / app-lifecycle 等子模块，这些实现细节不暴露给 runtime 层。对于 northbound 入口而言，`--target` 只选择一个**命名 target**；target 内部究竟由本地 driver、远端 driver、bridge driver 还是 node driver 执行，不直接暴露在 CLI / MCP / Agent 命令面里。

> **演进路径：** 若 driver 实现随能力增长变得过于庞大，可在平台 crate 内部引入私有 dispatcher 或子服务进行分发，无需改动 core 层的 trait 签名。

---

## 11. Runtime 设计

### 11.1 结构拆分

Runtime 拆分为 `RuntimeCore` 和 `Runtime` 两个结构，以避免 `ToolHandler` 持有 `Arc<Runtime>` 导致的循环引用：

```
Runtime
  ├── Arc<RuntimeCore>    ← ToolHandler 持有此引用（不持有 ToolRegistry）
  └── ToolRegistry        ← 持有所有 ToolHandler
```

```rust
/// 核心组件容器，不持有 ToolRegistry，可安全被 ToolHandler 引用
pub struct RuntimeCore {
    resolver: TargetResolver,
    snapshots: Arc<dyn SnapshotStore>,
    sessions: Arc<dyn SessionStore>,
    event_sink: Arc<dyn EventSink>,
    config: RuntimeConfig,
}

/// 外层聚合，对入口层暴露
pub struct Runtime {
    core: Arc<RuntimeCore>,
    tools: ToolRegistry,
}
```

### 11.2 依赖 trait

```rust
/// 快照存储
pub trait SnapshotStore: Send + Sync {
    async fn save(&self, snapshot: &Snapshot) -> Result<(), OperatorError>;
    async fn get(&self, id: &SnapshotId) -> Result<Option<Snapshot>, OperatorError>;
    async fn list(&self, target: &TargetId) -> Result<Vec<SnapshotId>, OperatorError>;
    async fn delete(&self, id: &SnapshotId) -> Result<(), OperatorError>;
    async fn resolve_artifact(&self, id: &ArtifactId) -> Result<PathBuf, OperatorError>;
    /// 清理过期快照及其 artifact 文件，返回清理数量
    /// 调用时机：RuntimeBuilder::build() 时调用一次，之后每隔 N 次 save() 懒触发
    async fn evict_expired(&self) -> Result<u32, OperatorError>;
}

/// 会话存储
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: &Session) -> Result<(), OperatorError>;
    async fn append(&self, id: &SessionId, event: &SessionEvent) -> Result<(), OperatorError>;
    async fn get(&self, id: &SessionId) -> Result<Option<Session>, OperatorError>;
    /// limit: None 时返回最近 100 条；调用方不应依赖无限返回
    async fn list(&self, limit: Option<usize>) -> Result<Vec<SessionId>, OperatorError>;
}

/// 事件输出（用于审计和调试）
pub trait EventSink: Send + Sync {
    async fn emit(&self, event: AuditEvent) -> Result<(), OperatorError>;
}
```

### 11.3 RuntimeCore 职责

- 注册平台 driver
- 解析 target ref，得到 `TargetDescriptor` 并选择正确的 driver
- 能力检查，拒绝不支持的操作
- 提供 typed `observe/query/act` 执行入口
- 管理 snapshot 和 session 生命周期
- 记录执行事件到 `EventSink`

### 11.4 TargetResolver

`TargetResolver` 负责把用户输入的**命名 target** 解析为结构化描述，再映射到对应的平台 driver：

```rust
pub struct TargetDescriptor {
    pub id: TargetId,
    pub platform: String,
    pub driver: String,
}
```

推荐的长期形态：

| 命名 target | 解析结果示例 |
|---|---|
| `macos` | `platform = "macos"`, `driver = "macos.system"` |
| `windows-lab` | `platform = "windows"`, `driver = "windows.remote"` |
| `harmony-phone` | `platform = "harmony"`, `driver = "harmony.node"` |

这里的 `remote` / `bridge` / `node` 只属于 driver 选择和配置范畴，不进入用户侧 target 语法。

---

## 12. Snapshot 与 Session

### 12.1 Snapshot

Snapshot 是系统的关键状态原语，作用：

- 将观察与动作**解耦**：act 操作优先使用 snapshot element ID，而不是重复模糊匹配
- 给 Agent 提供**稳定上下文**，避免每次操作都重新截图
- 提供**审计和回放**线索

**MVP 存储结构：**

```
~/.operator/
  snapshots/
    <snapshot-id>.json
  artifacts/
    <snapshot-id>.png         # image_artifact 对应的实际文件
  config.toml
```

**生命周期策略：**

- 每个 snapshot 可携带 `expires_at`，默认 TTL 为 24 小时（来自 `config.toml`）
- `SnapshotStore::evict_expired()` 在 `RuntimeBuilder::build()` 时调用一次（清理遗留），此后每隔 100 次 `save()` 懒触发一次
- 最大保留数量可在 `config.toml` 中配置

### 12.2 Session

Session 属于 **Phase 3（Agent / 审计）** 能力，不阻塞 CLI / MCP 骨架落地。

Session 记录一次完整的交互过程：

```rust
pub struct Session {
    pub id: SessionId,
    pub created_at: SystemTime,
    pub task: String,
    pub status: SessionStatus,
}

pub enum SessionEvent {
    UserInput { text: String },
    ToolCall { name: String, input: serde_json::Value },
    ToolResult { name: String, output: serde_json::Value },
    ModelResponse { content: String },
    Error { message: String },
    Completed { summary: Option<String> },
}

pub enum SessionStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
}
```

实现上优先使用 `.jsonl` 格式，每行一个 `SessionEvent`，不引入数据库。

---

## 13. Tool 设计

工具定义是整个架构复用的核心，**只有一份定义，同时服务 CLI、MCP、Agent**。但 JSON 只停留在工具边界，业务执行进入 runtime 后全部转为 typed 请求/响应。

### 13.1 ToolSpec 与 ToolRegistration

将静态描述与运行时 handler 分离：

```rust
/// 纯静态描述，可序列化，供 CLI help、MCP tool list、Agent tool catalog 使用
/// 建议通过 schemars crate 从 Rust 类型自动生成 input_schema，减少手写 JSON Schema 的错误
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub capabilities_required: &'static [Capability],
    pub has_side_effects: bool,
}

/// 运行时注册单元，包含 handler
pub struct ToolRegistration {
    pub spec: ToolSpec,
    pub handler: ToolHandler,
}

/// handler 接收工具专有 JSON 输入 + RuntimeCore + 已构造好的 ExecContext
///
/// ExecContext 由 ToolRegistry.invoke() 在调用 handler 前从 JSON 中提取并构造：
///   - `target`     → 取 JSON["target"]，缺省时读 RuntimeConfig.default_target
///   - `session_id` → 取 JSON["session_id"]，可选
///   - `timeout_ms` → 取 JSON["timeout_ms"]，缺省时读 RuntimeConfig.default_timeout_ms
///
/// 这三个字段被提取后不从 JSON 中删除（handler 可忽略），保证 handler 签名干净。
/// 约束：handler 内部必须尽快将 JSON 反序列化为 typed 请求结构。
pub type ToolHandler = Arc<
    dyn Fn(serde_json::Value, Arc<RuntimeCore>, ExecContext) -> BoxFuture<'static, Result<serde_json::Value, OperatorError>>
        + Send
        + Sync,
>;
```

> **循环引用说明：** `Runtime` 持有 `ToolRegistry`，`ToolRegistry` 持有 `ToolHandler`。若 `ToolHandler` 持有 `Arc<Runtime>`，则形成循环引用导致内存泄漏。解决方案是让 handler 只持有 `Arc<RuntimeCore>`，而 `RuntimeCore` 不包含 `ToolRegistry`，从根本上断开循环。

### 13.2 执行路径

```
CLI / MCP / Agent
  → ToolRegistry.invoke(name, input)
    → 能力检查（capabilities_required vs target.capabilities）
    → 副作用检查（has_side_effects vs 安全策略）
    → ToolHandler(input, Arc<RuntimeCore>)
      → 反序列化为 typed request
      → RuntimeCore.observe/query/act(..., ExecContext)
        → TargetResolver → PlatformDriver.observe/query/act()
```

### 13.3 工具分组（当前实现）

当前 `ToolRegistry` 内部仍使用一组稳定的 flat tool names；CLI 只是把它们重新编排成分组 shell surface，不改变 runtime 内部工具边界。

**观察类（无副作用）：**

| 工具 | 说明 |
|---|---|
| `observe` | 截图 + 获取 UI 元素树，生成 snapshot |
| `snapshot-get` | 获取已有 snapshot 详情 |
| `artifact-get` | 获取已持久化的截图 artifact 路径 |

**查询类（无副作用）：**

| 工具 | 说明 |
|---|---|
| `list-apps` | 列出运行中的应用 |
| `list-windows` | 列出窗口列表，可按 app 过滤 |
| `get-focus` | 查询当前焦点 app / window / element |
| `permissions-status` | 查询权限状态 |
| `capabilities` | 查询当前 target 支持的能力集 |

**动作类（有副作用）：**

| 工具 | 说明 |
|---|---|
| `click` / `move` / `scroll` / `drag` / `swipe` | 指针动作 |
| `type` / `press` / `hotkey` | 键盘动作 |
| `launch-app` / `switch-app` / `quit-app` / `relaunch-app` / `hide-app` / `unhide-app` | 应用生命周期动作 |
| `focus-window` / `close-window` / `minimize-window` / `maximize-window` / `move-window` / `resize-window` / `set-window-bounds` | 窗口管理动作 |

### 13.4 工具输入原则

定位目标的优先级（由高到低）：

1. `snapshot_id + element_id` — 最稳定，**强烈推荐**
2. `locator`（文本、角色等语义匹配）— 次选
3. 裸坐标 — 最后手段，工具层应打印警告日志

---

## 14. 配置系统

配置文件位于 `~/.operator/config.toml`，CLI flag 和环境变量可覆盖任何配置项。

```toml
[runtime]
default_target     = "macos"
snapshot_ttl_hours = 24
max_snapshots      = 200
default_timeout_ms = 10_000

[model.openai]
api_key  = ""
base_url = "https://api.openai.com/v1"

[model.doubao]
# API key 优先读取环境变量 ARK_API_KEY 或 DOUBAO_API_KEY
api_key  = ""
base_url = ""

[targets.macos]
platform = "macos"
driver   = "macos.system"

[targets.windows-lab]
platform = "windows"
driver   = "windows.remote"

[targets.windows-lab.driver_config]
endpoint = "wss://lab.example"

[targets.harmony-phone]
platform = "harmony"
driver   = "harmony.node"

[targets.harmony-phone.driver_config]
node     = "phone-01"

[mcp]
transport      = "stdio"   # stdio | http
disabled_tools = []

[security]
# MCP 模式下可设为 false，禁用所有有副作用的工具
allow_side_effects = true
# 审计记录是否落盘
audit_enabled = true
# 对工具输入/输出做脱敏后再落盘
redact_sensitive_fields = true
artifact_ttl_hours = 24

[agent]
model           = "gpt-5.4"
max_steps       = 50
step_timeout_ms = 30_000
```

target-specific 参数必须进入 `driver_config`，例如 `endpoint`、`node`；不要再把它们写成 target 表上的顶层字段。

**配置加载优先级：**

```
CLI flag > 环境变量 > ~/.operator/config.toml > 内置默认值
```

---

## 15. CLI 设计

CLI 偏工程化、可脚本化，是 `ToolRegistry` 的一个薄包装；但当前用户面已经从“平铺 tool 名”收敛成“按能力域组织的稳定 shell surface”。这层设计的权威补充说明见 [docs/COMMAND.md](docs/COMMAND.md)。

```bash
# 观察
operator observe frontmost --capture all
operator snapshot get s_123

# 查询
operator list windows --app TextEdit
operator focus

# 输入 / 应用 / 窗口
operator input click --text "Submit"
operator input type "hello world" --after-key return
operator app launch Calculator
operator window resize --window-id 42 --width 900 --height 700 --verify geometry

# MCP
operator mcp serve
```

**CLI 原则：**

- 所有命令支持 `--target`（默认读取配置中的 `default_target`）
- `--target` 选择命名 target，不暴露 local / remote / bridge 等连接细节
- 所有命令支持 `--json`，输出结构与 MCP 工具结果格式兼容
- 动作命令默认支持 `--timeout-ms` 覆盖超时
- `Core / Observe / Query / Action / MCP / A2A` 只作为 help 分组标题，不作为真实一级命令
- CLI 只做参数解析和格式化输出，不包含业务逻辑

---

## 16. MCP 设计

MCP server 直接暴露同一份工具定义，不另起一套逻辑。当前实现通过统一二进制入口：

```bash
operator mcp serve
```

内部协议适配仍由 `operator-mcp` crate 承载，但用户面只保留 `operator` 一个二进制。

### 16.1 Transport

当前实现只支持 **stdio** transport（兼容 Claude Desktop 等工具），后续按需扩展 HTTP Streamable transport。

### 16.2 设计要求

- 工具 schema 直接从 `ToolRegistry` 中导出，**零重复**
- handler 与 CLI / Agent 共用同一条执行链
- 支持通过配置禁用有副作用的工具（`security.allow_side_effects = false`）
- MCP 层只做协议适配（JSON-RPC 编解码），不包含平台逻辑

### 16.3 并发处理

MCP 是多客户端协议，多个请求可能同时到达同一个 target。MVP 处理策略优先保证确定性：

| 场景 | 策略 |
|---|---|
| 同一 target，任何操作 | MVP 一律串行，优先保证确定性 |
| 不同 target | 完全并发 |
| 队列超时 | 返回 `OperatorError::TargetBusy` |

> **实现说明：** MVP 阶段每个 target 维护一个 `tokio::sync::Semaphore`（许可数为 1）作为串行闸门。是否允许部分只读请求绕过队列，必须等 snapshot 一致性和平台行为经过验证后再放开。

---

## 17. Agent 设计

Agent 不单独维护工具系统，完全复用 `ToolRegistry`。这一层目前仍未实现；CLI root help 只保留 `A2A` 说明块作为未来入口占位，不反向污染 core/runtime 的自动化边界。

### 17.1 组件

```
AgentRunner（operator-agent crate）
  ├── ModelClient       # 调用 LLM，抽象具体 provider
  ├── SessionStore      # 记录对话历史（来自 RuntimeCore）
  └── ToolRegistry      # 复用同一份工具（来自 Runtime）
```

### 17.2 Agent Loop

```
1. 读取任务与历史 session
2. 构造 ModelRequest（含工具列表 + 历史消息）
3. 调用 ModelClient，获取 ModelResponse
4. 若响应包含工具调用：
   a. 通过 ToolRegistry 执行
   b. 记录调用和结果到 SessionStore
   c. 回到步骤 2
5. 若响应为最终答复或达到 max_steps，退出
```

> **原则：** Agent 的价值来自工具复用，不来自复杂推理框架。初版保持 loop 简单，优先保证工具行为稳定。

### 17.3 ModelClient 抽象

```rust
pub trait ModelClient: Send + Sync {
    async fn complete(
        &self,
        request: ModelRequest,
    ) -> Result<ModelResponse, OperatorError>;
}

pub struct ModelRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,       // 直接复用 ToolSpec
    pub max_tokens: Option<u32>,
}

pub struct ModelResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: StopReason,
    pub usage: TokenUsage,
}

pub enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: serde_json::Value },
}
```

这样将来可以接入 Anthropic、OpenAI、Gemini 或本地模型，但这些抽象只存在于 `operator-agent`，runtime 不绑定任何一家 SDK。

---

## 18. 平台实现建议

### 18.1 macOS（MVP 首选平台）

| 能力 | 实现方式 |
|---|---|
| 截图 | `CGWindowListCreateImage` / `ScreenCaptureKit` |
| UI 元素树 | `AXUIElement` (Accessibility API) |
| 鼠标输入 | `CGEvent` |
| 键盘输入 | `CGEvent` |
| 应用发现 | `NSWorkspace` |
| 应用启动 | `NSWorkspace.open` |
| 权限检测 | `AXIsProcessTrusted` / `CGRequestScreenCaptureAccess` |
| macOS 特有能力 | 通过 `Capability::Extension(CapabilityId { namespace: "macos", name: "menu" })` 声明 |

Rust 侧通过 `objc2` crate 调用 Objective-C API，或封装为 C FFI 桥接层。

### 18.2 Windows（第二阶段）

| 能力 | 实现方式 |
|---|---|
| 截图 | `BitBlt` / `DXGI Desktop Duplication` |
| UI 元素树 | UI Automation（`windows` crate） |
| 输入模拟 | `SendInput` |
| 应用/窗口管理 | `EnumWindows` / `ShellExecute` |
| 权限查询 | `GetForegroundWindow` |

### 18.3 HarmonyOS（Bridge 模式）

HarmonyOS 与桌面系统差异大，不建议初版追求 Rust 直接打系统 API。推荐 bridge 架构：

```
主控端（Rust PlatformDriver）
    ↕  WebSocket / ADB / 自定义 RPC
设备端（HarmonyOS 原生调试桥或辅助服务）
```

设备端实现屏幕截图、节点树查询、点击/滑动输入等，主控端 driver 负责协议通信和结果转换。

**能力建议：** 屏幕截图、页面节点树、点击/滑动/输入、应用启动、设备信息。

> **边界约束：** Harmony bridge 的页面、路由、设备节点等概念先停留在平台 crate 内，不提前进入 core 的公共 `Surface` / `Locator` 抽象。

> **待定事项：** Bridge 协议选型（WebSocket / ADB / 自定义 RPC）应在阶段 7 开始前独立调研确定，不应在设计阶段提前锁定。

---

## 19. 扩展机制

### 19.1 新增平台

1. 创建新 crate（`operator-platform-xxx`）
2. 实现 `PlatformDriver` trait
3. 在 runtime 装配层注册 driver，并决定是否在 CLI / MCP 中暴露对应 target
4. 声明支持的 `CapabilitySet`（平台特有能力使用 `Capability::Extension(CapabilityId { ... })`）
5. 若未来需要按平台裁剪发布包，再单独评估 Cargo feature 或分发策略

### 19.2 新增能力

1. 若为跨平台通用能力：在 `Capability` 枚举中增加变体
2. 若为平台特有能力：直接使用 `Capability::Extension(CapabilityId { ... })` 声明，无需修改 core
3. 在对应的 typed request / result 中增加字段或变体
4. 更新受影响的 tool schema
5. 在支持该能力的平台 driver 中实现

### 19.3 新增入口

新入口只依赖 `operator-runtime`（`Runtime + RuntimeCore`），不直接依赖任何平台 crate，不重复实现业务逻辑。

---

## 20. 并发与超时模型

### 20.1 并发策略

自动化执行的首要约束是**确定性**，并发策略以此为准：

| 场景 | 策略 |
|---|---|
| 同一 target，任何操作 | 串行执行（Semaphore 互斥） |
| 不同 target 间 | 完全并发 |
| 单个 agent session 内 | 工具调用串行 |
| MCP / CLI 队列超时 | 返回 `OperatorError::TargetBusy` |

### 20.2 超时层级

```
ExecContext.timeout_ms               # 单次 tool 调用超时（来自请求或配置默认值）
  └── 平台 driver 内部超时           # 如截图、AX 查询各自的超时
AgentRunner config.max_steps         # agent 最大步数上限
AgentRunner config.step_timeout_ms   # 单步超时（含 model 调用）
```

### 20.3 实现方式

- 使用 `tokio` 作为异步运行时
- 每个 target 对应一个 `tokio::sync::Semaphore`（许可数为 1，MVP 对所有操作生效）
- 超时使用 `tokio::time::timeout` 包裹 typed `observe/query/act` 调用

---

## 21. 安全模型

| 机制 | 说明 |
|---|---|
| 权限检查 | 执行前检查系统权限（录屏、辅助功能等） |
| 能力检查 | 执行前验证 target 支持所需 `Capability` |
| 副作用标记 | 每个工具显式标注 `has_side_effects` |
| MCP 安全模式 | `allow_side_effects = false` 时，有副作用工具返回拒绝 |
| 工具黑名单 | 通过 `disabled_tools` 配置项精确禁用指定工具 |
| 可配置审计日志 | 所有 tool 调用可通过 `EventSink` 记录，并支持脱敏和关闭 |

**初版不提供通用 shell 工具**，避免把 MCP 入口变成任意命令执行面。

---

## 22. 当前实现范围

### 22.1 当前 crate

- `operator-core`
- `operator-runtime`
- `operator-platform-macos`
- `operator-cli`
- `operator-mcp`
- `operator-testkit`

### 22.2 当前入口

- `operator` CLI
- `operator mcp serve`

### 22.3 当前能力面

**Observe / Query：**

- `observe`
- `snapshot-get`
- `artifact-get`
- `list-apps`
- `list-windows`
- `get-focus`
- `permissions-status`
- `capabilities`

**Action：**

- pointer / keyboard：`click`、`move`、`type`、`press`、`hotkey`、`scroll`、`drag`、`swipe`
- app lifecycle：`launch-app`、`switch-app`、`quit-app`、`relaunch-app`、`hide-app`、`unhide-app`
- window management：`focus-window`、`close-window`、`minimize-window`、`maximize-window`、`move-window`、`resize-window`、`set-window-bounds`

### 22.4 当前交付状态

- macOS 平台已能完成观察、查询、输入、应用生命周期和窗口管理
- 统一 `operator` CLI 已完成分组命令面和稳定 help 契约
- MCP stdio 模式已完成，并复用同一份 tool schema 和执行链
- `operator agent <task>` 已完成第一阶段接入，并直接调用 runtime 工具而非 CLI
- `operator-core` 与 `operator-runtime` 在不引入 LLM/provider 依赖时可独立编译
- 同一 target 的并发操作仍保持串行，优先保证确定性
- 运行时核心已经支持“同平台多 driver”的方向，但 CLI / Agent 的 runtime 装配仍直接注册 `MacosDriver`

### 22.5 未来阶段

- A2A：在已有 `operator-agent` 之上补齐 northbound agent 协议入口
- Windows / Harmony：补齐更多平台 driver

---

## 23. 当前进度与推荐后续顺序

**已完成：**

1. 内核与 runtime 骨架
2. macOS driver
3. 统一 `operator` CLI
4. `operator mcp serve`
5. 分组命令面与稳定 help 契约
6. `operator agent <task>` 第一阶段本地 runner

**推荐后续顺序：**

1. 平台/driver 注册层与命名 target 解析
2. Windows driver scaffold
3. Harmony driver scaffold
4. 更高阶的能力域（如 clipboard / dialog / menu 等）

---

## 24. 与 Peekaboo 风格架构的主要差异

本设计保留了以下关键优势：

- 一套工具定义复用 CLI、Agent、MCP
- snapshot-aware 自动化（observe 与 act 解耦）
- 平台能力可扩展（capability-driven）

同时做了明显精简：

| 差异点 | Peekaboo | Operator |
|---|---|---|
| 实现语言 | Swift | Rust |
| App 壳层 | 有 | 无 |
| Visualizer 系统 | 有 | 无 |
| 平台执行边界 | 多 service + orchestration | 单 `PlatformDriver`，对上暴露 typed `observe/query/act` |
| 平台装配 | Xcode workspace + submodule | Cargo workspace + runtime 装配 |
| 配置管理 | Tachikoma | `~/.operator/config.toml` |
| Agent crate | 与 runtime 合并 | 独立（已实现 phase-1 runner，可继续扩展） |

---

## 25. 风险与后续关注点

### 25.1 平台 driver 膨胀风险

即使改为 typed `observe/query/act`，平台实现内部仍可能膨胀。处理策略：

- 平台 crate 内部引入私有 capture / inspect / input / app 子模块
- **不**把平台内部拆分细节提升为 core 层公共 trait
- 新增能力时优先扩展 typed request / result，而不是重新引入动态总包装

### 25.2 HarmonyOS 差异风险

HarmonyOS 与桌面系统差异大，必须通过 capability 驱动而不是强行塞入窗口式抽象。Bridge 协议选型（WebSocket / ADB / 自定义 RPC）应在阶段 7 开始前独立调研，不应提前锁定。

### 25.3 元素定位稳定性

跨平台自动化的稳定性核心不在点击，而在元素定位。关键原则：

- MVP 就强制推行 snapshot + element ID 优先策略（`Locator::SnapshotElement`）
- 元素 ID 应在平台 driver 内保证同一 snapshot 内的唯一性和稳定性
- UI 变化导致 element ID 失效时，应明确报错（`OperatorError::ElementNotFound`），不得静默退化到坐标模式

### 25.4 请求模型扩展性

当前三分法（Observe / Query / Act）对于 MVP 足够，但随着能力增长可能出现分类模糊的边界情况。后续可以扩展 typed 请求结构或拆分子请求枚举，但不应退回到无类型 payload 方案。

### 25.5 Agent 不确定性

Agent 的价值来自工具复用，不来自复杂推理框架。初版应保持 loop 简单，优先保证工具行为稳定，再逐步引入计划、反思等能力。

### 25.6 审计与敏感数据风险

Snapshot、tool input/output、model response 都可能包含敏感信息。MVP 起就应支持：

- 审计可关闭
- 落盘前脱敏
- artifact TTL 与清理策略
- 明确区分调试日志与审计记录

---

## 26. 补充决策

### 26.1 Async Trait 实现策略

**决策：** 所有异步 trait（`PlatformDriver`、`SnapshotStore`、`SessionStore`、`EventSink`、`ModelClient`）使用 `async-trait` crate 标注。

```rust
#[async_trait::async_trait]
pub trait PlatformDriver: Send + Sync {
    async fn observe(&self, req: ObserveRequest, ctx: &ExecContext) -> Result<ObserveResult, OperatorError>;
    // ...
}
```

**原因：** Rust stable 目前不支持 `dyn Trait` 中的 async fn（RFC 3185 仍未稳定）。`async-trait` 是业界标准方案，成熟且无运行时开销之外的额外复杂性。待 `dyn async fn` 稳定后可无缝迁移，仅需删除 `#[async_trait]` 标注。

---

### 26.2 `snapshot-get` 工具的执行路径

**决策：** `snapshot-get` handler 直接调用 `RuntimeCore.snapshots.get()`，**不经过** `PlatformDriver`。

```
snapshot-get handler
  → Arc<RuntimeCore>.snapshots.get(snapshot_id)
    → FileSnapshotStore::get()  （读磁盘 JSON）
```

**原因：** Snapshot 是 runtime 的存储原语，与平台无关。`PlatformDriver` 只负责实时的观察/查询/动作，不应知道 snapshot 的存储实现。此工具无需能力检查，也无 `ExecContext.target`，handler 中直接从 JSON 提取 `snapshot_id` 即可。

**对 `ToolSpec` 的影响：** `snapshot-get` 的 `capabilities_required` 为空数组 `[]`，`has_side_effects = false`。

---

### 26.3 `Drag` 的 Locator 约束

**决策：** 若 `Drag.from` 和 `Drag.to` 均为 `Locator::SnapshotElement`，两者必须引用**同一个** `SnapshotId`。

**验证时机：** 在 `RuntimeCore`（调用 driver 之前），而非在 driver 内部。

```rust
// RuntimeCore 伪代码
if let (
    Locator::SnapshotElement { snapshot: s1, .. },
    Locator::SnapshotElement { snapshot: s2, .. },
) = (&req.action, &req.action) {
    if s1 != s2 {
        return Err(OperatorError::Platform(
            "drag: from/to must reference the same snapshot".into()
        ));
    }
}
```

**原因：** 跨 snapshot 的 element ID 在平台层无法关联，driver 无法安全处理。约束前置到 RuntimeCore 可给出明确错误，避免 driver 实现各自处理（或静默错误）。

混合 Locator（如 `from: SnapshotElement`，`to: Coords`）无此约束，由 driver 自行处理。

---

### 26.4 Phase 1/2 中 Session 的处理

**决策：** Phase 1/2 中 `ExecContext.session` 字段存在但被**静默忽略**（不报错、不写入任何存储）。

**实现方式：** `operator-runtime` 内置一个 `NullSessionStore`：

```rust
pub struct NullSessionStore;

#[async_trait::async_trait]
impl SessionStore for NullSessionStore {
    async fn create(&self, _: &Session) -> Result<(), OperatorError> { Ok(()) }
    async fn append(&self, _: &SessionId, _: &SessionEvent) -> Result<(), OperatorError> { Ok(()) }
    async fn get(&self, _: &SessionId) -> Result<Option<Session>, OperatorError> { Ok(None) }
    async fn list(&self, _: Option<usize>) -> Result<Vec<SessionId>, OperatorError> { Ok(vec![]) }
}
```

`RuntimeBuilder` 默认使用 `NullSessionStore`。Phase 3 实现 `.jsonl` 文件存储后，通过 `RuntimeBuilder::session_store(impl SessionStore)` 替换，不影响 CLI / MCP 层。

**原因：** `SessionId` 字段保留在 `ExecContext` 中，是为了 Phase 3 接入时零改动。静默忽略优于报错，因为 CLI 入口默认不传 session_id，不应因此失败。

---

### 26.5 `list-windows` 工具

**决策：** `list-windows` 纳入 MVP 工具集（Section 22.3 已更新），对应 `QueryRequest::ListWindows { app: Option<String> }`。

**原因：** `QueryRequest::ListWindows` 已在 core 模型中定义，驱动实现成本低，且对 CLI 调试和未来 Agent 的窗口操作前置查询有实际价值，无理由推迟到更晚阶段。

CLI 用法：

```bash
operator list windows
operator list windows --app "Safari"
```
