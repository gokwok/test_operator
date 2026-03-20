# Operator

Operator 是一个用 Rust 实现的跨平台自动化内核，统一 CLI、MCP、Agent 三种入口，并通过 typed runtime + platform driver 提供跨平台自动化能力。

当前仓库处于第一阶段启动期。本次 `OPE-5` 仅完成 cargo workspace 和占位 crate 的搭建，后续能力按 `DESIGN.md`、`AGENTS.md` 与 `docs/superpowers/plans/2026-03-20-operator-implementation.md` 继续推进。

## Workspace

- `crates/operator-core`: 核心领域模型与错误类型
- `crates/operator-runtime`: 运行时装配、工具注册与存储抽象
- `crates/operator-testkit`: 测试专用 mock、fixture、内存存储
- `crates/operator-platform-macos`: macOS 平台 driver
- `crates/operator-cli`: CLI 入口

## Development

```bash
cargo fmt --check
cargo test --workspace
```
