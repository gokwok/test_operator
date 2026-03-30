# Operator Command Surface

日期：2026-03-28

## 目的

本文件定义 Operator CLI 的命令面规范摘要。

自 `OPE-135` 起，仓库根目录的 [`CLI_DESIGN.md`](../CLI_DESIGN.md) 成为当前 CLI redesign 链的**详细 shell contract**：它负责定义根 help、域 help、叶子 help、示例、迁移映射和 `[planned]` 标记的完整文案。本文件负责：

- 用中文总结当前稳定命令树与分组语义
- 约束哪些命令属于本轮 redesign 链的实现范围
- 记录全局参数、共享参数组和旧命令迁移方向

两份文档必须同步维护；如果二者发生冲突，以 `CLI_DESIGN.md` 的命令树与 help 文案为准，并在同一 issue 中回写本文件，避免 authority drift。

## 设计目标

- 提供面向 agent / script 的稳定 northbound shell interface
- 把帮助导航统一收敛到 `Core / Observe / Interact / System / Integration / AI`
- 用少量、可探索的一级命令承载能力，而不是泄露内部 tool 名
- 将 `capture / elements / show`、扁平交互命令和 `app / window` 家族作为下一条实现链的目标命令面
- 明确标记尚未实现、但已进入设计保留的命令

## 非目标

- 不保留旧的 `observe / list / focus / input` 命令路径作为长期 shell contract
- 不在本轮 redesign 文档同步中引入新的 runtime 或平台能力
- 不把 `paste`、`clipboard`、`open` 视为当前实现链必须落地的范围
- 不把内部类型名、tool registry 或 driver 路由语法暴露给 shell 用户

## 权威关系

- `CLI_DESIGN.md`
  - 当前 redesign 链的详细命令契约
  - 权威定义根 help、域 help、叶子 help、示例与迁移表
- `docs/COMMAND.md`
  - 中文规范摘要
  - 权威定义文档层面的范围边界、稳定分组语义与共享参数约束

当前链路上如果要调整 CLI 命令面，应先修改 `CLI_DESIGN.md`，再同步本文件；不要只改其中之一。

## 根 help 分组

根 help 必须按以下顺序展示分组：

- `Core`
- `Observe`
- `Interact`
- `System`
- `Integration`
- `AI`

这些分组只承担导航职责，不是额外的一级命令前缀。

### Core

- `permissions`
- `capabilities`
- `snapshot <snapshot-id>`
- `artifact <artifact-id>`

### Observe

- `capture <surface>`
- `elements <surface>`
- `show`

### Interact

- `click`
- `type <text>`
- `press <key>`
- `hotkey <key>...`
- `scroll`
- `drag`
- `swipe`
- `move`
- `paste <text>` `[planned]`

### System

- `app <subcommand>`
- `window <subcommand>`
- `clipboard <subcommand>` `[planned]`
- `open <path-or-url>` `[planned]`

### Integration

- `mcp serve`

### AI

- `agent <task>`

## 稳定命令树

```text
operator
  permissions
  capabilities
  snapshot <snapshot-id>
  artifact <artifact-id>

  capture
    frontmost
    window
    region
    fullscreen

  elements
    frontmost
    window
    region
    fullscreen

  show

  click
  type
  press
  hotkey
  scroll
  drag
  swipe
  move
  paste              [planned]

  app
    list
    launch
    switch
    quit
    relaunch
    hide
    unhide

  window
    list
    focus
    close
    minimize
    maximize
    move
    resize
    set-bounds

  clipboard          [planned]
    get
    set

  open <path-or-url> [planned]

  mcp
    serve

  agent <task>
```

## 全局运行时参数

所有已交付或规划中的命令共享以下全局参数：

- `--json`
- `--target <target-name>`
- `--timeout-ms <ms>`

### `--target` 语义

`--target` 只选择一个**命名 target**。northbound shell surface 不暴露 transport、bridge、driver routing 或协议形态。以下字符串不属于用户面契约：

- `local:macos`
- `device:ios:123`
- `bridge:harmony`
- `windows.remote`

