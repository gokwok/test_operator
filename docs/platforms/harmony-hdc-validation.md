# Harmony HDC 首轮实机验证记录

日期：2026-03-27
命令面同步：2026-03-30（按当前稳定 shell contract 回写）
OPE-160 补充实测：2026-03-30
OPE-161 补充实测：2026-03-30

对应 Linear issue：`OPE-132`、`OPE-160`、`OPE-161`

## 目的

在真实 Harmony PC 目标上验证 `harmony.hdc` 第一阶段 northbound CLI 能力面，记录本轮实测通过项和已知缺口。

说明：

- `OPE-132` 实机验证发生时，CLI 仍处于 redesign 之前的旧命令面。
- 本文已按 2026-03-30 的当前稳定 shell contract 回写命令名。
- 如需对照当时的原始调用，旧路径主要为：
  - `observe frontmost --capture screenshot` -> `capture frontmost`
  - `list apps` -> `app list`
  - `list windows` -> `window list`
  - `input click|type|hotkey` -> `click|type|hotkey`

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

### 1. `capture frontmost`

命令：

```bash
operator --target harmony-pc --timeout-ms 60000 --json capture frontmost
```

实测结果：

- `OPE-132` 实机时使用的等价旧命令为 `observe frontmost --capture screenshot`
- 当前稳定命令面已收敛为 `capture frontmost`，不再要求用户显式传 `--capture screenshot`
- 成功返回 snapshot：`snapshot-1774602675208023-0`
- 成功持久化截图 artifact：`capture-1774602673278905-0.jpeg`
- 返回 `capture_bounds = { x: 1375, y: 317, width: 1594, height: 1586 }`
- 本轮截图对应前台 Notepad 窗口，而不是整屏桌面

### 2. `app list` / `window list`

命令：

```bash
operator --target harmony-pc --timeout-ms 60000 --json app list
operator --target harmony-pc --timeout-ms 60000 --json app list --all
operator --target harmony-pc --timeout-ms 60000 --json app list --name browser
operator --target harmony-pc --timeout-ms 60000 --json app list --bundle com.huawei.hmos.browser
operator --target harmony-pc --timeout-ms 60000 --json window list --app com.huawei.hmos.browser
```

结果：

- `OPE-132` 实机时对应旧命令分别为 `list apps` 和 `list windows`
- `OPE-160` 补充实测后，`app list` 当前稳定语义为 `app list --running`
- `app list` 成功返回带窗口的运行中 app 列表；本轮返回 5 个运行中 app
- `OPE-161` 后，`app list` 与 `app list --all` 都会优先显示 Harmony 提供的人类可读标签，而不是直接回落到 bundle id
- `app list --all` 现在返回的是“带桌面入口的 GUI 可操作 app catalog”，不再直接暴露原始已安装 bundle 全量；本轮返回 61 个条目，并在运行中项上回填 `is_running = true` 与 `pid`
- `app list --name` 现在同时按显示名称和 bundle id 片段做包含匹配；在当前设备上 `--name 浏览` 与 `--name browser` 都可返回 `浏览器`
- `app list --bundle com.huawei.hmos.notepad` 成功返回单个精确 bundle 匹配项，显示名为 `备忘录`
- `window list --app <bundle>` 成功返回目标窗口列表与 bounds
- 本轮在启动 Notepad 之前，前台窗口是 `com.huawei.hmos.browser` / `browser0`
- 启动 Notepad 后，窗口 `id = 282`、`title = notepad1`、`is_focused = true`

本轮 `OPE-161` 直跑 `target/debug/operator` 实测耗时：

- `app list`：`1.826s`
- `app list --all`：`1.583s`
- `app list --name browser`：实机确认仍低于 `3s`
- `app list --bundle com.huawei.hmos.notepad`：`0.430s`

结论：

- 默认 `app list`、`app list --all`、以及精确 `--bundle` 查询都低于 `3s` 目标，不构成当前 Harmony 自动化链路的性能瓶颈。

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
operator --target harmony-pc --timeout-ms 60000 --json click --app com.huawei.hmos.notepad
operator --target harmony-pc --timeout-ms 60000 --json type "OPE132 HARMONY VALIDATION 20260327" --app com.huawei.hmos.notepad
operator --target harmony-pc --timeout-ms 60000 --json hotkey ctrl a --app com.huawei.hmos.notepad
```

结果：

- `OPE-132` 实机时对应旧命令分别为 `input click`、`input type`、`input hotkey`
- `click` 成功，解析到前台 Notepad 窗口中心点 `(2172, 1110)`
- `type` 成功，返回 `typed`
- `hotkey ctrl a` 成功，返回 `sent hotkey`
- 三个动作都正确回填了 `target_app = com.huawei.hmos.notepad` 与 `target_window = notepad1`

## 本轮结论

- `harmony.hdc` 第一阶段的查询面、应用启动、以及基于 app/window target 的 `click/type/hotkey` 已经在真实 Harmony PC 上跑通。
- 第一阶段稳定截图路径应视为 `capture frontmost`；`OPE-132` 实机时跑通的是其 redesign 之前的等价旧命令。
- `show` 与任何 tree-backed `elements` 热路径仍不应被视为第一阶段稳定能力。
- 更完整的 northbound 状态见 [`docs/platforms/harmony-hdc-support-matrix.md`](./harmony-hdc-support-matrix.md)。
