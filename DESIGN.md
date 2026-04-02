# Operator Design

> 跨平台自动化内核，Rust 实现；当前已提供 CLI / MCP / Agent 入口，并为 A2A 保留扩展边界。

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

> **实现状态说明（2026-04-01）：** 本文档中的核心分层、typed runtime、snapshot/capability 模型仍然有效。当前 workspace 已落地统一 `operator` CLI、`operator mcp serve`、`operator agent <task>`，并通过 `operator-bootstrap` + `system_platform_registry()` 统一装配 `macos.system` 与 `harmony.hdc`。Windows driver 仍处于规划态；Harmony 已完成第一阶段接入，但 `get-focus`、window/region observe、细粒度窗口管理等能力仍有缺口。

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

```text
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
                  Runtime / RuntimeCore
                    ┌────┴─────────────────────────────┐
                    │         │          │              │
               Snapshot/   Session    Event         Target /
               Artifact     Store      Sink      Platform Registry
                 Store                               │
                                 ┌──────────┬────────┼──────────┐
                                 │          │        │          │
                              macOS     Windows   Harmony    (future)
                              Driver     Driver    Driver

Agent Runner
  ├── ModelRegistry / ModelProvider
  ├── Tool Registry
  └── Session Journal / Store
```

**核心思路：**

- `RuntimeCore` 是核心装配对象，不持有 `ToolRegistry`，避免循环引用
- `ToolRegistry` 独立持有，handler 持有 `Arc<RuntimeCore>` 而非完整 `Runtime`
- `PlatformDriver` 是平台执行边界，对上提供 typed `observe/query/act`
- `SnapshotStore` / `ArtifactStore` / `SessionStore` 提供可共享的状态协议，但不同入口不共享进程内实例

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

- `RuntimeCore` / `RuntimeBuilder` / `Runtime`
- `TargetResolver`
- `PlatformRegistry` / `PlatformDriverFactory`
- `ToolRegistry`
- `SnapshotStore` / `ArtifactStore` / `SessionStore`
- `EventSink`

### 5.3 Platform 层

每个平台一个独立 crate；最小要求是提供 `PlatformDriver`，若需要接入统一装配层，也可以额外提供 `PlatformDriverFactory`。

- 当前已实现：`operator-platform-macos`、`operator-platform-harmony`
- 未来扩展：`operator-platform-windows`

### 5.4 Entry 层

不同入口共享同一套 runtime 构造方式、tool catalog 和状态协议：

- 当前已实现：
  - `operator-cli` — 统一用户入口，暴露 `operator` 二进制
  - `operator-mcp` — MCP 协议适配库，由 `operator mcp serve` 复用
  - `operator-agent` — 本地单 session agent runner
- 共享装配支撑：
  - `operator-bootstrap` — 负责 `config.toml` 解析/编辑、命名 target 与 model selector 管理、`system_platform_registry()` 装配
- 未来扩展：
  - A2A surface — 复用 `operator-agent` 能力向外提供 agent 协议入口

> **说明：** Agent 单独成 crate 而非内嵌于 runtime，原因是 Agent 需要模型/provider 抽象；`operator-bootstrap` 单独成 crate，则是为了让 CLI / MCP / 测试入口共享同一份配置与平台注册逻辑，而不把平台选择硬编码进每个入口。

---

## 6. Workspace 结构

长期控制在少量清晰的 crate 内；当前 workspace 已经包含 core/runtime、bootstrap、macOS / Harmony 平台、CLI / MCP / Agent 入口以及测试支撑。

```text
operator/
  Cargo.toml                   # workspace 根，声明 members 和共用依赖
  crates/
    operator-core/              # 自动化领域模型、typed 请求/响应、错误
    operator-runtime/           # RuntimeCore、ToolRegistry、存储 trait/实现
    operator-bootstrap/         # 配置加载/编辑、平台注册表与 model bootstrap
    operator-platform-macos/    # macOS system driver
    operator-platform-harmony/  # Harmony HDC driver
    operator-cli/               # 统一用户二进制：operator
    operator-mcp/               # MCP 协议适配库（无独立 bin target）
    operator-agent/             # 单 session 本地 agent runner
    operator-testkit/           # 测试工具：MockPlatformDriver、内存存储等
```

### 6.1 当前实现与未来扩展

当前 workspace members 为：

