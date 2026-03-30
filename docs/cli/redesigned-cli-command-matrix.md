# Redesigned CLI Final Command Matrix

日期：2026-03-28

本文档给出 CLI redesign 里程碑完成后的最终命令矩阵。状态定义如下：

- `本轮实机验证通过`：`OPE-140` 已在真实 macOS 目标上直接跑过
- `已实现，未在 OPE-140 live rerun`：命令已交付并有前序 issue / 测试覆盖，但本轮未重新实机执行
- `[planned] / 不在当前范围`：设计稿保留，但当前未交付

## Core

| 命令面 | 状态 | 说明 |
| --- | --- | --- |
| `permissions` | 本轮实机验证通过 | 预检返回 `accessibility`、`system_events`、`screen_recording` 全部 `Granted` |
| `capabilities` | 已实现，未在 OPE-140 live rerun | 本轮 redesign 未改动其 northbound 路径 |
| `snapshot <snapshot-id>` | 本轮实机验证通过 | 成功回读 `capture frontmost` 产出的 snapshot |
| `artifact <artifact-id>` | 本轮实机验证通过 | 成功回读 `capture frontmost` 产出的 PNG artifact |

## Observe

| 命令面 | 状态 | 说明 |
| --- | --- | --- |
| `capture frontmost` | 本轮实机验证通过 | 成功返回 screenshot snapshot 与 artifact |
| `capture window` | 已实现，未在 OPE-140 live rerun | 与 `capture frontmost` 同属 OPE-136 的新 observe 路径 |
| `capture region` | 已实现，未在 OPE-140 live rerun | 与 `capture frontmost` 同属 OPE-136 的新 observe 路径 |
| `capture fullscreen` | 已实现，未在 OPE-140 live rerun | 与 `capture frontmost` 同属 OPE-136 的新 observe 路径 |
| `elements frontmost` | 本轮实机验证通过 | 成功返回前台 TextEdit 的 AX 树 snapshot |
| `elements window` | 已实现，未在 OPE-140 live rerun | 与 `elements frontmost` 同属 OPE-136 的新 observe 路径 |
| `elements region` | 本轮实机验证通过 | `OPE-149` 已确认该命令会枚举桌面上可见窗口，并只保留 `bounds` 与 region 相交的元素子树 |
| `elements fullscreen` | 本轮实机验证通过 | `OPE-149` 已确认该命令会聚合桌面上可见窗口的 AX 树；`--display-id` 目前仍是 best-effort hint |
| `show` | 本轮实机验证通过 | 返回当前聚焦 app / window / element 摘要 |

## Interact

| 命令面 | 状态 | 说明 |
| --- | --- | --- |
| `click` | 本轮实机验证通过 | `click --text OPE-140` 成功解析可见文本并点击 |
| `type <text>` | 本轮实机验证通过 | 成功把固定文本写入 TextEdit 前台文稿 |
| `press <key>` | 已实现，未在 OPE-140 live rerun | 扁平交互路径已在 OPE-137 落地 |
| `hotkey <key>...` | 已实现，未在 OPE-140 live rerun | 扁平交互路径已在 OPE-137 落地 |
| `scroll` | 已实现，未在 OPE-140 live rerun | 扁平交互路径已在 OPE-137 落地 |
| `drag` | 已实现，未在 OPE-140 live rerun | 扁平交互路径已在 OPE-137 落地 |
| `swipe` | 已实现，未在 OPE-140 live rerun | 扁平交互路径已在 OPE-137 落地 |
| `move` | 已实现，未在 OPE-140 live rerun | 扁平交互路径已在 OPE-137 落地 |
| `paste <text>` | `[planned] / 不在当前范围` | 仍只在 help / 文档中以 `[planned]` 展示 |

## System

| 命令面 | 状态 | 说明 |
| --- | --- | --- |
| `app list` | 本轮实机验证通过 | 默认等价于 `app list --running`；成功返回运行中可操作 app 列表并包含 `TextEdit` |
| `app list --all` | 已实现，未在 OPE-140 live rerun | 显式返回系统中所有可操作 app；非运行中的条目标记为 `is_running = false` |
| `app launch` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `app` 分组 |
| `app switch` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `app` 分组 |
| `app quit` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `app` 分组 |
| `app relaunch` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `app` 分组 |
| `app hide` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `app` 分组 |
| `app unhide` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `app` 分组 |
| `window list` | 本轮实机验证通过 | 成功返回聚焦窗口与 bounds / title |
| `window focus` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `window` 分组 |
| `window close` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `window` 分组 |
| `window minimize` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `window` 分组 |
| `window maximize` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `window` 分组 |
| `window move` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `window` 分组 |
| `window resize` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `window` 分组 |
| `window set-bounds` | 已实现，未在 OPE-140 live rerun | OPE-138 已收敛到 `window` 分组 |
| `clipboard get|set` | `[planned] / 不在当前范围` | 仍只在 help / 文档中以 `[planned]` 展示 |
| `open <path-or-url>` | `[planned] / 不在当前范围` | 仍只在 help / 文档中以 `[planned]` 展示 |

## Integration / AI

| 命令面 | 状态 | 说明 |
| --- | --- | --- |
| `mcp serve` | 本轮实机验证通过 | 本轮核对了 `operator mcp serve --help` 的 usage 与描述 |
| `agent <task>` | 本轮实机验证通过 | 本轮核对了 `operator agent --help` 的 usage / options；真实 agent 执行已在更早的 OPE-88 做过单独人工验证 |

## Legacy Migration

| 旧路径 | 新路径 | 状态 |
| --- | --- | --- |
| `operator observe frontmost` | `operator capture frontmost` | 本轮实机验证通过 |
| `operator observe frontmost --capture elements` | `operator elements frontmost` | 本轮实机验证通过 |
| `operator list apps` | `operator app list` | 本轮实机验证通过 |
| `operator focus` | `operator show` | 本轮实机验证通过 |
| `operator input click` | `operator click` | 本轮实机验证通过 |
