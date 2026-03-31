# Operator Command Surface

日期：2026-03-31

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
- 将 `capture / elements / show`、扁平交互命令、`app / window` 家族，以及 config-backed `model` 家族作为稳定命令面的持续演进方向
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
- `target <subcommand>`
- `model <subcommand>`

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
  target
    list
    show [name]
    use <name>
    set <name> --set <path=value>...
    unset <name> <path>...
    remove <name>
  model
    list
    show [name]
    use <name>
    set <name> --set <field=value>...
    unset <name> <field>...

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

### `target` 命令家族与 `--target` 的边界

- `operator target ...` 属于 `Core` 分组，用于检查和维护命名 target 配置；这是稳定 shell contract 的一部分。
- 自动化执行命令的 target 选择方式不变，仍然只通过全局 `--target <target-name>` 指定。
- `operator target ...` 不引入新的执行 target 语法，也不让 `capture` / `click` / `app` / `window` 等命令接受 `host:port`、`bridge:*`、`local:*` 之类协议形态字符串。
- target 管理命令的目标对象始终是 `.operator/config.toml` 中的 `[targets.<name>]` 条目，而不是 driver 内部传输层对象。

## 命名 Target 配置契约

`.operator/config.toml` 中每个 `[targets.<name>]` 条目的标准 envelope 固定为：

- `platform`
- `driver`
- `description`（可选）
- `driver_config`

约束：

- 除 `driver_config.*` 外，不允许把 driver-specific 字段写在 target 顶层。
- 顶层 target 字段不是开放扩展位；后续实现应拒绝未知顶层字段，而不是静默透传。
- `driver_config` 仍保持对 driver-specific key 的开放性，用于承载 `addr`、`agent_path`、`endpoint` 等差异化配置。
- `harmony.hdc` 当前默认示例必须保持最小 TCP 形式：
  - `driver = "harmony.hdc"`
  - `[targets.<name>.driver_config]`
  - `addr = "host:port"`
- Harmony 的高级覆盖项只应作为补充示例出现，不能进入默认 target 示例。

## `target` 子命令契约

### `target list`

- 列出所有已配置命名 target。
- 至少显示：
  - target 名称
  - 是否为当前 default target
  - `platform`
  - `driver`
  - 可选 `description`

### `target show [name]`

- 展示一个命名 target 的完整标准 envelope。
- 不传 `name` 时，默认展示当前 default target。

### `target use <name>`

- 将 `[runtime].default_target` 切换为指定 target 名称。
- 只修改默认 target 指针，不改动该 target 的其他字段。

### `target set <name> --set ...`

- 使用通用 path-based mutation，而不是为每个 driver 定制专门 flag。
- `--set <path=value>` 可重复传入。
- 允许写入的路径仅限：
  - `platform`
  - `driver`
  - `description`
  - `driver_config.<key>[.<nested-key>...]`

路径限制：

- 只有 `driver_config` 下面允许继续使用 dotted path。
- 禁止空 path segment、前导/尾随 `.`、数组索引语法。
- 禁止 `targets.<name>.*`、`runtime.*` 或其他未知顶层路径。

值语义：

- `value` 按 TOML value 解析，而不是一律当字符串。
- 带引号值保持字符串。
- 不带引号的 `true` / `false` / 整数 / 浮点数保留类型。
- inline array / inline table 只允许出现在 `driver_config.*` 下。
- 不支持 `null`；删除字段必须使用 `target unset`。

### `target unset <name> <path>...`

- 删除一个或多个 target 字段。
- 允许删除的路径仅限：
  - `description`
  - `driver_config.<key>[.<nested-key>...]`
- `platform` 和 `driver` 属于必填字段，不能通过 `unset` 移除。

### `target remove <name>`

- 删除整个 `[targets.<name>]` 条目。
- 若目标仍是当前 default target，后续实现应要求先切换 default target，再允许删除。

## Agent Model 配置契约

agent model/provider 配置的权威持久化位置固定为：

```toml
[agent.model]
default = "openai"

[agent.model.provider.openai]
api_key = "..."
base_url = "https://api.openai.com/v1"
model_name = "gpt-5.4"

[agent.model.provider.doubao]
api_key = "..."
base_url = "https://ark.cn-beijing.volces.com/api/v3"
model_name = "doubao-seed-2-0-lite-260215"
```

