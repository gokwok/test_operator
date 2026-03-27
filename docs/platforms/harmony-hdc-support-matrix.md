# Harmony HDC 第一阶段 Support Matrix

日期：2026-03-27

本文档区分三种状态：

- `实机验证通过`：`OPE-132` 已在真实 Harmony PC 上跑过
- `已实现，未在 OPE-132 实机覆盖`：代码与测试已存在，但本轮没有做 live rerun
- `不支持 / 已知缺口`：第一阶段明确不承诺，或当前命令面仍有偏差

## 已实机验证通过

| 命令面 | 状态 | 说明 |
|---|---|---|
| `permissions` | 实机验证通过 | `hdc.connect` / `hdc.shell` / `hdc.capture` / `hdc.ui_bridge` 全部 `Granted` |
| `capabilities` | 实机验证通过 | 返回 `AppLifecycle`、`Capture`、`KeyboardInput`、`Permissions`、`PointerInput`、`WindowQuery` |
| `list apps` | 实机验证通过 | 返回运行中 bundle 列表 |
| `list windows` | 实机验证通过 | 返回窗口列表、focus 和 bounds；结果包含系统面板窗口 |
| `app launch <bundle>` | 实机验证通过 | `com.huawei.hmos.notepad` 成功被拉起并切到前台 |
| `input click --app <bundle>` | 实机验证通过 | 成功解析目标窗口并点击中心点 |
| `input type <text> --app <bundle>` | 实机验证通过 | 成功在前台 Notepad 目标上输入文本 |
| `input hotkey <keys...> --app <bundle>` | 实机验证通过 | `ctrl+a` 成功下发到前台 Notepad 目标 |
| `observe frontmost --capture screenshot` | 实机验证通过 | 成功返回截图 snapshot，并在有 bounds 时裁剪到前台窗口 |

## 已实现，未在 OPE-132 实机覆盖

| 命令面 | 状态 | 说明 |
|---|---|---|
| `app switch <bundle>` | 已实现，未在 OPE-132 实机覆盖 | 复用 bring-to-foreground 语义 |
| `app quit <bundle>` | 已实现，未在 OPE-132 实机覆盖 | 代码路径已接入 `stop_app()` |
| `app relaunch <bundle>` | 已实现，未在 OPE-132 实机覆盖 | 代码路径已接入 `stop_app() + start_app()` |
| `input press <key>` | 已实现，未在 OPE-132 实机覆盖 | 代码路径和键位映射已存在 |
| `input swipe` | 已实现，未在 OPE-132 实机覆盖 | 第一阶段已接入 |
| `input drag` | 已实现，未在 OPE-132 实机覆盖 | 第一阶段已接入；步骤数和修饰键当前只会给 warning，不会真实下沉 |

## 不支持 / 已知缺口

| 命令面 | 状态 | 说明 |
|---|---|---|
| `observe frontmost` | 不支持 / 当前命令面漂移 | 默认仍请求 `InspectTree`，当前会报 `capability not supported: InspectTree`；第一阶段实际应改用 `--capture screenshot` |
| `focus` | 不支持 / 已知缺口 | 当前同样会落到 `InspectTree` 缺失路径 |
| `observe ... --capture elements` | 不支持 / 已知缺口 | 第一阶段不承诺 tree-backed hot-path observe |
| `input move` | 不支持 / 已知缺口 | `hmdriver_rs` 当前没有稳定 cursor move API |
| `input scroll` | 不支持 / 已知缺口 | 第一阶段未承诺 wheel / scroll 语义 |
| `app hide` / `app unhide` | 不支持 / 已知缺口 | Harmony 当前无等价 app hide/unhide 能力 |
| `window focus` / `close` / `minimize` / `maximize` | 不支持 / 已知缺口 | 第一阶段不承诺窗口 chrome 管理 |
| `window move` / `resize` / `set-bounds` | 不支持 / 已知缺口 | 第一阶段不承诺窗口几何控制 |

## 使用建议

- 若目标是当前可用的视觉自动化闭环，优先使用：
  - `list apps`
  - `list windows`
  - `app launch`
  - `observe frontmost --capture screenshot`
  - `input click/type/press/hotkey`
- 若脚本依赖“前台窗口截图”，当前不要省略 `--capture screenshot`。
- 若调用方消费 `list windows` 结果，必须自己过滤系统面板窗口。
