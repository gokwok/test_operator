# Operator Command Surface

日期：2026-03-23

## 目的

本文档定义 Operator 的新 CLI 命令面。

目标不是继续暴露 runtime 的 tool registry，而是提供一个面向 agent/script 的稳定 shell interface。CLI 只负责：

- 以可探索的命令树承载能力
- 把用户面参数映射到现有 typed runtime/tool contract
- 用一致的 help 和 example 降低 agent 的探索成本

本文档不改变 `operator-core` / `operator-runtime` / `operator-platform-*` 的分层，也不把 `Core / Observe / Query / Action / MCP / A2A` 变成真正的一级命令。这些词只用于 help 展示分组。

## 设计目标

- 面向 agent/script，而不是人工记忆内部 tool 名
- 一级命令数量有限，支持渐进式揭露
- 帮助信息按执行模型分组，但真实命令按对象域和任务域组织
- 最终统一为 `operator` 一个主二进制
- `operator-mcp` 入口并入 `operator mcp serve`
- 当前未实现的 A2A 只在 help 中保留标题，不伪造空能力

## 非目标

- 不追求兼容现有平铺命令
- 不把 tool registry 暴露为用户入口
- 不在本次重设计里实现新的平台能力或新的 runtime 能力
- 不在本次重设计里实现 A2A

## 核心原则

### 1. help 分组只承担导航作用

`Core / Observe / Query / Action / MCP / A2A` 只出现在 help 里，用于指导用户和 agent 发现命令；它们不是 shell contract 的一部分。

### 2. 真实命令保持少而稳

CLI 真实暴露的顶层命令应只有：

- `permissions`
- `capabilities`
- `observe`
- `snapshot`
- `artifact`
- `list`
- `focus`
- `input`
- `app`
- `window`
- `mcp`

### 3. snapshot 是一等原语

`observe` 负责创建 snapshot；`snapshot get` / `artifact get` 负责取回持久化产物。`observe` 不再和 `snapshot-get` / `artifact-get` 并列成三个平铺命令。

### 4. 内部抽象不直接泄露到用户面

用户不会看到：

- tool 名
- `ToolRegistry`
- `ActionTargetSelector`
- `ActionFocusPolicy`

但这些 typed 抽象继续保留在 runtime 和 driver 内部。

## 最终命令树

```text
operator
  permissions
  capabilities

  observe
    frontmost
    window
    region
    fullscreen

  snapshot
    get

  artifact
    get

  list
    apps
    windows

  focus

  input
    click
    move
    type
    press
    hotkey
    scroll
    drag
    swipe

  app
    launch
    switch
    quit
    relaunch
    hide
    unhide

  window
    focus
    close
    minimize
    maximize
    move
    resize
    set-bounds

  mcp
    serve
```

`A2A` 不作为真实命令出现；只在根 help 中以 `reserved` 文案保留。

## help 分组

根 help 按以下分组展示：

- `Core`
  - `permissions`
  - `capabilities`
- `Observe`
  - `observe`
  - `snapshot`
  - `artifact`
- `Query`
  - `list`
  - `focus`
- `Action`
  - `input`
  - `app`
  - `window`
- `MCP`
  - `mcp`
- `A2A`
  - `reserved`

根 help 不直接列出 `click`、`launch-app`、`snapshot-get` 这种叶子命令。

## 参数体系

### 全局运行时参数

所有命令共享，并且应作为真正的 global flags 工作：

- `--json`
- `--target <target>`
- `--timeout-ms <ms>`

### Observe 参数

#### `observe frontmost`

- 不再需要 `--surface frontmost`
- 支持 `--capture <all|elements|screenshot|none>`

#### `observe window`

- 必需：`--window-id <id>`
- 支持 `--capture <all|elements|screenshot|none>`

#### `observe fullscreen`

- 可选：`--display-id <id>`
- 支持 `--capture <all|elements|screenshot|none>`

#### `observe region`

- 必需：`--x --y --width --height`
- 支持 `--capture <all|elements|screenshot|none>`

`--capture` 是用户面概念，内部继续映射为 runtime 现有的 `include_screenshot` / `include_elements`。

默认值：`all`

理由：