约束：

- `default` 选择 `operator agent` 在未显式传 `--model` 时使用的默认 selector。
- 当前稳定 selector 名称是：
  - `openai`
  - `doubao`
- `[agent.model.provider.<name>]` 当前只允许两条 provider entry：
  - `openai`
  - `doubao`
- selector 与 provider `model_name` 的映射是 config-backed contract：
  - `openai` selector 选择 OpenAI provider，常见 `model_name` 为 `gpt-5.4`
  - `doubao` selector 选择 Doubao provider，常见 `model_name` 为 `doubao-seed-2-0-lite-260215`
- provider entry 只允许以下字段：
  - `api_key`
  - `base_url`
  - `model_name`
- `model_name` 是实际发往远端 provider 的 model id，不等同于 northbound selector 名。
- `api_key` 可持久化在 TOML 中，但 Core inspection surface 绝不能明文显示。

### `operator agent --model` 与配置默认值的边界

- `operator agent --model <selector>` 显式覆盖 `[agent.model].default`。
- 未传 `--model` 时，默认 selector 来自 `[agent.model].default`。
- 选中的 selector 再映射到 `[agent.model.provider.<selector>]` 条目。
- 当 provider entry 中的字段缺失时，后续实现允许向环境变量回退以保留兼容性：
  - OpenAI: `OPENAI_API_KEY` / `OPENAI_BASE_URL`
  - Doubao: `ARK_API_KEY` / `DOUBAO_API_KEY` / `ARK_BASE_URL` / `DOUBAO_BASE_URL`
- CLI 兼容 alias 仍然保留在 northbound shell surface：
  - `gpt-5.4` -> `openai`
  - `doubao-seed` -> `doubao`

### `operator agent --include-elements` 的验证边界

- `operator agent --include-elements` 控制 agent 是否允许在 hot-path auto-observe 和最终完成验证中请求 `include_elements=true`。
- 默认不传时为关闭，agent 只依赖 screenshot-only observe 刷新 UI 上下文并完成 finish gate。
- 显式传入后，planner 可以在默认验证路径中使用 element tree；这适用于需要结构化 UI 校验的任务，但会提高 observe 延迟和 token 成本。
- 该开关只影响 `operator agent` 的默认验证/观察策略，不改变底层 `observe` tool 本身支持 `include_elements=true` 的能力。

### `operator agent` 默认进度输出

- 非 `--json` 路径下，`operator agent <task>` 默认向终端输出简洁的实时进度流。
- 进度流至少包含：
  - session 启动上下文
  - 轮次/turn header
  - planner 给出的下一步 summary
  - tool call 开始
  - tool result 的成功/失败摘要
  - finish gate 拒绝原因或最终完成摘要
- 该进度流是面向人的 northbound shell UX，不保证稳定的 machine-readable 形状。
- `--json` 仍保持静默执行并只输出最终结构化结果，不混入进度行。

### `model` 子命令契约

`model` 命令家族已经落地为稳定 Core shell contract：

- `model list`
- `model show [name]`
- `model use <name>`
- `model set <name> --set <field=value>...`
- `model unset <name> <field>...`

详细 help 文案、参数块顺序和示例以 [`CLI_DESIGN.md`](../CLI_DESIGN.md) 为准。

### `model list`

- 列出所有已配置 selector。
- 至少显示：
  - selector 名称
  - 是否为当前 default selector
  - provider kind
  - `model_name`
  - `base_url`
  - 脱敏后的 `api_key`

### `model show [name]`

- 展示一个 selector/provider 条目的完整标准形状。
- 不传 `name` 时，默认展示 `[agent.model].default` 指向的 selector。
- `api_key` 在 text / JSON 输出中都必须脱敏。

### `model use <name>`

- 更新 `[agent.model].default`。
- 只改默认 selector 指针，不改 provider 字段。

### `model set <name> --set ...`

- `<name>` 指向单个 provider entry，即 `[agent.model.provider.<name>]`。
- 允许写入的字段仅限：
  - `api_key`
  - `base_url`
  - `model_name`
