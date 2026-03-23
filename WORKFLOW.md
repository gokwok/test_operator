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
    DEV_BRANCH="${OPERATOR_DEV_BRANCH:-codex/cli-redesign}"
    git -C "$SOURCE_REPO" rev-parse --is-inside-work-tree >/dev/null
    printf '%s\n' "$SOURCE_REPO" > .symphony-source-repo-path
    printf '%s\n' "$DEV_BRANCH" > .symphony-dev-branch
    ln -sfn "$SOURCE_REPO" source-repo
  before_run: |
    SOURCE_REPO="$(cat .symphony-source-repo-path)"
    DEV_BRANCH="$(cat .symphony-dev-branch)"
    printf 'source-repo: %s\n' "$SOURCE_REPO"
    printf 'expected-dev-branch: %s\n' "$DEV_BRANCH"
    printf 'current-branch: %s\n' "$(git -C "$SOURCE_REPO" branch --show-current)"
    git -C "$SOURCE_REPO" status --short
  after_run: |
    SOURCE_REPO="$(cat .symphony-source-repo-path)"
    git -C "$SOURCE_REPO" status --short || true
  timeout_ms: 60000

agent:
  max_concurrent_agents: 1
  max_retry_backoff_ms: 300000

codex:
  command: codex app-server
  approval_policy: never
  thread_sandbox: danger-full-access
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
3. `docs/COMMAND.md` when the current issue touches the CLI or MCP shell surface. This is mandatory for `OPE-54` through `OPE-61`.
4. The latest plan file under `docs/superpowers/plans/`
5. The current Linear issue
6. The real source repository checkout named by `.symphony-source-repo-path`
7. The current workspace state

If these sources conflict, stop scope expansion and take the most conservative path.

## Startup Sequence

Before any code change:

1. Read `AGENTS.md`
2. Read `DESIGN.md`
3. If the issue touches the CLI or MCP shell surface, read `docs/COMMAND.md`. For `OPE-54` through `OPE-61`, this step is mandatory.
4. Read the latest plan file under `docs/superpowers/plans/`
5. Read `.symphony-source-repo-path` to identify the real source repository checkout for final delivery
6. Read `.symphony-dev-branch` to identify the shared serial-development branch expected for the current chain
7. Re-check the current Linear issue state and description

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
- If `docs/COMMAND.md` applies to the current issue, treat it as the user-facing shell contract authority.
- Do not push platform-specific concepts into `operator-core`.
- If the issue acceptance commands or file paths are stale, update the issue first, then continue implementation.
- If you discover meaningful out-of-scope work, record it as a follow-up instead of expanding scope.
- The issue workspace is for orchestration metadata and transcripts only. It is not a development clone and must not hold the final commit.
- All code reads, edits, tests, and commits must happen in the real source repository checkout from `.symphony-source-repo-path`.
- For a serial issue chain that uses `.symphony-dev-branch`, do not create per-issue branches, per-issue clones, or per-issue worktrees. Reuse the shared branch in the real source repository checkout.
- If `git -C "$(cat .symphony-source-repo-path)" branch --show-current` does not match `.symphony-dev-branch`, keep the issue `In Progress`, record the blocker, and stop.
- Do not treat an isolated issue workspace, temporary `/tmp` clone, exported patch, or other packaging-only artifact as the final delivery location.
- If source-repository validation passes nowhere, or the same change has not been delivered back into the real source repository checkout from `.symphony-source-repo-path`, the issue is not done.
- If source repository `.git` metadata cannot be updated safely enough to create the required commit, keep the issue `In Progress`, record the blocker, and stop.

## Validation Rules

Before claiming completion:

1. Run the verification commands required by the current issue
2. Run the formatting and static checks required by `AGENTS.md`
3. If delivery was copied or applied back into the real source repository checkout, re-run the issue verification commands from that final delivery location before closing the issue
4. Expand validation when the change affects shared interfaces or multiple crates

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
4. A clean, traceable commit exists in the real source repository checkout named by `.symphony-source-repo-path`
5. The delivered result is no longer only in the issue workspace, a temporary clone, or an exported patch

If auth, permissions, missing Linear tools, or issue/design conflicts prevent safe completion, keep the issue in `In Progress`, record the blocker, and stop.

## Final Response

The final response should include only:

- what was completed
- what was validated
- what is still blocked

Do not ask a human to do routine next steps unless an external blocker truly requires it.
