# macOS Elements Surface Validation

日期：2026-03-28

关联 issue：`OPE-149`

## 目标

验证 `operator elements region` / `operator elements fullscreen` 在 macOS 上不再退化为“前台窗口 AX 树别名”，并记录当前真实语义边界。

## 当前语义

- `elements frontmost`
  - 仍以当前前台 app 的可检查窗口为入口。
- `elements region`
  - 先枚举桌面上可见窗口的 AX 树，再按元素 `bounds` 与请求 region 的相交关系裁剪结果。
- `elements fullscreen`
  - 聚合桌面上可见窗口的 AX 树。
- `elements fullscreen --display-id`
  - 当前保留为 northbound contract 的 best-effort hint。
  - 现阶段不会进一步缩小 macOS AX 查询范围，但命令会保留传入的 `display_id` surface 元数据。

## 实机验证

验证环境：本机 macOS GUI session，具备 Accessibility / Screen Recording / Apple Events 权限。

### 1. `elements frontmost`

命令：

```bash
cargo run -q -p operator-cli -- --json elements frontmost
```

结果摘要：

- 成功
- roots：`1`
- elements：`1`
- surface：`Frontmost`

### 2. `elements region` 小矩形裁剪

命令：

```bash
cargo run -q -p operator-cli -- --json elements region --x 0 --y 0 --width 50 --height 20
```

结果摘要：

- 成功
- roots：`1`
- elements：`1`
- 说明 region 裁剪会把结果收缩到更小的元素子树

### 3. `elements region` 较大矩形

命令：

```bash
cargo run -q -p operator-cli -- --json elements region --x 0 --y 0 --width 400 --height 220
```

结果摘要：

- 成功
- roots：`2`
- elements：`7`
- surface：`Region { rect: { x: 0, y: 0, width: 400, height: 220 } }`

### 4. `elements fullscreen`

命令：

```bash
cargo run -q -p operator-cli -- --json elements fullscreen
```

结果摘要：

- 成功
- roots：`2`
- elements：`7`
- surface：`Fullscreen { display_id: null }`

### 5. `elements fullscreen --display-id 2`

命令：

```bash
cargo run -q -p operator-cli -- --json elements fullscreen --display-id 2
```

结果摘要：

- 成功
- roots：`2`
- elements：`7`
- surface：`Fullscreen { display_id: 2 }`
- 说明当前 `display_id` 会保留在 northbound surface 中，但不会改变本轮 AX 聚合结果

## 结论

- `elements region` / `elements fullscreen` 已不再与 `elements frontmost` 返回相同的单根前台窗口树。
- `region` 语义已经收口到“多窗口枚举 + bounds 相交裁剪”。
- `fullscreen` 语义已经收口到“桌面可见窗口聚合”。
- `display_id` 仍存在平台边界，已在 help / `CLI_DESIGN.md` / `docs/COMMAND.md` 中明确标注为 best-effort 行为。