- path 语义固定为“相对于单个 provider entry 的 field”，不允许 dotted path、绝对路径或其他顶层 key。
- 当前支持的 provider 名称仅限 `openai` / `doubao`。
- 所有字段值当前都应解析为字符串；删除字段使用 `model unset` 而不是 `null`。

### `model unset <name> <field>...`

- 从单个 provider entry 中删除一个或多个字段。
- 允许删除的字段仅限：
  - `api_key`
  - `base_url`
  - `model_name`
- `default` 不属于 `model unset` 的可删除范围；切换默认 selector 使用 `model use`。

### `api_key` 脱敏规则

- `api_key` 绝不能明文显示。
- 仅允许保留最后 4 个可见字符。
- 其余可见字符全部替换为 `*`。
- text 输出和 JSON 输出必须一致遵守该规则。

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
  - 如果 CLI 未显式传入 `--timeout-ms`，则 shell surface 会为 all-mode `app list` 注入 `30000ms` 的 runtime timeout
- `app list --name <TEXT>`：
  - 按应用名做包含匹配
  - 当前实现按大小写不敏感的 contains 规则过滤
  - Harmony 当前还会对 bundle id 做同样的 contains 匹配，避免本地化显示名替换后无法通过英文 bundle 片段定位应用
  - 如果未显式传 `--running` 或 `--all`，则默认切到 `--all`
- `app list --bundle <BUNDLE_ID>`：
  - 按 bundle id 做全匹配
  - 如果未显式传 `--running` 或 `--all`，则默认切到 `--all`
  - 可与 `--running` 或 `--all` 组合使用
- `app list --flush`：
  - 触发 Harmony `--all` app catalog 的强制刷新
  - 如果未显式传 `--running` 或 `--all`，则默认切到 `--all`
  - Harmony 的 `--all` catalog 会按 target 绑定缓存到 operator home（默认 `~/.operator`）下
  - 只有本地缓存不存在或显式传入 `--flush` 时才会重新扫描 desktop app catalog

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

`window list` 的当前 northbound CLI contract 必须显式带 `--app <APP>`：

- CLI 不再支持无过滤的全量窗口枚举路径
- 这样可以把公开 shell contract 固定在按 app 的快路径上
- 如仍需无过滤全量枚举，只允许保留为内部 runtime / driver path，不再对 CLI 调用方承诺

### Planned 系统命令

以下命令已进入设计稿，但明确不属于当前实现链：

- `clipboard get|set` `[planned]`
- `open <path-or-url>` `[planned]`

文档必须继续保留这些命令的 `[planned]` 标记，避免调用方误判为当前可用能力。

## Integration / AI 契约

- `mcp serve` 是唯一稳定的 MCP shell 入口
- `agent <task>` 是唯一稳定的自然语言任务入口
- `agent --model <selector>` 的长期权威语义由上面的 config-backed selector contract 定义
- `agent --include-elements` 是控制 agent 默认 observe/finish verification 成本与结构化校验强度的稳定 northbound 开关

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
- 仅对 `paste`、`clipboard`、`open` 保留 `[planned]` 标记
- 不展示内部 tool 名，也不展示旧命令路径

### 域 help 与叶子 help

域 help、叶子 help 的详细文案、参数块顺序、示例数量和迁移提示，以 `CLI_DESIGN.md` 为准。实现侧的 help snapshot tests 应直接围绕该设计稿收敛，而不是重新发明另一套文案。

## 当前实现链状态

截至 2026-03-28：

- `OPE-135` 到 `OPE-139` 已完成 redesign 所需的文档、help、参数解析和命令迁移
- `OPE-140` 已在真实 macOS 目标上完成 `capture` / `elements` / `show`、扁平交互命令、`app` / `window` 分组，以及 `mcp` / `agent` help 的人工辅助验证
- `OPE-171` 到 `OPE-173` 已交付 config-backed `model` 命令家族，以及 `agent --model` 的 selector/help/doc 对齐
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
- `model` 命令家族和 `agent --model` wording 与 config-backed selector contract 保持一致
- 任何后续实现 issue 都可以直接把 `CLI_DESIGN.md` 作为 help / parse 契约来源，而不会再与 `docs/COMMAND.md` 冲突