- `operator-agent`
- `operator-bootstrap`
- `operator-cli`
- `operator-core`
- `operator-mcp`
- `operator-platform-harmony`
- `operator-platform-macos`
- `operator-runtime`
- `operator-testkit`

当前入口层装配已经从“CLI / Agent 直接手工注册 `MacosDriver`”演进为：

1. `operator-bootstrap` 负责读取/编辑 `~/.operator/config.toml`
2. `RuntimeBuilder` 负责注入 `FileSnapshotStore`、`FileArtifactStore`、`FileSessionStore`
3. `system_platform_registry()` 统一注册 `macos.system` 与 `harmony.hdc`
4. `TargetResolver` 基于命名 target 解析 `platform / driver / driver_config`

因此 runtime 内核已经具备多平台、多 driver 的基础装配能力；后续新增平台时，优先新增 platform crate + factory，并通过 bootstrap registry 接入，而不是在 CLI / MCP / Agent 中各自硬编码。

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
    /// 窗口查询
    WindowQuery,
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

> **设计说明：** 当前实现已经将“窗口查询”和“窗口管理”拆分为 `WindowQuery` / `WindowManagement` 两个能力位，以便 Harmony 这类平台只声明可查询但不可管理窗口的场景。平台特有能力仍通过结构化 `CapabilityId` 扩展，不把平台差异做成裸字符串。

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

```text
Runtime
  ├── Arc<RuntimeCore>    ← ToolHandler 持有此引用（不持有 ToolRegistry）
  └── ToolRegistry        ← 持有所有 ToolHandler
```

```rust
/// 核心组件容器，不持有 ToolRegistry，可安全被 ToolHandler 引用
pub struct RuntimeCore {
    resolver: TargetResolver,
    platform_registry: PlatformRegistry,
    artifacts: Arc<dyn ArtifactStore>,
    snapshots: Arc<dyn SnapshotStore>,
    sessions: Arc<dyn SessionStore>,
    event_sink: Arc<dyn EventSink>,
    config: RuntimeConfig,
    driver_cache: Mutex<HashMap<TargetId, Arc<dyn PlatformDriver>>>,
}

/// 外层聚合，对入口层暴露
pub struct Runtime {
    core: Arc<RuntimeCore>,
    tools: ToolRegistry,
}
```

### 11.2 依赖 trait

```rust
/// artifact 解析
pub trait ArtifactStore: Send + Sync {
    async fn resolve_artifact(&self, id: &ArtifactId) -> Result<PathBuf, OperatorError>;
}

/// 快照存储
pub trait SnapshotStore: Send + Sync {
    async fn save(&self, snapshot: &Snapshot) -> Result<(), OperatorError>;
    async fn get(&self, id: &SnapshotId) -> Result<Option<Snapshot>, OperatorError>;
    async fn list(&self, target: &TargetId) -> Result<Vec<SnapshotId>, OperatorError>;
    async fn delete(&self, id: &SnapshotId) -> Result<(), OperatorError>;
    async fn resolve_artifact(&self, id: &ArtifactId) -> Result<PathBuf, OperatorError>;
    async fn evict_expired(&self) -> Result<u32, OperatorError>;
}

/// 会话存储
pub trait SessionStore: Send + Sync {
    async fn create(&self, session: &Session) -> Result<(), OperatorError>;
    async fn set_status(&self, id: &SessionId, status: SessionStatus) -> Result<(), OperatorError>;
    async fn append(&self, id: &SessionId, event: &SessionEvent) -> Result<(), OperatorError>;
    async fn get(&self, id: &SessionId) -> Result<Option<Session>, OperatorError>;
    async fn events(&self, id: &SessionId) -> Result<Vec<SessionEvent>, OperatorError>;
    async fn list(&self, limit: Option<usize>) -> Result<Vec<SessionId>, OperatorError>;
}

/// 事件输出（用于审计和调试）
pub trait EventSink: Send + Sync {
    async fn emit(&self, event: AuditEvent) -> Result<(), OperatorError>;
}
```

> **实现说明：** `RuntimeBuilder` 目前要求显式传入 `SnapshotStore`；`ArtifactStore` 若未注入，会自动回退到基于 `SnapshotStore::resolve_artifact()` 的适配层。`SessionStore` 默认为 `NullSessionStore`，入口层可按需替换为文件存储或测试实现。

### 11.3 RuntimeCore 职责

