# Operator Agent CLI Validation Runbook

日期：2026-03-25

## 目标

对公开 CLI 入口 `operator agent <task>` 做一轮人工辅助验证，确认：

- `gpt-5.4` 可通过统一 `operator` 二进制执行真实任务
- `doubao-seed` 可通过统一 `operator` 二进制执行真实任务
- text 输出与 `--json` 输出都可读且稳定

## 前提

- macOS 桌面会话可用
- `Notes` 已安装
- `operator permissions` 返回：
  - `accessibility: Granted`
  - `screen_recording: Granted`
- 当前 shell 已加载 provider 凭据：
  - `OPENAI_API_KEY`
  - `OPENAI_BASE_URL`
  - `ARK_API_KEY`
  - `ARK_BASE_URL`

## 执行约束

- 必须串行执行，不要并发跑多个 `operator agent`
- 每次 run 使用单独的 `OPERATOR_HOME`，避免 session 文件互相覆盖
- 每次 run 结束后，人工确认 Notes 中实际可见内容

## 预检

```bash
source ~/.zshrc
cd /Users/gokwok/code/work/Operator/.worktrees/ope-88-agent-cli-validation
mkdir -p /tmp/operator-ope88
cargo run -p operator-cli --bin operator -- permissions
```

## 验证命令

### GPT text

```bash
OPERATOR_HOME=/tmp/operator-ope88/gpt-text \
cargo run -p operator-cli --bin operator -- \
  agent \
  --model gpt-5.4 \
  "Open Notes, create a new temporary note, and type exactly two lines: OPE88 GPT TEXT 20260325 and validation token: ope88-gpt-text-20260325"
```

### GPT json

```bash
OPERATOR_HOME=/tmp/operator-ope88/gpt-json \
cargo run -p operator-cli --bin operator -- \
  --json \
  agent \
  --model gpt-5.4 \
  "Open Notes, create a new temporary note, and type exactly two lines: OPE88 GPT JSON 20260325 and validation token: ope88-gpt-json-20260325"
```

### Doubao text

```bash
OPERATOR_HOME=/tmp/operator-ope88/doubao-text \
cargo run -p operator-cli --bin operator -- \
  agent \
  --model doubao-seed \
  "Open Notes, create a new temporary note, and type exactly two lines: OPE88 DOUBAO TEXT 20260325 and validation token: ope88-doubao-text-20260325"
```

### Doubao json

```bash
OPERATOR_HOME=/tmp/operator-ope88/doubao-json \
cargo run -p operator-cli --bin operator -- \
  --json \
  agent \
  --model doubao-seed \
  "Open Notes, create a new temporary note, and type exactly two lines: OPE88 DOUBAO JSON 20260325 and validation token: ope88-doubao-json-20260325"
```

## 记录要求

每次 run 至少记录：

- 执行命令
- model
- text 或 `--json` 输出是否清晰
- Notes 中人工可见结果
- 最终结论：`pass` / `fail`

