# OPE-105 Validation Report

日期：2026-03-26

状态：Recorded

## 目标

验证 compact planner context 和 visual grounding 改造后的 loop，确认：

- `gpt-5.4` 与 `doubao-seed` 都能消费当前/前一张截图
- planner prompt 不再携带旧版 pretty JSON 请求转储
- 至少一条真实桌面任务在每个模型上都能完成
- live 验证中暴露的明显回归被记录并拆分后续 issue

## 自动验证

已实际运行并通过：

```bash
cargo test -p operator-agent
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Live Evidence

### Doubao

- Ready check pass:
  - transcript: `/tmp/ope105-doubao-calc-20260326/sessions/agent-1.jsonl`
  - task: `Open Calculator and verify the Calculator app is frontmost and ready for input.`
- `18 x 18` task initially暴露了坐标协议问题：
  - early failure transcript: `/tmp/ope105-doubao-calc-18x18-20260326-r2/sessions/agent-1.jsonl`
  - root cause: `doubao-seed` 输出的是以截图为参考、`basis=1000` 的相对坐标，而 runtime 当时按屏幕绝对坐标执行
- 协议修复后通过：
  - pass transcript: `/tmp/ope105-doubao-calc-18x18-20260326-r3/sessions/agent-1.jsonl`
  - final screenshot: `/tmp/ope105-doubao-calc-18x18-20260326-r3/artifacts/capture-1774524813224230-8.png`

### GPT-5.4

- 直接图片调用证明 `gpt-5.4` 返回的是截图像素坐标，不是方形归一化坐标：
  - source image: `/tmp/ope105-gpt54-ac-20260326-r1/artifacts/capture-1774525807725170-6.png`
  - returned point: `(176, 314)` on a `460 x 816` screenshot
- 协议修复后，`AC` 点击链路成功：
  - pass transcript: `/tmp/ope105-gpt54-ac-20260326-r4/sessions/agent-1.jsonl`
  - final verified screenshot: `/tmp/ope105-gpt54-ac-20260326-r4/artifacts/capture-1774527666655459-3.png`
- `Frontmost + include_elements=true` 的条带快照问题已修复，fresh observe 不再稳定退化成 `33px` 标题栏截图

## 代码修复摘要

- 为不同模型引入显式坐标协议：
  - `doubao-seed` -> surface normalized 1000
  - `gpt-5.4` -> surface image pixels
- runtime 新增 screenshot-relative locator 归一化，避免把图像像素坐标误当成屏幕绝对坐标
- macOS observe 在 `Frontmost` 模式下先解析到稳定窗口，再统一驱动 capture 与 inspect，避免截图/元素树落在不同窗口
- app-target 在 auto-focus 后补做 anchor window 刷新，并增加多种窗口回填回归测试

## Remaining Follow-up

`gpt-5.4` 的 live `Calculator AC` 任务仍有一条残留问题：

- 首次坐标点击若保留 `WindowState` / `Geometry` 校验，真实机器上仍可能报：
  - `post-action window-state verification requires target window metadata`
- 该问题已拆分到：
  - `OPE-106 Stabilize app-target window metadata for post-action GUI verification`

## 结论

`OPE-105` 的最低和主体目标已完成：

- 两个模型都完成了代表性 live task
- 坐标协议和 frontmost observe 的关键回归已被修复
- compact context + visual grounding 的新 loop 在真实桌面任务上可用

剩余的 app-target window metadata 稳定性问题已从本次验证 issue 中拆出，单独继续收口。