- 解析命名 target，得到带 `driver_config` 的 `TargetDescriptor`
- 通过 `PlatformRegistry` 查找 factory，实例化并缓存 per-target driver
- 能力检查，拒绝不支持的 observe/query/action
- 提供 typed `observe/query/act` 执行入口，并统一处理超时
- 保存 snapshot、管理 artifact/session 生命周期、记录审计事件
- 在动作执行前做请求归一化，在动作执行后做可选 verification（focus / geometry / window state）

### 11.4 TargetResolver

`TargetResolver` 负责把用户输入的**命名 target** 解析为结构化描述，再映射到对应的平台 driver：

```rust
pub struct TargetDescriptor {
    pub id: TargetId,
    pub platform: String,
    pub driver: String,
    pub driver_config: DriverConfig,
}
```

推荐的长期形态：

| 命名 target | 解析结果示例 |
|---|---|
| `macos` | `platform = "macos"`, `driver = "macos.system"` |
| `windows-lab` | `platform = "windows"`, `driver = "windows.remote"` |
| `harmony-pc` | `platform = "harmony"`, `driver = "harmony.hdc"` |

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
    <artifact-id>.<ext>       # image_artifact 对应的实际文件（如 png / jpeg）
  config.toml
```

**生命周期策略：**

- 每个 snapshot 可携带 `expires_at`，默认 TTL 为 24 小时（来自 `config.toml`）
- `SnapshotStore::evict_expired()` 在 `RuntimeBuilder::build()` 时调用一次（清理遗留），此后每隔 `snapshot_evict_interval` 次 `save()` 懒触发一次（默认 100）
- 最大保留数量可在 `config.toml` 中配置

### 12.2 Session

Session 已进入当前实现，用于 Agent transcript、失败回放和调试；CLI / MCP 仍然可以在不显式传 `session_id` 的情况下运行。

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

当前 `SessionStore` 还提供 `set_status()` 与 `events()` 读取接口。文件实现 `FileSessionStore` 在 `~/.operator/sessions/` 下分别持久化：

- `<session-id>.json` — session header / 当前状态
- `<session-id>.jsonl` — 事件日志（每行一个 `SessionEvent`）

`RuntimeBuilder` 默认仍可使用 `NullSessionStore`，因此不关心会话能力的入口依然可以保持近似无状态。

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
| `observe` | 按请求获取截图 / UI 元素树，生成 snapshot |
| `snapshot-get` | 获取已有 snapshot 详情 |
| `artifact-get` | 获取已持久化的截图 artifact 路径 |

**查询类（无副作用）：**

| 工具 | 说明 |
|---|---|
| `list-apps` | 列出运行中的应用或目标侧可操作 app catalog |
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

配置文件位于 `~/.operator/config.toml`；当前 CLI 只对局部字段提供显式覆盖（例如 `--target`、`--timeout-ms`、`--model`、`--max-steps`），环境变量目前主要用于 `OPERATOR_HOME` 以及 agent provider 凭据 / base URL 的回退。

```toml
[runtime]
default_target          = "macos"
snapshot_ttl_hours      = 24
max_snapshots           = 200
default_timeout_ms      = 10_000
snapshot_evict_interval = 100

[targets.macos]
platform    = "macos"
driver      = "macos.system"
description = "Built-in local macOS automation target"

[targets.windows-lab]
platform    = "windows"
driver      = "windows.remote"
description = "Shared Windows lab machine"

[targets.windows-lab.driver_config]
endpoint = "wss://lab.example"

[targets.harmony-pc]
platform    = "harmony"
driver      = "harmony.hdc"
description = "Harmony device reachable over HDC TCP"

[targets.harmony-pc.driver_config]
addr = "192.168.8.43:35319"
# optional overrides:
# connect_key = "pc-01"
# key_dir = "/Users/alice/.hdc"
# timeout_ms = 60_000
# agent_path = "/tmp/agent.so"
# remote_agent_path = "/data/local/tmp/agent.so"
# startup_delay_ms = 500

[security]
# 全局运行时闸门；关闭后所有有副作用工具都会被拒绝
allow_side_effects = true
# 审计记录是否落盘
audit_enabled = true
# 对工具输入/输出做脱敏后再落盘
redact_sensitive_fields = true
artifact_ttl_hours = 24

[agent.model]
default = "openai"

[agent.model.provider.openai]
api_key = ""
base_url = "https://api.openai.com/v1"
model_name = "gpt-5.4"
api_kind = "responses"