用户只传 target 名称，实际解析到哪个 platform / driver / driver_config，由 runtime 配置负责。

## Observe / Read 契约

### `capture`

- 负责截图导向的观察路径
- 支持 `frontmost` / `window` / `region` / `fullscreen`
- `window` 需要 `--window-id <id>`
- `region` 需要 `--x --y --width --height`
- `fullscreen` 可选 `--display-id <id>`

### `elements`

- 负责无障碍元素树查询
- 支持与 `capture` 相同的 surface 子命令
- surface 参数规则与 `capture` 保持一致
- macOS 当前实现说明：
  - `elements region` 会枚举桌面上可见窗口的 AX 树，并只保留 `bounds` 与请求 region 相交的元素子树
  - `elements fullscreen` 会聚合桌面上可见窗口的 AX 树
  - `elements fullscreen --display-id` 目前仅保留为 best-effort hint，尚不会进一步缩小 macOS AX 查询范围
- 详细实机记录见 [`docs/platforms/macos-elements-surface-validation.md`](./platforms/macos-elements-surface-validation.md)

### `show`

- 负责显示当前聚焦的 app / window / element 摘要
- 取代旧的 `focus` 读命令

### `snapshot` / `artifact`

- `snapshot <snapshot-id>`
- `artifact <artifact-id>`

对象 id 一律使用位置参数，不再保留 `snapshot-get` / `artifact-get` 一类旧路径。

## Interact 契约

交互动作从旧的 `input <subcommand>` 收敛为根级命令：

- `click`
- `type`
- `press`
- `hotkey`
- `scroll`
- `drag`
- `swipe`
- `move`

### 共享 locator 参数组

单 locator 命令共享以下互斥定位方式：

- `--text <text>`
- `--role <role> [--index <n>]`
- `--snapshot <id> --element <id>`
- `--x <x> --y <y>`

双 locator 命令继续使用：

- `--from-*`
- `--to-*`

### 共享 target 参数组

当命令需要指向 app / window 时，使用以下 selector flags：

- `--app <name-or-bundle-id>`
- `--pid <pid>`
- `--window-id <id>`
- `--window-title <title>`
- `--window-index <index>`

这些 selector 继续互斥，不暴露 runtime 内部类型名。

### 共享行为参数

- `--focus <auto|never>`
- `--verify <focus|geometry|window-state>`

`type`、`press`、`hotkey` 的主载荷使用位置参数：

- `type <text>`
- `press <key>`
- `hotkey <key>...`

`type` 的尾随按键使用：

- `--after-key <key>`

### Planned 交互命令

- `paste <text>` 处于 `[planned]`
- 当前 redesign 链不会在 `OPE-135` 中实现该命令
- 后续是否实现，取决于单独 issue 是否接入 clipboard runtime 能力

## System 契约

### `app`

应用相关命令统一收敛到 `app` 家族：

- `app list`
- `app launch`
- `app switch`
- `app quit`
- `app relaunch`
- `app hide`
- `app unhide`

`app launch` 的主载荷采用位置参数形式：

- `app launch <bundle-id-or-name>`

`app list` 的当前 northbound 语义分成两个显式模式：

- `app list` 与 `app list --running` 等价：
  - 返回当前正在运行、且当前至少拥有一个窗口的可操作应用列表
  - macOS 结果来自原生 running application 枚举，并用 Core Graphics 窗口 owner 集合过滤
  - 默认排除 `.prohibited` 的 background-only processes
- `app list --all`：
  - 返回当前系统中所有可操作的应用列表
  - macOS 通过扫描标准 app bundle 目录并与运行中的 app 集合合并来生成结果
  - 非运行中的应用会保留 `is_running = false`，并且没有 `pid`
- `app list --name <TEXT>`：
  - 按应用名做包含匹配
  - 当前实现按大小写不敏感的 contains 规则过滤
  - 如果未显式传 `--running` 或 `--all`，则默认切到 `--all`
- `app list --bundle <BUNDLE_ID>`：
  - 按 bundle id 做全匹配
  - 如果未显式传 `--running` 或 `--all`，则默认切到 `--all`
  - 可与 `--running` 或 `--all` 组合使用

