---
tracker:
  kind: linear
  api_key: $LINEAR_API_KEY
  project_slug: "operator-implementation-5debc4ccffd3"
  active_states:
    - Todo
    - In Progress
  terminal_states:
    - Done
    - Duplicate
    - Canceled
    - Cancelled
    - Closed

polling:
  interval_ms: 10000

workspace:
  root: ~/code/symphony-workspaces/operator

hooks:
  after_create: |
    SOURCE_REPO="${OPERATOR_SOURCE_REPO:-/Users/gokwok/code/work/Operator}"
    git clone --depth 1 "$SOURCE_REPO" .
    git fetch --all --prune
  before_run: |
    git status --short
  after_run: |
    git status --short || true
  timeout_ms: 60000

agent:
  max_concurrent_agents: 1
  max_retry_backoff_ms: 300000

codex:
  command: codex app-server
  approval_policy: never
  thread_sandbox: workspace-write
  turn_sandbox_policy:
    type: workspaceWrite
  turn_timeout_ms: 3600000
  read_timeout_ms: 5000
  stall_timeout_ms: 300000
---

You are working on a Linear issue for the Operator repository.

Issue context:

- Identifier: `{{ issue.identifier }}`
- Title: `{{ issue.title }}`
- State: `{{ issue.state }}`
- URL: `{{ issue.url }}`
- Labels: `{{ issue.labels }}`

{% if attempt %}
This is a retry or continuation for the same issue. Reuse the current workspace state. Do not restart investigation or validation unless new code changes require it.
{% endif %}

## Authority Order

Always follow this order:

1. `AGENTS.md`
2. `DESIGN.md`
3. The latest plan file under `docs/superpowers/plans/`
4. The current Linear issue
5. The current workspace state

If these sources conflict, stop scope expansion and take the most conservative path.

## Startup Sequence

Before any code change:

1. Read `AGENTS.md`
2. Read `DESIGN.md`
3. Read the latest plan file under `docs/superpowers/plans/`
4. Re-check the current Linear issue state and description

State rules:

- If the issue is `Backlog`, stop and do not implement.
- If the issue is `Todo`, move it to `In Progress` before editing files.
- If the issue is already `In Progress`, continue the task.
- If the issue is terminal, stop.

If no Linear tool is available in the session, treat that as a real blocker and do not start coding.

## Execution Rules

- Work only on the current issue scope.
- Prefer the smallest useful change.
- Respect the layering and boundaries in `DESIGN.md`.
- Do not push platform-specific concepts into `operator-core`.
- If the issue acceptance commands or file paths are stale, update the issue first, then continue implementation.
- If you discover meaningful out-of-scope work, record it as a follow-up instead of expanding scope.

## Validation Rules

Before claiming completion:

1. Run the verification commands required by the current issue
2. Run the formatting and static checks required by `AGENTS.md`
3. Expand validation when the change affects shared interfaces or multiple crates

Minimum validation gate:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

If the issue has more specific validation commands, those take precedence.

## Commit Rules

Only commit after validation passes.

Each commit must:

- use Conventional Commits
- map one issue to one feature-level commit
- use `git commit -s`
- include `Refs: {{ issue.identifier }}` or `Closes: {{ issue.identifier }}`
- include `Co-authored-by: Codex <codex@openai.com>` in the commit body

## Done Rules

Move the issue to `Done` only when all are true:

1. The full issue scope is complete
2. The required validation commands were actually run and passed
3. `cargo fmt` and `cargo clippy` passed
4. A clean, traceable commit exists

If auth, permissions, missing Linear tools, or issue/design conflicts prevent safe completion, keep the issue in `In Progress`, record the blocker, and stop.

## Final Response

The final response should include only:

- what was completed
- what was validated
- what is still blocked

Do not ask a human to do routine next steps unless an external blocker truly requires it.