[agent.model.provider.doubao]
api_key = ""
base_url = "https://ark.cn-beijing.volces.com/api/v3"
model_name = "doubao-seed-2-0-lite-260215"
api_kind = "chat_completions"
```

命名 target 的标准 envelope 固定为：

- `platform`
- `driver`
- `description`（可选）
- `driver_config`

除 `description` 外，target-specific 参数必须进入 `driver_config`，例如 `endpoint`、`addr`、`agent_path`；不要再把它们写成 target 表上的顶层字段。默认 Harmony 示例应保持最小 TCP 配置，只展示 `driver_config.addr = "host:port"`；高级覆盖项只作为补充示例出现。

agent model/provider 配置契约固定为：

- `[agent.model].default`
- `[agent.model.provider.openai]`
- `[agent.model.provider.doubao]`

语义约束：

- `default` 选择 `operator agent` 在未显式传 `--model` 时使用的默认 selector。
- 当前持久化 selector 名称是 `openai` 和 `doubao`。
- selector 与 provider `model_name` 的关系固定为“northbound selector -> provider-specific model id”：
  - `openai` selector 对应 OpenAI provider，默认示例 `model_name = "gpt-5.4"`
  - `doubao` selector 对应 Doubao provider，默认示例 `model_name = "doubao-seed-2-0-lite-260215"`
- provider entry 当前只允许四个字段：
  - `api_key`
  - `base_url`
  - `model_name`
  - `api_kind`
- `api_kind` 是 provider-specific HTTP dialect；当前支持：
  - `responses`
  - `chat_completions`
- `api_kind` 缺失时，按 selector 使用默认值：
  - `openai` -> `responses`
  - `doubao` -> `chat_completions`
- `model_name` 是最终发往远端 provider 的真实模型 id；selector 名称是 northbound shell contract。
- `operator agent --model <selector>` 显式覆盖 `[agent.model].default`；未传时读取配置默认值。
- 当 provider 字段缺失时，bootstrap/agent 解析会向环境变量回退：
  - OpenAI：`OPENAI_API_KEY`、`OPENAI_BASE_URL`
  - Doubao：`ARK_API_KEY` / `DOUBAO_API_KEY`、`ARK_BASE_URL` / `DOUBAO_BASE_URL`
- CLI 兼容 alias 保留为 northbound 输入兼容层：
  - `gpt-5.4` -> `openai`
  - `doubao-seed` -> `doubao`
- Core inspection surface（`operator model list/show`）必须对 `api_key` 做脱敏：只保留最后 4 个可见字符，前面全部替换为 `*`。

> **当前边界：**
>
> - 持久化配置实际消费的 agent 子树只有 `[agent.model]`；`max_steps`、`step_timeout_ms`、`include_elements`、`observe_delay_ms` 等 loop 运行参数仍由 `operator agent` CLI 与 `AgentConfig::default()` 决定。
> - 当前配置文件尚未解析独立的 `[mcp]` section，也尚未实现 `disabled_tools` 之类的细粒度工具策略；MCP 复用 runtime 的全局 `security.allow_side_effects` 闸门。
> - 配置优先级不是单一全局链条，而是分域处理：
>   - operator home：`OPERATOR_HOME` > `~/.operator`
>   - runtime target / timeout 等调用时参数：CLI flag > runtime config > 内置默认值
>   - agent provider 凭据 / base_url：`[agent.model.provider.*]` 优先，缺失时回退到环境变量

---

## 15. CLI 设计

CLI 偏工程化、可脚本化，是 `ToolRegistry` 的一个薄包装；当前用户面已经从“平铺 tool 名”收敛成“按能力域组织的稳定 shell surface”。这层设计的权威补充说明见 [docs/COMMAND.md](docs/COMMAND.md)。

```bash
# 观察
operator capture frontmost
operator snapshot s_123

# 查询
operator window list --app TextEdit
operator show

# 输入 / 应用 / 窗口
operator click --text "Submit"
operator type "hello world" --after-key return
operator app launch Calculator
operator window resize --window-id 42 --width 900 --height 700 --verify geometry

# 配置
operator target list
operator model list

