# Harmony HDC 首轮实机验证记录

日期：2026-03-27

对应 Linear issue：`OPE-132`

## 目的

在真实 Harmony PC 目标上验证 `harmony.hdc` 第一阶段 northbound CLI 能力面，记录本轮实测通过项、当前命令面漂移，以及已知缺口。

## 验证环境

- 仓库：`/Users/gokwok/code/work/Operator`
- 目标名：`harmony-pc`
- driver：`harmony.hdc`
- 本轮实测 `addr`：`192.168.8.43:35319`
- 验证方式：人工辅助实机验证

## 前置条件

1. 目标必须已经通过 HDC 以 TCP 形式可达。
2. `harmony.hdc` 当前要求 `driver_config.addr` 为 `host:port`，不能直接使用 `hdc list targets` 返回的设备序列号。
3. 建议把本轮命令超时显式提升到 `60000ms`，避免首次连接或截图时触发默认超时。

示例配置：

```toml
[runtime]
default_target = "harmony-pc"

[targets.harmony-pc]
platform = "harmony"
driver = "harmony.hdc"

[targets.harmony-pc.driver_config]
addr = "192.168.8.43:35319"
```

## 实测命令与结果

### 0. 健康与能力

命令：

```bash
operator --target harmony-pc --timeout-ms 60000 --json permissions
operator --target harmony-pc --timeout-ms 60000 --json capabilities
```

结果：

- `permissions` 返回 `hdc.connect`、`hdc.shell`、`hdc.capture`、`hdc.ui_bridge` 全部为 `Granted`
- `capabilities` 返回：
  - `AppLifecycle`
  - `Capture`
  - `KeyboardInput`
  - `Permissions`
  - `PointerInput`
  - `WindowQuery`

### 1. `observe frontmost`

命令：

```bash
operator --target harmony-pc --timeout-ms 60000 --json observe frontmost
```

结果：

- 当前直接失败，错误为 `capability not supported: InspectTree`
- 说明 CLI 默认 `observe frontmost` 仍按 `capture=all` 请求 `InspectTree`
- 这与第一阶段 screenshot-first 预期存在 northbound shell surface 漂移

本轮可工作的替代命令：

```bash
operator --target harmony-pc --timeout-ms 60000 --json observe frontmost --capture screenshot
```

实测结果：

- 成功返回 snapshot：`snapshot-1774602675208023-0`
- 成功持久化截图 artifact：`capture-1774602673278905-0.jpeg`
- 返回 `capture_bounds = { x: 1375, y: 317, width: 1594, height: 1586 }`
- 本轮截图对应前台 Notepad 窗口，而不是整屏桌面

### 2. 应用与窗口查询

命令：

```bash
operator --target harmony-pc --timeout-ms 60000 --json list apps
operator --target harmony-pc --timeout-ms 60000 --json list windows
```

结果：

- `list apps` 成功返回运行中的 bundle 列表
- `list windows` 成功返回窗口列表与 bounds
- 本轮在启动 Notepad 之前，前台窗口是 `com.huawei.hmos.browser` / `browser0`
- 启动 Notepad 后，窗口 `id = 282`、`title = notepad1`、`is_focused = true`

注意：

- Harmony 窗口列表中会包含大量系统面板和状态栏窗口
- 调用方应优先基于 `app_name`、`is_focused`、`bounds` 做过滤，而不要把所有条目都当成普通应用窗口

### 3. 应用生命周期

命令：

```bash
operator --target harmony-pc --timeout-ms 60000 --json app launch com.huawei.hmos.notepad
```

结果：

- 成功返回 `launched com.huawei.hmos.notepad`
- 本轮验证中，Notepad 被带到前台并可继续承接输入动作

### 4. 指针与键盘输入

命令：

```bash
operator --target harmony-pc --timeout-ms 60000 --json input click --app com.huawei.hmos.notepad
operator --target harmony-pc --timeout-ms 60000 --json input type "OPE132 HARMONY VALIDATION 20260327" --app com.huawei.hmos.notepad
operator --target harmony-pc --timeout-ms 60000 --json input hotkey ctrl a --app com.huawei.hmos.notepad
```

结果：

- `input click` 成功，解析到前台 Notepad 窗口中心点 `(2172, 1110)`
- `input type` 成功，返回 `typed`
- `input hotkey ctrl a` 成功，返回 `sent hotkey`
- 三个动作都正确回填了 `target_app = com.huawei.hmos.notepad` 与 `target_window = notepad1`

## 本轮结论

- `harmony.hdc` 第一阶段的查询面、应用启动、以及基于 app/window target 的 `click/type/hotkey` 已经在真实 Harmony PC 上跑通。
- screenshot-first `observe` 可用，但当前 shell 默认 `observe frontmost` 仍不可直接作为第一阶段稳定命令，需要显式传 `--capture screenshot`。
- `focus` 及任何 tree-backed hot-path observe 仍不应被视为第一阶段稳定能力。
- 更完整的 northbound 状态见 [`docs/platforms/harmony-hdc-support-matrix.md`](./harmony-hdc-support-matrix.md)。
