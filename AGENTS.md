# AGENTS.md

本文件是本仓库中指导 agent 认识项目、执行实现任务、验证结果、提交代码、更新 Linear 的唯一权威规范。

## 0. 会话启动流程

每次新 agent session 开始时，必须按顺序完成以下步骤，再进行任何代码操作：

1. 读取 `DESIGN.md`，了解架构边界、核心抽象和依赖方向。
2. 读取 `docs/superpowers/plans/` 下最新日期的实施计划，确认任务列表和当前阶段。
3. 查询 Linear 项目 `Operator Implementation`，确认当前 `In Progress` issue、最后一个 `Done` issue、以及下一个待处理 issue。
4. 以 Linear 状态为准，确定本次 session 的工作范围，再开始实现。

> 跳过上述步骤直接写代码，会导致实现偏离计划或重复已完成工作。

## 1. 权威文档与优先级

按以下顺序理解和执行项目工作：

1. 本文件 `AGENTS.md`
2. 设计文档 [`DESIGN.md`](./DESIGN.md)
3. 实施计划：`docs/superpowers/plans/` 目录下最新日期的计划文件
4. 当前正在处理的 Linear issue
5. 仓库内已提交代码的实际结构

执行规则：

- `DESIGN.md` 是架构边界、核心抽象、crate 分层、执行模型的权威来源。
- 实施顺序、任务粒度、验证命令，以实施计划和对应 Linear issue 为准。
- 如果 `DESIGN.md`、实施计划、Linear issue、当前代码实现之间存在冲突，先停止扩写功能，先澄清并更新 Linear，再继续实现。

## 2. 项目定位

Operator 是一个 Rust 实现的跨平台自动化内核，目标是统一 CLI、MCP、Agent 三种入口，并通过 typed runtime + platform driver 的方式实现跨平台自动化能力。

关键约束：

- 核心设计以 [`DESIGN.md`](./DESIGN.md) 为准。
- 入口层不能反向污染 core/runtime。
- 平台差异通过 capability 和 driver 下沉，不通过入口层条件分支硬编码。
- 同一组工具定义需要同时服务 CLI、MCP、Agent。

## 3. 项目总体结构

当前仓库可能处于逐步搭建状态，但目标结构以 `DESIGN.md` 和实施计划为准。

目标 workspace 结构：

```text
operator/
  Cargo.toml
  AGENTS.md
  DESIGN.md
  crates/
    operator-core/            # Phase 1 MVP
    operator-runtime/         # Phase 1 MVP
    operator-testkit/         # Phase 1 MVP
    operator-platform-macos/  # Phase 1 MVP
    operator-cli/             # Phase 1 MVP
    operator-mcp/             # Phase 2
    operator-agent/           # Phase 3
    operator-platform-windows/  # Phase 4，暂未实现
    operator-platform-harmony/  # Phase 4，暂未实现
  docs/
    superpowers/plans/
    platforms/
```

各模块职责：

- `operator-core`
  - 领域模型、错误类型、typed request/response、ID、snapshot、locator、capability 等核心抽象。
  - 不依赖平台实现、CLI、MCP、Agent。
- `operator-runtime`
  - `RuntimeCore`、`Runtime`、`RuntimeBuilder`、`TargetResolver`、`ToolRegistry`、store traits 与文件存储。
  - 负责执行链路和运行时装配，不直接耦合具体平台 API。
- `operator-testkit`
  - `MockPlatformDriver`、内存 store、测试 fixture。
  - 只服务测试，不参与生产逻辑。
- `operator-platform-*`
  - 各平台 driver crate。
  - 只实现平台能力，不实现入口逻辑。
- `operator-cli`
  - CLI 入口，负责参数解析、调用 `ToolRegistry`、格式化输出。
- `operator-mcp`
  - MCP server 入口，负责协议适配，不重复实现业务逻辑。
- `operator-agent`
  - Agent runner 和 `ModelClient` 抽象，复用 runtime 和工具定义。
- `docs/`
  - 设计、实施计划、平台调研、补充说明。