- `observe` 默认就应该产出有价值的 snapshot
- agent/script 在第一次探索时不应拿到“既没有 screenshot 也没有 elements”的弱结果

### 持久化对象读取参数

对象 id 改为位置参数：

- `snapshot get <snapshot-id>`
- `artifact get <artifact-id>`

不再继续使用：

- `snapshot-get --snapshot-id ...`
- `artifact-get --artifact-id ...`

### Query 参数

- `list apps`
- `list windows [--app <name>]`
- `focus`

`focus` 直接是叶子命令，不再使用 `get-focus` 或 `focus get`。

### Action target selector 参数

这些参数只在需要指向 app/window 的 action 命令里出现：

- `--app <name-or-bundle-id>`
- `--pid <pid>`
- `--window-id <id>`
- `--window-title <title>`
- `--window-index <index>`

约束：

- 这些 selector 继续互斥
- 用户面只说“target flags”或“selector flags”，不暴露 runtime 内部类型名

### focus policy 参数

用户面从：

- `--focus-policy <auto|never>`

收敛为：

- `--focus <auto|never>`

内部继续映射到 `ActionFocusPolicy`。

### locator 参数

#### 单 locator 参数组

供以下命令共享：

- `input click`
- `input move`
- `input type`
- `input scroll`

支持以下几种互斥定位方式：

- `--snapshot <id> --element <id>`
- `--text <text>`
- `--role <role> [--index <n>]`
- `--x <x> --y <y>`

#### 双 locator 参数组

供以下命令共享：

- `input drag`
- `input swipe`

继续使用：

- `--from-*`
- `--to-*`

这部分当前设计已经契合 runtime 的 locator 模型，不需要改成别的抽象。

### verification 参数

继续统一使用：

- `--verify <focus|geometry|window-state>`

但只在 runtime 真实支持的命令上暴露：

- `app launch`：不暴露
- `window close`：不暴露
- `window maximize`：不暴露
- `window minimize`：仅暴露 `window-state`

这与 `OPE-53` 后的 runtime 契约保持一致。

### 动作主载荷位置参数

以下命令改为位置参数输入，而不是长 flag：

- `input type <text>`
- `input press <key>`
- `input hotkey <key>...`
- `app launch <bundle-id-or-name>`

`type` 的尾随按键建议改名为：

- `--after-key <key>`

以取代当前的 `--trailing-key`。

## 代表性命令

```bash
operator permissions
operator capabilities --json

operator observe frontmost --capture all
operator observe window --window-id 42 --capture elements
operator observe region --x 0 --y 44 --width 1280 --height 720

operator snapshot get s_123
operator artifact get capture-1.png

operator list apps
operator list windows --app TextEdit
operator focus

operator input click --text Save --app Notes --focus auto --verify focus
operator input move --x 240 --y 320
operator input type "hello operator" --window-title Draft --after-key return
operator input press tab --count 2
operator input hotkey command shift p
operator input scroll --delta-y -3 --window-id 42
operator input drag --from-x 100 --from-y 100 --to-x 400 --to-y 220
operator input swipe --from-x 900 --from-y 400 --to-x 200 --to-y 400 --duration-ms 300

operator app launch Calculator
operator app switch --app TextEdit
operator app quit --pid 101

operator window focus --window-id 42 --verify focus
operator window minimize --window-id 42 --verify window-state
operator window resize --window-id 42 --width 900 --height 700 --verify geometry

operator mcp serve
```

## help 契约

### 根 help

根 help 必须：

- 只展示域命令
- 按 `Core / Observe / Query / Action / MCP / A2A` 分组
- 列出全局运行时参数
- 不展示内部 tool 名
- 不展示旧平铺命令

### 域 help

例如 `operator input --help`、`operator app --help`、`operator window --help`，必须展示：

- 该域下的子命令
- 该域共享的参数组
- 1-2 条示例

### 叶子命令 help

例如 `operator input click --help`、`operator window resize --help`，必须展示：

- 一行用途说明
- `Usage`
- 命令特有参数
- 共享参数组
- verification 约束
- 2-4 条 example

### 稳定性要求

help 输出要进入 snapshot tests，保证：

