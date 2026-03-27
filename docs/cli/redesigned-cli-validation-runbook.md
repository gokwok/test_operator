# Redesigned CLI macOS 实机验证 Runbook

日期：2026-03-28

对应 Linear issue：`OPE-140`

## 目标

在真实 macOS 桌面会话上验证 redesigned CLI 的最终 northbound shell surface，重点覆盖：

- 新的 observe/read 路径：`capture`、`elements`、`show`、`snapshot`、`artifact`
- 扁平交互路径：`click`、`type`
- 新的 system 分组：`app list`、`window list`
- integration / AI help：`mcp serve --help`、`agent --help`
- 旧命令路径迁移提示是否仍与 `docs/COMMAND.md` / `CLI_DESIGN.md` 一致

## 验证环境

- 仓库：`/Users/gokwok/code/work/Operator`
- 分支：`codex/ope-140-cli-validation`
- Host platform：macOS
- 运行 target：默认 `macos`
- 目标应用：`TextEdit`
- `OPERATOR_HOME`：`/tmp/operator-ope140`
- 验证方式：人工辅助实机验证

## 前置条件

1. 当前 shell 已在仓库根目录。
2. 本机 `Accessibility`、`System Events`、`Screen Recording` 权限全部可用。
3. 使用独立的 `OPERATOR_HOME`，避免覆盖已有 snapshot / artifact。
4. 先把 TextEdit 切到一个可重复的空白文稿状态。

## 预检

```bash
cd /Users/gokwok/code/work/Operator
cargo build -p operator-cli --bin operator
rm -rf /tmp/operator-ope140 && mkdir -p /tmp/operator-ope140

OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json permissions
target/debug/operator --help

osascript -e 'tell application "TextEdit" to activate' \
  -e 'tell application "System Events" to keystroke "n" using command down'
```

## 实测命令

### Read / Observe

```bash
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json show
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json app list
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json window list
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json capture frontmost
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json elements frontmost
```

记录 `capture frontmost` 返回的：

- `snapshot.id`
- `snapshot.image_artifact`
- `metadata.capture_bounds`

然后继续回读：

```bash
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json snapshot <snapshot-id>
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json artifact <artifact-id>
```

### Interact

先在已经聚焦的 TextEdit 文本区写入固定内容：

```bash
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json type "OPE-140 CLI VALIDATION"
osascript -e 'tell application "TextEdit" to get text of front document as string'
```

然后用刚刚写入的可见文本做 locator：

```bash
OPERATOR_HOME=/tmp/operator-ope140 target/debug/operator --json click --text OPE-140
```

### Integration / AI help

```bash
target/debug/operator mcp serve --help
target/debug/operator agent --help
```

### Legacy migration spot checks

```bash
target/debug/operator observe frontmost
target/debug/operator observe frontmost --capture elements
target/debug/operator list apps
target/debug/operator focus
target/debug/operator input click --text OPE-140
```

## 记录要求

每条命令至少记录：

- 命令本身
- 是否通过
- 关键证据（snapshot / artifact ID、focus 摘要、返回的窗口 / app、help usage、迁移提示）
- 如果命令失败，记录原始错误并停止把该命令记为“已验证通过”
