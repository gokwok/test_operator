# macOS Action Effects Validation Runbook

日期：2026-03-28

对应 Linear issue：`OPE-145`

## 目标

在真实 macOS 桌面会话上验证第一阶段 action effects：

- `click effect`
- `move trail`
- `drag path`
- `scroll flash`
- `keyboard HUD`

本 runbook 同时覆盖两层验证：

- effect helper 级视觉验证：通过 `operator` 二进制内置的纯 Rust helper 模式直接渲染 overlay，并抓取截图确认特效样式与可见性
- `operator` CLI 集成验证：通过真实 action 命令确认 macOS driver 已在成功动作后触发 effect facade

## 验证环境

- 仓库：`/Users/gokwok/code/work/Operator`
- 分支：`codex/ope-145-pure-rust-effects`
- Host platform：macOS
- 运行 target：默认 `macos`
- 目标应用：`TextEdit`
- `OPERATOR_HOME`：`/tmp/operator-ope145`
- 本地截图目录：`/tmp/ope145-effects`

## 前置条件

1. 当前 shell 位于仓库根目录。
2. `Accessibility`、`System Events`、`Screen Recording` 权限均已授予。
3. 使用独立的 `OPERATOR_HOME` 和截图目录，避免污染已有状态。
4. TextEdit 能正常启动并创建一个未保存文稿。

## 预检

```bash
cd /Users/gokwok/code/work/Operator
rm -rf /tmp/operator-ope145 /tmp/ope145-effects
mkdir -p /tmp/operator-ope145 /tmp/ope145-effects

cargo build -p operator-cli --bin operator --features macos-action-effects

OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator --json permissions

osascript -e 'tell application "TextEdit" to activate' \
  -e 'tell application "TextEdit" to make new document'
```

## 视觉验证

### Helper 级截图

分别执行以下命令，并在每条命令运行期间抓一张全屏截图：

```bash
OPERATOR_INTERNAL_ACTION_EFFECT_PAYLOAD='{"kind":"click","point":{"x":735,"y":320},"mode":"double"}' \
  target/debug/operator __operator-macos-action-effect-helper

OPERATOR_INTERNAL_ACTION_EFFECT_PAYLOAD='{"kind":"move","point":{"x":900,"y":650}}' \
  target/debug/operator __operator-macos-action-effect-helper

OPERATOR_INTERNAL_ACTION_EFFECT_PAYLOAD='{"kind":"drag","from":{"x":380,"y":540},"to":{"x":1080,"y":520}}' \
  target/debug/operator __operator-macos-action-effect-helper

OPERATOR_INTERNAL_ACTION_EFFECT_PAYLOAD='{"kind":"scroll","point":{"x":1180,"y":320},"dx":0,"dy":-160}' \
  target/debug/operator __operator-macos-action-effect-helper

OPERATOR_INTERNAL_ACTION_EFFECT_PAYLOAD='{"kind":"keyboard","label":"cmd+shift+p"}' \
  target/debug/operator __operator-macos-action-effect-helper
```

记录要求：

- `click` 是否显示 ring / ripple
- `move` 是否显示 trail / 落点 pulse
- `drag` 是否显示路径、起点、终点
- `scroll` 是否显示方向 flash
- `keyboard` 是否显示带标题和主文本的 HUD

### CLI 集成验证

先确认 TextEdit 处于前台，且 `show` / `window list` 能解析到当前目标：

```bash
OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator show --json
OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator window list --json --app TextEdit
```

然后执行以下命令：

```bash
OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator move --json \
  --app TextEdit --x 439 --y 362

OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator click --json \
  --app TextEdit --x 439 --y 362

OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator drag --json \
  --app TextEdit --from-x 260 --from-y 180 --to-x 620 --to-y 180

OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator scroll --json \
  --app TextEdit --x 400 --y 260 --delta-x 0 --delta-y -160

OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator hotkey --json \
  --app TextEdit command a

OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator press --json \
  --app TextEdit delete

OPERATOR_HOME=/tmp/operator-ope145 target/debug/operator type --json \
  --app TextEdit --role AXTextArea --clear-before \
  "OPE-145 action effects validation"
```

记录要求：

- 每条命令是否返回成功
- `target_app` / `target_window` 是否仍指向 TextEdit
- 指针动作是否返回坐标
- 键盘动作是否返回正确的 `detail`
- 如需留痕，可在命令运行时额外抓取全屏截图

## 判定标准

满足以下条件即可认为本轮 effects 验证通过：

1. helper 级五种特效都已人工观察到，并能在截图中辨认；
2. `operator` CLI 的 `move` / `click` / `drag` / `scroll` / `hotkey` / `press` / `type` 都返回成功；
3. 未发现 effect 渲染失败导致 action 失败、卡死或返回模型漂移；
4. effect 仍保持 feature-gated 与 best-effort 行为。