- 分组标题顺序稳定
- 命令顺序稳定
- 关键 example 稳定

## 二进制形态

最终用户面只保留一个主二进制：

- `operator`

`operator-mcp` 应并入：

- `operator mcp serve`

`operator-cli` 的 crate 可以继续存在，但 shell contract 最终以 `operator` 为准。

## 兼容性策略

最终交付不保留旧平铺命令。

会被移除的典型命令包括：

- `observe --surface ...`
- `snapshot-get`
- `artifact-get`
- `list-apps`
- `list-windows`
- `get-focus`
- `click`
- `move`
- `type`
- `launch-app`
- `focus-window`

如果实现过程中需要短期 hidden alias 作为迁移手段，可以在中间提交存在；但最终 help 和最终交付不得保留双轨命令面。

## 实施拆分

### [OPE-54](https://linear.app/aios-operator/issue/OPE-54/48-introduce-grouped-cli-information-architecture-and-root-help)

Introduce grouped CLI information architecture and root help

- 建立新顶层命令骨架
- 建立 help 分组标题
- 建立全局参数体系
- 迁移 `permissions` / `capabilities` / `list` / `focus`

### [OPE-55](https://linear.app/aios-operator/issue/OPE-55/49-redesign-observe-snapshot-and-artifact-command-surface)

Redesign observe, snapshot, and artifact command surface

- 迁移 `observe`
- 迁移 `snapshot get`
- 迁移 `artifact get`
- 引入 `--capture`

### [OPE-56](https://linear.app/aios-operator/issue/OPE-56/50-redesign-input-command-family)

Redesign input command family

- 迁移所有 pointer/keyboard 动作到 `input`
- 收敛 locator / selector / focus / verify 参数面
- 引入 `type <text>` / `press <key>` / `hotkey <key>...`

### [OPE-57](https://linear.app/aios-operator/issue/OPE-57/51-redesign-app-command-family)

Redesign app command family

- 迁移 `app launch/switch/quit/relaunch/hide/unhide`
- 收敛 app lifecycle 用户面

### [OPE-58](https://linear.app/aios-operator/issue/OPE-58/52-redesign-window-command-family)

Redesign window command family

- 迁移 `window focus/close/minimize/maximize/move/resize/set-bounds`
- 保持 `OPE-53` 后的 verification 约束

### [OPE-59](https://linear.app/aios-operator/issue/OPE-59/53-unify-mcp-under-operator-mcp-serve)

Unify MCP under `operator mcp serve`

- 把 MCP 入口接到统一 CLI
- 为后续退役 `operator-mcp` 做准备

### [OPE-60](https://linear.app/aios-operator/issue/OPE-60/54-rename-primary-binary-to-operator-and-remove-legacy-flat-commands)

Rename primary binary to `operator` and remove legacy flat commands

- 正式切换壳层契约
- 删除旧平铺命令

### [OPE-61](https://linear.app/aios-operator/issue/OPE-61/55-polish-cli-help-output-examples-and-snapshot-coverage)

Polish CLI help output, examples, and snapshot coverage

- 固化根 help / 域 help / 叶子 help
- 加 examples
- 加 help snapshot tests
- 在根 help 里保留 `A2A` heading 的 `reserved` 文案

## blocker 链

- `OPE-55` blocked by `OPE-54`
- `OPE-56` blocked by `OPE-54`
- `OPE-57` blocked by `OPE-54`
- `OPE-58` blocked by `OPE-54`
- `OPE-59` blocked by `OPE-54`
- `OPE-60` blocked by `OPE-55`, `OPE-56`, `OPE-57`, `OPE-58`, `OPE-59`
- `OPE-61` blocked by `OPE-60`

## 验收准则

每张 CLI 重设计 issue 至少要满足：

- 新命令 parse 测试通过
- 旧命令拒绝测试在需要移除的阶段通过
- help snapshot tests 通过
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`

最终完成态应满足：

- 根 help 只展示新的命令树
- `Core / Observe / Query / Action / MCP / A2A` 只作为 help 分组标题存在
- tool registry 对 shell 用户完全不可见
- 用户通过 `operator --help` 可以逐层探索全部已实现能力
