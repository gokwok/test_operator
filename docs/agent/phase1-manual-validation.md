# Operator Agent Phase 1 Manual Validation

## Goal

Validate the phase-1 local runner against a real macOS desktop and a real `Notes` target for both supported models:

- `gpt-5.4`
- `doubao-seed`

This checklist is a developer validation workflow. It is not a supported public command surface.

## Preconditions

- macOS desktop session is active.
- Accessibility, Screen Recording, and any required Apple Events permissions are granted when prompted.
- `Notes` is installed and can be brought to the foreground.
- The current shell exports provider credentials temporarily only for the validation session.
- The repository is checked out on a clean issue worktree for OPE-83.

## Temporary Environment Setup

Set credentials in the current shell only:

```bash
export OPENAI_BASE_URL=<openai-base-url>
export OPENAI_API_KEY=<openai-api-key>

export ARK_BASE_URL=<doubao-api-root>
export ARK_API_KEY=<doubao-api-key>
```

Notes:

- `OPENAI_BASE_URL` is the API root used by the OpenAI Responses-compatible endpoint, for example `http://localhost:4000/v1`.
- `ARK_BASE_URL` must be the API root, not the final `/chat/completions` endpoint. For this validation use `https://ark.cn-beijing.volces.com/api/v3`.

## Harness Preflight

Run:

```bash
cargo run -p operator-agent --example local_run -- --help
```

Expected:

- help output marks the runner as a developer-only harness
- `--task`, `--target`, `--model`, and `--state-root` are listed

## Validation Matrix

Run the following 4 cases in order:

1. `gpt-5.4` normal task
2. `gpt-5.4` reflector task
3. `doubao-seed` normal task
4. `doubao-seed` reflector task

For each run:

- confirm `Notes` is available to the agent
- record the exact command
- preserve the harness output
- confirm the visible UI result manually
- clean up the temporary note before moving to the next case unless the next case intentionally reuses it

## Notes-Specific Expectations

- In a blank `Notes` note, the first line is expected to render as the note title.
- Validation should treat the note as exact two-line content, not as a separate editable title field plus body field.
- When validating the UI manually:
  - first line may appear in title styling
  - second line should remain the exact lowercase `validation token: ...` body line

## Case Template

### Common command shape

```bash
cargo run -p operator-agent --example local_run -- \
  --model <model-name> \
  --target local:macos \
  --state-root <state-root> \
  --task "<task text>"
```

### Required evidence

- exact task text
- exact command
- final harness summary
- relevant transcript excerpt
- relevant tool trace excerpt
- human observation result
- final verdict: `pass`, `partial`, or `fail`

## Cases

### Case 1: GPT-5.4 normal task

Goal:

- create a temporary note in `Notes`
- ensure the first line and second line match the assigned tokenized content exactly

Expected:

- the run finishes successfully
- `Notes` visibly shows the expected two-line content

### Case 2: GPT-5.4 reflector task

Goal:

- update a temporary note in a way that requires a fresh confirmation before finishing

Expected:

- the run either triggers reflector rejection or forces another loop before final success
- the final UI shows the expected edited content

### Case 3: Doubao normal task

Goal:

- create a temporary note in `Notes`
- ensure the first line and second line match the assigned tokenized content exactly

Expected:

- the run finishes successfully
- `Notes` visibly shows the expected two-line content

### Case 4: Doubao reflector task

Goal:

- update a temporary note in a way that requires a fresh confirmation before finishing

Expected:

- the run either triggers reflector rejection or forces another loop before final success
- the final UI shows the expected edited content

## Cleanup

After all 4 cases:

- remove any temporary validation notes left in `Notes`
- rerun automated verification:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

## Notes For Human Partner

- If a permissions dialog appears, resolve it and note the intervention in the report.
- If `Notes` is not frontmost when a run begins, bring it to a predictable state when requested.
- If the UI result is ambiguous, state exactly what is visible rather than inferring success.
- If an `observe` result collapses to a minimal top-level `AXWindow` with no readable note text after `Command+N` or `Command+A`, record it as a surface-instability event rather than a content failure.