依赖方向必须保持单向：

- entry -> runtime -> core
- platform -> core
- testkit -> core/runtime
- entry 不得直接承载平台业务逻辑
- core 不得依赖 entry、platform、LLM provider

## 4. 任务执行方式

所有实施任务都必须围绕 Linear issue 执行，Linear 是项目进度的唯一真相源。

执行规则：

- 开始编码前，先找到对应的 Linear issue。
- 读取 issue 描述、范围、验证命令、关联里程碑。
- 状态流转规则见 Section 5。
- 实现范围必须严格对齐当前 issue，不要顺手扩展到下一个 issue。
- 如果发现 issue 描述与仓库现实不一致，先更新 issue，再继续写代码。
- 完成实现后，必须先完成验证，再将 issue 更新为 `Done`。

进度判断规则：

- 当前项目进度以 Linear 项目 `Operator Implementation` 中各 issue 的状态为准。
- 不以本地代码量、未提交 diff、个人判断作为进度依据。
- 判断”当前做到哪里”时，优先查看：
  - 当前 `In Progress` issue
  - 已 `Done` 的最后一个 issue
  - 仍在 `Backlog` / `Todo` 的下一个 issue

## 5. Linear 工作流

默认使用以下 Linear 结构：

- Team: `Operator`
- Project: `Operator Implementation`
- State flow: `Backlog` -> `Todo` -> `In Progress` -> `Done`

状态流转规则：

- `Todo`：issue 已确认待做，但尚未开始实现。
- `In Progress`：**第一次修改任何文件之前**，将 issue 从 `Todo` 切换为 `In Progress`。不要在完成后才补改状态。
- `Done`：只有在 issue 范围全部完成且验证全部通过后才能设置。
- 如果工作被阻塞，不要错误地改成 `Done`；应在 issue 中记录阻塞原因，状态保持 `In Progress`。
- 一个 Linear issue 对应一个 feature 级 commit。
- 如果任务拆分发生变化，先在 Linear 中调整 issue，再修改代码。

推荐执行顺序：

- 按实施计划中的 issue 顺序推进。
- 无明确理由时，不跳过前置 issue。
- 若需要并行推进，必须确认两个 issue 在代码和职责上没有重叠冲突。

## 6. 编码规范

Rust 代码必须遵守以下规范：

- 提交前必须通过 `cargo fmt --all` 和 `cargo clippy`（具体命令见 Section 7）。
- 实现新能力或修复缺陷时，优先补测试，再补实现。
- 优先编写最小可行改动，不做无关重构。
- 模块边界、类型边界、依赖方向必须符合 `DESIGN.md`。
- 默认使用强类型接口；JSON 只应停留在入口边界和工具边界。
- 新增公共抽象前，先确认是否真的属于跨平台共性，而不是某个平台的特例。
- 除非当前文件已经大量使用，否则不要为了“优化”随意引入复杂泛型、宏或过度抽象。

文档与注释规范：

- 对外文档、设计说明、计划文档优先使用中文。
- 注释只写必要信息，解释边界、约束、非显然决策，不写废话注释。
- 如果实现偏离 `DESIGN.md`，必须在注释或文档中明确说明原因，并同步更新 Linear。

## 7. 验证规范

没有验证，不算完成。

最小验证要求：

- 受影响 crate 的测试必须运行。
- 若改动影响 workspace 公共接口，优先运行更大范围验证。
- 若 issue 中定义了明确验证命令，以 issue 为准。

提交前至少检查：

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

常见验证策略：

- 小范围改动：运行受影响 crate 或目标测试文件。
- 影响公共接口或工具注册：运行相关 crate 全量测试。
- 影响 workspace 装配、feature、共享类型：必要时运行 `cargo test --workspace`。
- 平台相关 smoke test 仅在对应平台执行，但必须在结果说明中明确是否实际运行。

禁止行为：

- 没跑命令就声称“已通过”。
- 只跑 `cargo fmt` 不跑 `clippy`。
- 明知有 warning 却继续提交，除非用户明确接受且 issue 中有记录。