# MCP / Agent
operator mcp serve
operator agent "Open Notes and type hello"
```

**CLI 原则：**

- 所有 runtime 工具类命令支持 `--target`（默认读取配置中的 `default_target`）
- `--target` 选择命名 target，不暴露 local / remote / bridge 等连接细节
- 所有 runtime 工具类命令支持 `--json`，输出结构与 MCP 工具结果格式兼容
- 动作命令默认支持 `--timeout-ms` 覆盖超时
- `Core / Observe / Interact / Integration / AI` 只作为 help 分组标题，不作为真实一级命令
- CLI 只做参数解析、配置编辑和格式化输出，不包含平台业务逻辑

`Core` 分组除 `permissions` / `capabilities` / `snapshot` / `artifact` 外，还包含命名 target 管理家族：

- `operator target list`
- `operator target show [name]`
- `operator target use <name>`
- `operator target set <name> --set <path=value>...`
- `operator target unset <name> <path>...`
- `operator target remove <name>`

这些命令只负责检查和维护 `~/.operator/config.toml` 中的命名 target 定义；实际自动化命令仍然只通过全局 `--target <name>` 选择执行目标，不暴露 transport 或 driver routing 语法。

`AI` 分组还包含 model selector / provider 管理家族：

- `operator model list`
- `operator model show [selector]`
- `operator model use <selector>`
- `operator model set <selector> --set <field=value>...`
- `operator model unset <selector> <field>...`

这些命令只编辑 `[agent.model]` 子树，并在 read path 中统一脱敏 `api_key`。legacy 协议形态字符串（如 `local:macos`、`device:harmony:...`）不再属于 northbound contract，也不再由 runtime resolver 兜底解析。

> **当前 shell contract 细节：** CLI 的 `window list` 为了保持输出契约清晰，当前要求显式传 `--app <NAME>`；而 runtime 内部 `list-windows` tool 仍保留 `app: Option<String>` 的 typed 能力，以便 Agent / 测试或其他入口复用。

---

## 16. MCP 设计

MCP server 直接暴露同一份工具定义，不另起一套逻辑。当前实现通过统一二进制入口：

```bash
operator mcp serve
```

底层协议适配由 `operator-mcp` crate 承载，但 runtime、tool schema、target 解析与平台装配仍复用 `operator-bootstrap` + `RuntimeBuilder`。

### 16.1 Transport

当前实现只支持 **stdio** transport（兼容 Claude Desktop 等工具）；transport 选择尚未下沉到 `config.toml`。后续如需扩展 HTTP Streamable transport，应保持工具定义与执行链完全复用。

### 16.2 设计要求

- 工具 schema 直接从 `ToolRegistry` 中导出，**零重复**
- handler 与 CLI / Agent 共用同一条执行链
- 当前安全策略只有全局运行时闸门：`security.allow_side_effects = false`
- MCP 层只做协议适配（JSON-RPC 编解码），不包含平台逻辑
- 当前 server 支持 `initialize`、`notifications/initialized`、`tools/list`、`tools/call`、`ping`

### 16.3 并发处理

MCP 是多客户端协议，多个请求可能同时到达同一个 target。MVP 处理策略优先保证确定性：

| 场景 | 策略 |
|---|---|
| 同一 target，任何操作 | 当前一律串行，优先保证确定性 |
| 不同 target | 完全并发 |
| 队列超时 | 返回 `OperatorError::TargetBusy` |

> **实现说明：** 当前 `operator-mcp` 为每个 target 维护一个 `tokio::sync::Semaphore`（许可数为 1）作为串行闸门；side-effect policy 与默认 target / timeout 都从 runtime 配置注入，而不是在 MCP 层另起一套配置模型。

---

## 17. Agent 设计

Agent 不单独维护工具系统，完全复用 `ToolRegistry`。这一层已经以 `operator-agent` crate 落地，并由 `operator agent <task>` 暴露本地单 session、单 target、单 loop 的 northbound 入口；A2A surface 仍是后续扩展。

### 17.1 组件

```text
AgentRunner
  ├── ModelRegistry / ResolvedModel
  ├── PlannerPromptBuilder
  ├── DecisionParser / DecisionValidator / DecisionNormalizer
  ├── FinishGate
  ├── ToolExecutor
  ├── LoopStateContextManager / ObservationCache
  ├── SessionJournal
  └── Runtime（SessionStore + ToolRegistry）