### `window`

窗口相关命令统一收敛到 `window` 家族：

- `window list`
- `window focus`
- `window close`
- `window minimize`
- `window maximize`
- `window move`
- `window resize`
- `window set-bounds`

`window list` 在未指定 `--app` 时属于显式全量枚举路径：

- macOS 上这条路径可能明显更慢
- 已知目标 app 时，应优先使用 `window list --app <APP>`

### Planned 系统命令

以下命令已进入设计稿，但明确不属于当前实现链：

- `clipboard get|set` `[planned]`
- `open <path-or-url>` `[planned]`

文档必须继续保留这些命令的 `[planned]` 标记，避免调用方误判为当前可用能力。

## Integration / AI 契约

- `mcp serve` 是唯一稳定的 MCP shell 入口
- `agent <task>` 是唯一稳定的自然语言任务入口

根 help 中这两类命令分别归入：

- `Integration`
- `AI`

不再使用旧的 `MCP` / `Agent` 分组命名。

## 迁移映射

旧命令路径与新命令路径的稳定迁移关系如下：

| 旧路径 | 新路径 |
| --- | --- |
| `operator observe frontmost` | `operator capture frontmost` |
| `operator observe window` | `operator capture window` |
| `operator observe region` | `operator capture region` |
| `operator observe fullscreen` | `operator capture fullscreen` |
| `operator observe frontmost --capture elements` | `operator elements frontmost` |
| `operator observe window --capture elements` | `operator elements window` |
| `operator list apps` | `operator app list` |
| `operator list windows` | `operator window list` |
| `operator focus` | `operator show` |
| `operator input click` | `operator click` |
| `operator input type` | `operator type` |
| `operator input press` | `operator press` |
| `operator input hotkey` | `operator hotkey` |
| `operator input scroll` | `operator scroll` |
| `operator input drag` | `operator drag` |
| `operator input swipe` | `operator swipe` |
| `operator input move` | `operator move` |

## help 契约

### 根 help

根 help 必须：

- 以 `Usage` 开头
- 在 `Usage` 后展示 slogan
- 只展示域命令与根级叶子命令
- 按 `Core / Observe / Interact / System / Integration / AI` 分组
- 列出全局运行时参数
- 对 `paste`、`clipboard`、`open` 保留 `[planned]` 标记
- 不展示内部 tool 名，也不展示旧命令路径

### 域 help 与叶子 help

域 help、叶子 help 的详细文案、参数块顺序、示例数量和迁移提示，以 `CLI_DESIGN.md` 为准。实现侧的 help snapshot tests 应直接围绕该设计稿收敛，而不是重新发明另一套文案。

## 当前实现链状态

截至 2026-03-28：

- `OPE-135` 到 `OPE-139` 已完成 redesign 所需的文档、help、参数解析和命令迁移
- `OPE-140` 已在真实 macOS 目标上完成 `capture` / `elements` / `show`、扁平交互命令、`app` / `window` 分组，以及 `mcp` / `agent` help 的人工辅助验证
- 最终实测 runbook、验证报告与命令矩阵见：
  - [`docs/cli/redesigned-cli-validation-runbook.md`](./cli/redesigned-cli-validation-runbook.md)
  - [`docs/cli/redesigned-cli-validation-report.md`](./cli/redesigned-cli-validation-report.md)
  - [`docs/cli/redesigned-cli-command-matrix.md`](./cli/redesigned-cli-command-matrix.md)
- `paste`、`clipboard`、`open` 在本链条内持续保持 `[planned]`，不应被提前视为已交付能力

## 验收准则

本轮 CLI redesign 文档链至少要满足：

- `docs/COMMAND.md` 与 `CLI_DESIGN.md` 中的命令树、分组命名、planned 标记保持一致
- `capture`、`elements`、`show`、`app list`、`window list`、扁平交互命令的迁移方向清晰可查
- `paste`、`clipboard`、`open` 被明确标注为 `[planned]`
- 任何后续实现 issue 都可以直接把 `CLI_DESIGN.md` 作为 help / parse 契约来源，而不会再与 `docs/COMMAND.md` 冲突