## 8. Commit 提交规范

提交必须小而完整，一个 commit 对应一个 Linear issue 的完整可验证交付。

提交规则：

- 遵守 Conventional Commits，例如：
  - `feat(core): add typed automation domain models`
  - `fix(runtime): validate drag snapshot ids before driver call`
  - `docs: clarify runtime layering in agents guide`
- commit 内容必须只包含当前 issue 的范围。
- 不要把多个 issue 混在同一个 commit。
- 提交信息中必须能追溯到对应 Linear issue。
- 提交命令必须使用 `git commit -s`，保留你的 `Signed-off-by` 签名。
- 提交时必须把 Codex 作为 co-author。

推荐 commit 形式：

```bash
git commit -s -m "feat(core): add typed automation domain models" \
  -m "Refs: OPE-7" \
  -m "Co-authored-by: Codex <codex@openai.com>"
```

如果 issue 已完成并关闭，也可以使用：

```bash
git commit -s -m "feat(core): add typed automation domain models" \
  -m "Closes: OPE-7" \
  -m "Co-authored-by: Codex <codex@openai.com>"
```

## 9. 分支与变更管理

- 默认在当前任务相关分支上工作；若需要新分支，使用 Linear 自动生成的分支名（格式通常为 `username/ope-7-issue-title`），或手动使用 `ope-<id>-<short-description>` 格式，全小写，单词用连字符分隔。
- 不要重写或回滚不属于当前任务的用户改动。
- 不要使用破坏性 git 命令，例如：
  - `git reset --hard`
  - `git checkout -- <file>`
  - 强制覆盖他人未合并改动
- 如果工作树中存在与你的任务冲突的未提交改动，先暂停并澄清。

### 遇到阻塞时的处理流程

当遇到以下情况时，不要强行推进：

- 编译错误或测试失败无法自行修复
- `DESIGN.md`、实施计划、Linear issue、当前代码之间存在冲突
- issue 描述不足以确定实现范围

处理步骤：

1. 停止当前实现，不要继续写代码。
2. 在对应 Linear issue 的 comment 中记录阻塞原因（具体是什么冲突、卡在哪里）。
3. 如有 `blocked` label，打上该标签。
4. issue 状态保持 `In Progress`，不要改为 `Done`。
5. 等待用户介入澄清后，再继续。

## 10. 实现边界

以下行为默认禁止，除非对应 issue 或用户明确要求：

- 修改 `DESIGN.md` 中的核心架构决策
- 擅自引入新的 workspace crate
- 擅自扩大 MVP 范围
- 将平台特有概念提升到 core 公共接口
- 在入口层重复实现 runtime 已有逻辑
- 引入与当前任务无关的依赖

以下行为默认鼓励：

- 优先复用已有抽象
- 优先沿用 `DESIGN.md` 中既定命名
- 优先让测试和验证命令能直接映射到 issue 完成条件
- 让每个实现结果都能自然落到一个 Linear issue 和一个 feature commit 上

## 11. 实施顺序

默认按实施计划推进：

1. workspace bootstrap
2. `operator-core`
3. `operator-runtime`
4. `operator-testkit`
5. tool registry and tools
6. macOS driver
7. CLI
8. MCP
9. Agent
10. Windows/Harmony scaffolds

详细任务顺序和验证命令见 `docs/superpowers/plans/` 目录下最新日期的计划文件。

## 12. 完成标准

一个 issue 只有同时满足以下条件，才算完成：

1. 实现范围与 issue 描述一致
2. 必要测试已补齐
3. issue 中列出的验证命令已实际运行并通过
4. 代码已格式化并通过 `clippy`
5. 已形成单独、清晰、可回溯的 commit
6. commit 已包含 Linear issue 标识
7. commit 已使用 `-s`，包含你的 `Signed-off-by` 签名
8. commit 已包含 `Co-authored-by: Codex <codex@openai.com>`
9. Linear issue 状态已更新为 `Done`

如果上述任一条件不满足，则只能说“部分完成”或“实现已完成但尚未验证”，不能说“已完成”。