```

当前实现特点：

- `ModelRegistry` 支持 `openai` / `doubao` 两个 selector，并保留 CLI alias：`gpt-5.4` -> `openai`、`doubao-seed` -> `doubao`
- provider identity 当前为 `OpenAi` 与 `Doubao`；HTTP dialect 通过 `api_kind` 选择 `responses` 或 `chat_completions`
- planner 不依赖 provider-native tool calling；工具目录以 prompt reference + JSON schema 方式提供给模型
- `ToolExecutor` 会根据 target `CapabilitySet` 与 `allow_side_effects` 过滤工具目录；当当前 observation 不足以支撑 selector locator 时，还会裁剪相关 schema
- loop 热状态保留在内存中，持久化由 `SessionJournal` + `SessionStore` 承担，二者分离

### 17.2 Agent Loop

```text
1. 解析 selector / provider，创建 runtime session 与 AgentSessionState
2. 通过 list-apps 建立 bootstrap app context；若显式传 --app，可预启动应用
3. 在支持 capture 的 target 上，首次规划前自动 observe；对有副作用工具执行后再自动刷新 observation
4. LoopStateContextManager + PlannerPromptBuilder 组装紧凑上下文：
   - task / notes / recent history / tool summaries
   - 当前与上一轮视觉输入
   - target capabilities / app catalog / UI stale 标志
5. 调用模型，DecisionParser 解析 JSON 决策；DecisionValidator 做 schema 校验；
   DecisionNormalizer 按模型坐标策略重写 locator
6. ToolExecutor 执行 runtime 工具，并注入 target / session_id / timeout_ms
7. PlannerRetryPolicy 处理 parse/validation 失败；RepeatedErrorPolicy 避免同类错误循环
8. FinishGate 先做确定性判定，再按需触发模型反思式收口
9. 更新 session 状态、journal 与最终 summary；支持 ctrl-c / Notify 中断
```

> **热路径说明：** 当前默认是 screenshot-first loop。`include_elements = false` 时，不把完整 UI tree 作为 planner 热路径输入；元素树主要用于 locator 解析、冷路径调试和显式校验。

### 17.3 Model / Planner 抽象

当前模型抽象不是单个 `complete()` 接口，而是 `ModelProvider::stream(ModelRequest) -> ModelStream` + `ModelRegistry`：

```rust
pub trait ModelProvider: Send + Sync + 'static {
    fn stream(&self, req: ModelRequest) -> ModelStream;
}

pub struct ModelRequest {
    pub config: ModelConfig,
    pub context: Context,
    pub options: CallOptions,
    pub stream: bool,
    pub timeout: Option<Duration>,
}

pub struct ModelConfig {
    pub provider: ProviderKind,
    pub api_kind: ApiKind,
    pub id: ModelId,
    pub coordinate_policy: CoordinatePolicy,
    pub default_options: CallOptions,
    pub default_timeout_ms: Option<u64>,
}
```

这里有两个实现相关的约束：

- `Context` 支持 `Text / Image / Thinking / ToolCall / ToolResult` block，因此当前/上一轮截图会以模型原生图片输入进入 planner 与 finish gate
- planner 的 provider-specific hint 继续绑定在真实 provider 身份上，而不是绑定在 `api_kind` 上
- `coordinate_policy` 目前是 selector 级配置的一部分：
  - OpenAI：`SurfaceImagePixels`
  - Doubao：`SurfaceNormalized1000`

这让同一份 northbound planner contract 可以在不同 provider 下复用，而坐标归一化仍能保持稳定。

---

## 18. 平台实现建议

### 18.1 macOS（当前主桌面平台）

| 能力 | 当前实现 |
|---|---|
| 截图 | `screencapture` CLI + artifact 文件落盘 + bounds/image-size 归一化 |
| UI 元素树 | `osascript -l JavaScript` 查询并展开 Accessibility tree |
| 鼠标输入 | Quartz `CGEvent` |
| 键盘输入 | Quartz `CGEvent` |
| 应用发现 / 启动 / 切换 | `NSWorkspace`、`open`、AppleScript/JXA 辅助 |
| 窗口管理 | AppleScript / AX button / bounds setter |
| 权限检测 | `AXIsProcessTrusted` + `screencapture` / `System Events` probe |
| 可选动作效果回显 | `action-effects` helper feature |

> **实现取向：** 当前 macOS driver 有意优先复用系统 CLI / script bridge 与少量 FFI，而不是一次性把所有 Cocoa / AX 能力直接包成大而全的 Rust 绑定。

### 18.2 Windows（第二阶段）

| 能力 | 建议实现方式 |
|---|---|
| 截图 | `BitBlt` / `DXGI Desktop Duplication` |
| UI 元素树 | UI Automation（`windows` crate） |
| 输入模拟 | `SendInput` |
| 应用/窗口管理 | `EnumWindows` / `ShellExecute` |
| 权限查询 | `GetForegroundWindow` / 安全上下文探测 |

### 18.3 HarmonyOS（HDC bridge，已落地第一阶段）

Harmony 侧当前不是“待调研的泛 bridge”，而是已经以 `operator-platform-harmony` crate 落地 `harmony.hdc` driver。当前实现结构为：

```text
HarmonyHdcDriverFactory
  └── HarmonyHdcDriver
        └── HarmonyHdcWorker（后台线程）
              ├── shell session
              └── UI session
