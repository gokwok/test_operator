# Harmony HDC 第一阶段 Support Matrix

日期：2026-03-27
命令面同步：2026-03-30（按当前稳定 shell contract 回写）

本文档区分三种状态：

- `实机验证通过`：`OPE-132` 已在真实 Harmony PC 上跑过
- `已实现，未在 OPE-132 实机覆盖`：代码与测试已存在，但本轮没有做 live rerun
- `不支持 / 已知缺口`：第一阶段明确不承诺，或当前命令面仍有偏差

说明：

- 本表使用当前稳定命令面：`capture / elements / show / app / window / click / type / press / hotkey ...`
- `OPE-132` 原始 live run 发生在 redesign 之前；若某行备注中出现“等价旧命令”，表示实机当时验证的是旧路径，但当前稳定命令已映射到同一能力。

## 已实机验证通过

| 命令面 | 状态 | 说明 |
|---|---|---|
| `permissions` | 实机验证通过 | `hdc.connect` / `hdc.shell` / `hdc.capture` / `hdc.ui_bridge` 全部 `Granted` |
| `capabilities` | 实机验证通过 | 返回 `AppLifecycle`、`Capture`、`KeyboardInput`、`Permissions`、`PointerInput`、`WindowQuery` |
| `app list` | 实机验证通过 | `OPE-132` 实机时对应旧命令为 `list apps`；返回运行中 bundle 列表 |
| `window list` | 实机验证通过 | `OPE-132` 实机时对应旧命令为 `list windows`；返回窗口列表、focus 和 bounds；结果包含系统面板窗口 |
| `app launch <bundle>` | 实机验证通过 | `com.huawei.hmos.notepad` 成功被拉起并切到前台 |
| `click --app <bundle>` | 实机验证通过 | `OPE-132` 实机时对应旧命令为 `input click --app <bundle>`；成功解析目标窗口并点击中心点 |
| `type <text> --app <bundle>` | 实机验证通过 | `OPE-132` 实机时对应旧命令为 `input type <text> --app <bundle>`；成功在前台 Notepad 目标上输入文本 |
| `hotkey <keys...> --app <bundle>` | 实机验证通过 | `OPE-132` 实机时对应旧命令为 `input hotkey <keys...> --app <bundle>`；`ctrl+a` 成功下发到前台 Notepad 目标 |

## 已实现，未在 OPE-132 实机覆盖

| 命令面 | 状态 | 说明 |
|---|---|---|
| `capture frontmost` | 已实现，未在 OPE-132 实机覆盖 | 当前稳定等价命令；`OPE-132` 实机时跑通的是旧路径 `observe frontmost --capture screenshot`，行为为 screenshot-first frontmost capture，并在有 bounds 时裁剪到前台窗口 |
| `app switch <bundle>` | 已实现，未在 OPE-132 实机覆盖 | 复用 bring-to-foreground 语义 |
| `app quit <bundle>` | 已实现，未在 OPE-132 实机覆盖 | 代码路径已接入 `stop_app()` |
| `app relaunch <bundle>` | 已实现，未在 OPE-132 实机覆盖 | 代码路径已接入 `stop_app() + start_app()` |
| `press <key>` | 已实现，未在 OPE-132 实机覆盖 | 代码路径和键位映射已存在 |
| `swipe` | 已实现，未在 OPE-132 实机覆盖 | 第一阶段已接入 |
| `drag` | 已实现，未在 OPE-132 实机覆盖 | 第一阶段已接入；步骤数和修饰键当前只会给 warning，不会真实下沉 |

## 不支持 / 已知缺口

| 命令面 | 状态 | 说明 |
|---|---|---|
| `show` | 不支持 / 已知缺口 | 当前仍会落到 `InspectTree` 缺失路径；Harmony 第一阶段不承诺 focus/tree-backed 读取面 |
| `elements frontmost|window|region|fullscreen` | 不支持 / 已知缺口 | 第一阶段不承诺 tree-backed hot-path observe |
| `move` | 不支持 / 已知缺口 | `hmdriver_rs` 当前没有稳定 cursor move API |
| `scroll` | 不支持 / 已知缺口 | 第一阶段未承诺 wheel / scroll 语义 |
| `app hide` / `app unhide` | 不支持 / 已知缺口 | Harmony 当前无等价 app hide/unhide 能力 |
| `window focus` / `close` / `minimize` / `maximize` | 不支持 / 已知缺口 | 第一阶段不承诺窗口 chrome 管理 |
| `window move` / `resize` / `set-bounds` | 不支持 / 已知缺口 | 第一阶段不承诺窗口几何控制 |

## 旧命令迁移说明

- `observe frontmost` -> `capture frontmost`
- `focus` -> `show`
- `list apps` -> `app list`
- `list windows` -> `window list`
- `input click|type|press|hotkey|swipe|drag|move|scroll` -> `click|type|press|hotkey|swipe|drag|move|scroll`

## 使用建议

- 若目标是当前可用的视觉自动化闭环，优先使用：
  - `app list`
  - `window list`
  - `app launch`
  - `capture frontmost`
  - `click/type/press/hotkey`
- 若脚本依赖“前台窗口截图”，使用 `capture frontmost`，不要继续依赖旧的 `observe ... --capture screenshot` 路径。
- 若调用方消费 `window list` 结果，必须自己过滤系统面板窗口。
