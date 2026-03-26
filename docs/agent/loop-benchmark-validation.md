# Operator Agent Loop Benchmark Validation

日期：2026-03-26

状态：Recorded

## 目标

为 `OPE-98` 记录 redesigned `operator-agent` loop 与 `operation_agent/agent_v3` baseline 的人工辅助验证结果，重点关注：

- 首步延迟
- 平均 step 延迟
- completion outcome

## 口径与限制

- 当前 loop 的 live desktop 样本运行于 macOS，本仓库入口使用 `cargo run -p operator-agent --example local_run -- ...`。
- `agent_v3` 在本机仓库中的可成功样本全部来自历史日志目录 `/Users/gokwok/code/work/operation_agent/agent_v3/logs`。
- 2026-03-26 对 `agent_v3` 做了一次 live rerun，想用当前环境直接复跑“打开 Calculator 并确认可输入”这一任务；该 rerun 在 step 1 即失败，没有拿到可用 model output。
- 因为当前 workspace 中没有可直接驱动 macOS desktop 的 `agent_v3` 入口，且本次 live rerun 失败，所以本报告把 `agent_v3` 的成功基线视为 historical baseline，同时单独记录这次 live rerun failure。
- 当前 loop 的 persisted transcript 不保存 step 级时间戳，因此本次首步/平均 step 延迟来自外部 watcher 对 session log 落盘时刻的观测。该方法足以用于相对比较，但不是 runtime 内建 telemetry。

## Live Current-Loop Evidence

### Case A: Calculator ready, attended live run

- Date: 2026-03-26
- Command:

```bash
cargo run -p operator-agent --example local_run -- \
  --model doubao-seed \
  --state-root /tmp/ope98-op-1 \
  --task "Open Calculator and verify the Calculator app is frontmost and ready for input."
```

- Outcome: Passed
- Evidence:
  - persisted transcript: `/tmp/ope98-op-1/sessions/agent-1.jsonl`
  - terminal summary confirmed completion after `launch-app -> observe -> get-focus -> observe -> finish`
  - final transcript includes:
    - `focus.app_name = "Calculator"`
    - final `observe(include_elements=true)` captured the Calculator window and interactive buttons
- Timing notes:
  - transcript-level timing summary reported `observe capture_duration_ms total=4943`
  - this run was collected before adding the external watcher, so it does not have a separately captured step-latency series

### Case B: Calculator ready, unattended timed rerun

- Date: 2026-03-26
- Probe state root: `/tmp/ope98-operator-calc-single`
- Outcome: Timed out after 60s watchdog
- Measured latency:
  - first-step latency: `4443ms`
  - average step latency: `4293.2ms`
- Measured step sequence:
  - `launch-app` at `4443ms`
  - `observe` at `4443ms`
  - `get-focus` at `7716ms`
  - `switch-app` at `20306ms`
  - `list-windows` at `23479ms`
  - `permissions-status` at `26041ms`
  - `launch-app` at `29689ms`
  - `observe` at `29689ms`
  - `list-windows` at `33188ms`
  - `observe` at `41463ms`
  - `get-focus` at `47225ms`
- Failure character:
  - rerun entered a recovery loop after repeated `get-focus -> null` and `list-windows -> (-1728)` responses
  - this means the redesigned loop is faster on the first step than the historical `agent_v3` samples below, but unattended completion on this specific desktop task is still not stable enough to call “fully reliable”

## agent_v3 Baseline Evidence

### Case C: Live rerun attempt on 2026-03-26

- Session id: `ope98-calc-err`
- Log root: `/tmp/ope98-agentv3-logs/ope98-calc-err`
- Command shape:

```bash
GUI_AGENT_LOG_DIR=/tmp/ope98-agentv3-logs \
uv run python -m thinkflow.runtime \
  --package . \
  --host 127.0.0.1 --port 8080 \
  --tools-upstream-url http://127.0.0.1:9001/mcp \
  --lpu-upstream-url "$ARK_BASE_URL" \
  --lpu-upstream-api-key "$ARK_API_KEY"
```

- Payload task: `打开计算器，并确认它已经打开且可以输入。`
- Outcome: Failed before first model output
- Evidence:
  - `request.json` for `step_0001/attempt_01` was created
  - `planner_error.txt` records:

```text
Model call failed (base_url=http://localhost:59509/v1): Connection error.
```

- Interpretation:
  - current `agent_v3` live rerun is blocked by the session-local LPU proxy path in the current environment
  - this failure is part of the benchmark result because it blocks any apples-to-apples live comparison on this machine today

### Case D: Historical multi-step success baseline

- Source session: `/Users/gokwok/code/work/operation_agent/agent_v3/logs/device-12_1770109339791_371944`
- Task: `微博打开通知免打扰`
- Outcome: Passed
- Steps: `8`
- Measured from consecutive `request.json.timestamp` values:
  - first-step latency: `6941ms`
  - average step latency: `7860.6ms`
- Final action: `finish`

### Case E: Historical short success baseline

- Source session: `/Users/gokwok/code/work/operation_agent/agent_v3/logs/phase4-acc-c1`
- Task: `Phase4验收任务：请先向我提问“是否同意本次验收？”...`
- Outcome: Passed
- Steps: `2`
- Measured from consecutive `request.json.timestamp` values:
  - first-step latency: `4991ms`
  - average step latency: `4991.0ms`
- Final action: `finish`

### Case F: Historical short success baseline (slow variant)

- Source session: `/Users/gokwok/code/work/operation_agent/agent_v3/logs/phase4-debug2-c1`
- Task: `Phase4验收任务：请先向我提问‘是否同意本次验收？’...`
- Outcome: Passed
- Steps: `2`
- Measured from consecutive `request.json.timestamp` values:
  - first-step latency: `12136ms`
  - average step latency: `12136.0ms`
- Final action: `finish`

## Summary Table

| Case | Loop | Task shape | Outcome | First-step latency | Avg step latency |
| --- | --- | --- | --- | --- | --- |
| A | current loop | desktop app launch + ready verify | pass | n/a | n/a |
| B | current loop | desktop app launch + ready verify | timeout at 60s | `4443ms` | `4293.2ms` |
| C | `agent_v3` live rerun | same Calculator prompt | fail at step 1 | n/a | n/a |
| D | `agent_v3` historical | multi-step in-app settings flow | pass | `6941ms` | `7860.6ms` |
| E | `agent_v3` historical | short `call_user -> finish` flow | pass | `4991ms` | `4991.0ms` |
| F | `agent_v3` historical | short `call_user -> finish` flow | pass | `12136ms` | `12136.0ms` |

## Findings

1. On the only directly measured current-loop desktop rerun with step telemetry, first-step latency was lower than every successful historical `agent_v3` sample inspected in this workspace.
2. The redesigned loop can complete the desktop Calculator task under attended conditions, but the unattended rerun still exposed focus/window-resolution flake (`get-focus -> null`, `list-windows -> -1728`).
3. `agent_v3` could not be rerun successfully in the current environment on 2026-03-26 because the session-local LPU proxy path failed before the first planner output.
4. The practical conclusion for `OPE-98` is not “the benchmark is universally green”; it is:
   - the redesigned loop is directionally faster on first action latency
   - the new loop already demonstrates real desktop completion
   - stability work is still needed before unattended success rate can be claimed as consistently better

## Review Note

本报告已提交到仓库，供 `OPE-98` 的人工审阅使用；对应 live evidence 和 historical baseline 路径均在本文中给出，便于复核。