```

一期实现特征：

- target 通过 `platform = "harmony"` + `driver = "harmony.hdc"` 接入
- `driver_config.addr` 为必填；其余可选覆盖项包括 `connect_key`、`key_dir`、`timeout_ms`、`agent_path`、`remote_agent_path`、`startup_delay_ms`
- 当前 capability 面为：`Capture`、`InspectTree`、`PointerInput`、`KeyboardInput`、`AppLifecycle`、`WindowQuery`、`Permissions`
- `observe` 当前只支持 `frontmost` 与 `fullscreen`；`window` / `region` surface 仍返回 unsupported
- 查询已覆盖 app catalog、running apps、windows、permissions、capabilities；`GetFocus` 仍未落地
- 动作当前覆盖 `click`、`type`、`press`、`hotkey`、`drag`、`swipe`、`launch/switch/quit/relaunch-app`；`move`、`scroll`、`hide/unhide-app`、窗口管理动作仍未实现

> **边界约束：** Harmony 特有的设备连接、mission、UI bridge 等概念继续停留在 platform crate 内，不上浮到 core 的公共 `Surface` / `Locator` / `Action` 模型。

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
| 权限检查 | 执行前检查系统权限（录屏、辅助功能、HDC 连接/UI bridge 等） |
| 能力检查 | 执行前验证 target 支持所需 `Capability` |
| 副作用标记 | 每个工具显式标注 `has_side_effects` |
| 全局副作用闸门 | `allow_side_effects = false` 时，有副作用工具返回拒绝，并写入审计事件 |
| 可配置审计日志 | 所有 tool 调用可通过 `EventSink` 记录，并支持脱敏和关闭 |
| 会话落盘 | Agent / CLI 可选接入 `SessionStore`，把 transcript 与状态单独持久化 |

**当前尚未实现**按工具粒度的 `disabled_tools` 黑名单；如后续需要，应在 runtime policy 层补齐，而不是只在 MCP 层做一次性过滤。

**初版不提供通用 shell 工具**，避免把 MCP 入口变成任意命令执行面。

---

## 22. 当前实现范围

### 22.1 当前 crate

- `operator-agent`
- `operator-bootstrap`
- `operator-cli`
- `operator-core`
- `operator-mcp`
- `operator-platform-harmony`
- `operator-platform-macos`
- `operator-runtime`
- `operator-testkit`

### 22.2 当前入口

- `operator` CLI
- `operator mcp serve`
- `operator agent <task>`

### 22.3 当前能力面

**统一 runtime tools：**

- observe / snapshot：`observe`、`snapshot-get`、`artifact-get`
- query：`list-apps`、`list-windows`、`get-focus`、`permissions-status`、`capabilities`
- pointer / keyboard：`click`、`move`、`type`、`press`、`hotkey`、`scroll`、`drag`、`swipe`
- app lifecycle：`launch-app`、`switch-app`、`quit-app`、`relaunch-app`、`hide-app`、`unhide-app`
- window management：`focus-window`、`close-window`、`minimize-window`、`maximize-window`、`move-window`、`resize-window`、`set-window-bounds`

**平台实现差异：**

- `macos.system`：已覆盖 observe、query、pointer/keyboard、app lifecycle、window management、permissions；支持可选 `action-effects` helper
- `harmony.hdc`：已覆盖 capture、inspect tree、app catalog / windows / permissions、`click/type/press/hotkey/drag/swipe`、`launch/switch/quit/relaunch-app`
  - 当前缺口：`get-focus`、`observe(window/region)`、`move`、`scroll`、`hide/unhide-app`、窗口管理动作
  - 因此“统一 tool 面”与“具体平台可用子集”之间仍存在一期差距，运行时会在 capability 或 driver 层拒绝不支持的调用

### 22.4 当前交付状态

- `operator-core` 已沉淀 typed 领域模型、locator/snapshot/action/query 原语与能力模型
- `operator-runtime` 已提供 `RuntimeBuilder`、`PlatformRegistry`、`TargetResolver`、`ToolRegistry`、typed `observe/query/act` 执行链，以及 action normalization / verification
- 文件存储已覆盖 snapshot、artifact、session 三类状态；`RuntimeBuilder` 默认仍可回退到 null session store
- `operator-bootstrap` 已提供 config 文档读写、命名 target 编辑、model selector/provider 编辑，以及 `system_platform_registry()`
- `operator-cli` 已完成统一二进制命令面，并承载 target/model 配置编辑、MCP serve 和 Agent 入口
- `operator-mcp` 已完成 stdio JSON-RPC server，并对同一 target 的并发请求做串行化
- `operator-agent` 已完成本地单 session runner、model registry、planner parser/validator/normalizer、finish gate、auto-observe、session journal 与 interrupt handling
- `operator-platform-macos` 与 `operator-platform-harmony` 都已接入统一 runtime；Windows 仍未实现

### 22.5 未来阶段

- A2A：在已有 `operator-agent` 之上补齐 northbound agent 协议入口
- Windows：补齐桌面平台 driver
- Harmony：补齐 `get-focus`、更多 observe surface、更多动作/窗口语义
- Policy：如有需要，补齐细粒度工具 allow/deny list 与更强的审计策略

---

## 23. 当前进度与推荐后续顺序

**已完成：**

1. typed core / runtime 骨架
2. 文件化 snapshot / artifact / session 存储
3. `operator-bootstrap` 配置与平台注册层
4. macOS system driver
5. Harmony HDC 第一阶段 driver
6. 统一 `operator` CLI
7. `operator mcp serve`
8. `operator agent <task>` 本地 runner

**推荐后续顺序：**

1. Windows driver
2. 补齐 Harmony 一期缺口（`get-focus`、更多 observe surface、更多动作语义）
3. 细化 runtime policy（如 `disabled_tools` / allowlist / 更强审计）
4. A2A northbound surface / 多 session 编排
5. 更高阶能力域（clipboard / dialog / menu 等）

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

**决策：** 所有异步 trait（`PlatformDriver`、`SnapshotStore`、`SessionStore`、`EventSink`、`ModelProvider` 等）使用 `async-trait` crate 标注。

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

**更新后的决策：** `ExecContext.session` 现在已经是活动字段，但仍保持“可选接入”的装配策略：

- `RuntimeBuilder` 默认仍使用 `NullSessionStore`
- 需要会话能力的入口（当前主要是 CLI / Agent）会显式注入 `FileSessionStore`
- 因此**不是**所有调用都会持久化 session；只有在入口层装配了真实 `SessionStore` 时，`session_id` 才会落盘生效

当前文件实现采用两层结构：

- `sessions/<id>.json`：`Session` header 与当前 `SessionStatus`
- `sessions/<id>.jsonl`：顺序追加的 `SessionEvent`

`SessionStore` 也已经扩展为除 `create/append/get/list` 外，再提供：

- `set_status(id, status)` — 更新 session 状态
- `events(id)` — 读取完整事件流

**原因：** 这样可以同时满足两类入口：

1. 纯 CLI / MCP 工具调用：保持接近无状态，不强制用户关心 session
2. Agent / 回放 / 调试：保留完整 transcript、状态迁移与失败诊断能力

也就是说，旧的“静默忽略 `ExecContext.session`”只在默认 `NullSessionStore` 场景下仍成立；在当前标准 CLI / Agent 装配路径中，session 已经真实持久化。

---

### 26.5 `list-windows` 工具

**决策：** `list-windows` 纳入 MVP 工具集（Section 22.3 已更新），对应 `QueryRequest::ListWindows { app: Option<String> }`。

**原因：** `QueryRequest::ListWindows` 已在 core 模型中定义，驱动实现成本低，且对 CLI 调试和未来 Agent 的窗口操作前置查询有实际价值，无理由推迟到更晚阶段。

CLI 用法：

```bash
operator list windows
operator list windows --app "Safari"
```
