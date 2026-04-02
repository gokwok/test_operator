# Operator

Operator 是一个用 Rust 实现的跨平台自动化内核，统一 CLI、MCP、Agent 三种入口，并通过 typed runtime + platform driver 提供跨平台自动化能力。

当前仓库已经完成第一批可用入口与运行时装配：

- 统一 `operator` CLI
- `operator mcp serve`
- `operator agent <task>`
- 通过 `operator-bootstrap` + `system_platform_registry()` 统一装配 `macos.system` 与 `harmony.hdc`

权威设计与执行规范见：

- [`DESIGN.md`](./DESIGN.md)
- [`AGENTS.md`](./AGENTS.md)

## Workspace

- `crates/operator-core`: 核心领域模型与错误类型
- `crates/operator-runtime`: 运行时装配、target 解析、工具注册与存储抽象
- `crates/operator-testkit`: 测试专用 mock、fixture、内存存储
- `crates/operator-bootstrap`: 配置解析/编辑与平台注册逻辑
- `crates/operator-platform-macos`: macOS 平台 driver
- `crates/operator-platform-harmony`: Harmony HDC 平台 driver
- `crates/operator-cli`: CLI 入口
- `crates/operator-mcp`: MCP 协议适配库
- `crates/operator-agent`: 本地单 session agent runner

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```
