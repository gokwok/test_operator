# macOS Action Effects Validation Report

状态：Complete

日期：2026-03-28

对应 Linear issue：`OPE-145`

## 环境

- Host platform：macOS
- 运行 target：默认 `macos`
- 验证仓库：`/Users/gokwok/code/work/Operator`
- 验证分支：`codex/ope-145-pure-rust-effects`
- Target app：`TextEdit`
- `OPERATOR_HOME`：`/tmp/operator-ope145`
- 本地截图目录：`/tmp/ope145-effects`
- Validation mode：human-assisted live validation

## 自动化验证

- `cargo test -p operator-platform-macos`: Passed
- `cargo test -p operator-platform-macos --features action-effects`: Passed
- `cargo test -p operator-cli --features macos-action-effects`: Passed
- `cargo test --workspace`: Passed
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: Passed

## 预检

- `target/debug/operator --json permissions`: Passed
  - `Accessibility: Granted`
  - `System Events: Granted`
  - `Screen Recording: Granted`
- TextEdit 可创建未保存文稿，`window list --json` 可解析到前台 `TextEdit / 未命名` 窗口

## Helper 级视觉验证

本轮通过 `target/debug/operator __operator-macos-action-effect-helper` 的纯 Rust helper 模式，在真实桌面上抓取本地截图后人工检查。结论如下：

| 效果 | 结果 | 证据 |
| --- | --- | --- |
| `click effect` | Passed | `/tmp/ope145-effects/click.png` 中可见双击 ring / ripple |
| `move trail` | Passed | `/tmp/ope145-effects/move.png` 中可见红色 trail 与落点 pulse |
| `drag path` | Passed | `/tmp/ope145-effects/drag.png` 中可见红色路径、起点与终点 ring |
| `scroll flash` | Passed | `/tmp/ope145-effects/scroll.png` 中可见红色 flash / bar |
| `keyboard HUD` | Passed | `/tmp/ope145-effects/keyboard.png` 中可见底部 HUD，标题为 `KEYBOARD`，正文为 `cmd+shift+p` |

## `operator` CLI 集成验证

本轮使用带 `macos-action-effects` feature 的 `operator` 二进制，确认成功动作仍走真实 runtime / driver 链路，并且 target 绑定保持正确。

| 命令 | 结果 | 证据 |
| --- | --- | --- |
| `operator show --json` | Passed | 返回 `app_name = TextEdit`、`role = AXTextArea` |
| `operator window list --json` | Passed | 返回 TextEdit 前台窗口 `未命名`，bounds = `{ x: 146, y: 71, width: 586, height: 488 }` |
| `operator move --json --app TextEdit --x 439 --y 362` | Passed | 返回 `detail = moved`，坐标 `(439, 362)`，`side_effects = MoveCursor` |
| `operator click --json --app TextEdit --x 439 --y 362` | Passed | 返回 `detail = clicked`，坐标 `(439, 362)`，`side_effects = Click(Left)` |
| `operator drag --json --app TextEdit --from-x 260 --from-y 180 --to-x 620 --to-y 180` | Passed | 返回 `detail = dragged`，坐标起点 `(260, 180)` 与终点 `(620, 180)` |
| `operator scroll --json --app TextEdit --x 400 --y 260 --delta-x 0 --delta-y -160` | Passed | 返回 `detail = scrolled`，坐标 `(400, 260)`，`delta_y = -160` |
| `operator hotkey --json --app TextEdit command a` | Passed | 返回 `detail = sent hotkey`，`keys = [command, a]` |
| `operator press --json --app TextEdit delete` | Passed | 返回 `detail = pressed delete` |
| `operator type --json --app TextEdit --role AXTextArea --clear-before "OPE-145 pure rust action effects validation"` | Passed | 返回 `detail = cleared and typed text`，并解析出点击坐标 `(439, 362)` |

## 备注

- 本轮验证重点是纯 Rust effect 渲染与集成触发路径，不是重新验收所有动作语义；动作语义本身已经由 `operator-platform-macos` 合约测试覆盖。
- 在当前桌面会话中，`type` 命令虽然返回成功并解析到了 TextEdit 的 `AXTextArea`，但通过 AppleScript 回读文稿内容时未稳定得到文本回显。因此本轮没有把“文稿内容变化”作为 `keyboard HUD` 的准入证据，而是以：
  - helper 级 `keyboard HUD` 本地图像核对；
  - `type` / `hotkey` / `press` 的真实 CLI 成功返回；
  作为 effect 验收依据。
- 本轮未观察到 effect 渲染失败反向影响 action 结果，也未发现开启 `macos-action-effects` 后的 northbound 输出契约漂移。
- 纯 Rust helper 采用 `operator` 当前二进制自举内部模式，不再依赖 Swift 脚本或 `/usr/bin/swift`。

## 结论

- 第一阶段 macOS action effects 已全部可见：`click`、`move`、`drag`、`scroll`、`keyboard`
- `keyboard HUD` 已接入 `type`、`press`、`hotkey` 的成功路径，且保持 best-effort / feature-gated 设计
- 本轮验证未发现需要回写 `EFFECT_DESIGN.md` 的边界漂移
