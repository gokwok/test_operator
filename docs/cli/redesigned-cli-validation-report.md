# Redesigned CLI macOS Validation Report

状态：Complete

日期：2026-03-28

对应 Linear issue：`OPE-140`

## 环境

- Host platform：macOS
- 运行 target：默认 `macos`
- Target app：`TextEdit`
- 验证仓库：`/Users/gokwok/code/work/Operator`
- `OPERATOR_HOME`：`/tmp/operator-ope140`
- Validation mode：human-assisted live validation

## 自动化验证

- `cargo test --workspace`: Passed
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: Passed

## 预检

- `target/debug/operator --json permissions`: Passed
  - `accessibility: Granted`
  - `system_events: Granted`
  - `screen_recording: Granted`
- `target/debug/operator --help`: Passed
  - 根 help 分组与 `CLI_DESIGN.md` 保持一致：`Core / Observe / Interact / System / Integration / AI`
  - `paste`、`clipboard`、`open` 仍带 `[planned]`

## 人工辅助实测结果

| 命令 | 结果 | 证据 |
| --- | --- | --- |
| `operator show` | Passed | 返回 `app_name = TextEdit`、`bundle_id = com.apple.TextEdit`、`role = AXTextArea`、`label = 文本输入区` |
| `operator app list` | Passed | 返回 `126` 个运行中 app，包含 `TextEdit`（`pid = 99694`） |
| `operator window list` | Passed | 返回 `5` 个窗口；聚焦窗口为 `TextEdit` / `未命名`，`id = 15006102542963268405` |
| `operator capture frontmost` | Passed | 返回 `snapshot-1774643300926506-0` 与 `capture-1774643300499510-0.png`；`capture_bounds = { x: 146, y: 71, width: 586, height: 488 }`；图像大小 `1172x976` |
| `operator elements frontmost` | Passed | 返回 `snapshot-1774643306023750-0`；根节点为 `AXWindow` / `未命名`；包含 `AXTextArea` 节点 `ax-0-0-0` |
| `operator snapshot snapshot-1774643300926506-0` | Passed | 能按新位置参数路径回读 `capture frontmost` 的 snapshot |
| `operator artifact capture-1774643300499510-0.png` | Passed | 返回 artifact 路径 `/tmp/operator-ope140/artifacts/capture-1774643300499510-0.png`；文件大小 `68438` bytes |
| `operator type "OPE-140 CLI VALIDATION"` | Passed | 命令返回 `typed text`；随后通过 AppleScript 读回 TextEdit 前台文稿，内容为 `OPE-140 CLI VALIDATION` |
| `operator click --text OPE-140` | Passed | text locator 成功解析并点击，返回坐标 `(439, 362)` |
| `operator mcp serve --help` | Passed | Usage 为 `operator mcp serve [OPTIONS]`，描述为启动 MCP stdio server |
| `operator agent --help` | Passed | Usage 为 `operator agent [OPTIONS] <TASK>`，`--model` / `--max-steps` 选项与设计稿一致 |

## 迁移提示核对

本轮抽样验证以下旧路径，实际错误提示与 `docs/COMMAND.md` 中的迁移映射一致：

| 旧路径 | 实际提示 | 结论 |
| --- | --- | --- |
| `operator observe frontmost` | `use operator capture frontmost instead` | 与文档一致 |
| `operator observe frontmost --capture elements` | `use operator elements frontmost instead` | 与文档一致 |
| `operator list apps` | `use operator app list instead` | 与文档一致 |
| `operator focus` | `use operator show instead` | 与文档一致 |
| `operator input click --text OPE-140` | `use operator click instead` | 与文档一致 |

## 结论

- redesigned CLI 的最终 shell surface 已在真实 macOS 目标上完成一轮人工辅助验证
- 新的 `capture / elements / show`、`snapshot / artifact`、扁平 `click / type`、`app / window` 分组以及 `mcp / agent` help 都已跑通
- 旧命令路径的迁移提示与 `docs/COMMAND.md` 保持一致，没有发现需要回写的新 shell-contract 问题
- 最终支持矩阵见 [`docs/cli/redesigned-cli-command-matrix.md`](./redesigned-cli-command-matrix.md)
