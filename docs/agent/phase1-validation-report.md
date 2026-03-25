# Operator Agent Phase 1 Validation Report

Status: Complete

## Environment

- Date: 2026-03-25
- Host platform: macOS
- Primary target: `Notes`
- Validation mode: human-assisted live validation

## Summary Matrix

| Model | Case | Goal | Result | Reflector evidence | Notes |
| --- | --- | --- | --- | --- | --- |
| `gpt-5.4` | Normal | Create a temporary note with exact two-line content | Passed | N/A | Live rerun `A7` succeeded end-to-end; earlier reruns still exposed intermittent minimal AX snapshots after `Command+N` / `Command+A` |
| `gpt-5.4` | Reflector | Force re-check before finish on blank-note completion | Passed | Prompt-compliant under adversarial instructions | `A8` completed with `hotkey -> observe -> finish`; `A11` still chose `observe` before `finish` even when the task text tried to push an immediate finish |
| `doubao-seed` | Normal | Create a temporary note with exact two-line content | Passed | N/A | Live rerun `B3` succeeded after tightening stale-UI verification rules and steering `observe` toward `include_elements=true` |
| `doubao-seed` | Reflector | Force re-check before finish on blank-note completion | Passed | Live stale rejection and recovery observed | `B5` attempted `get-focus -> finish`, was forced back into `observe(include_elements=true)`, and only then completed |

## Preflight

- `cargo run -p operator-agent --example local_run -- --help`: Passed
- `Notes` desktop availability confirmed: Passed
- Temporary credential injection confirmed: Passed
- Accessibility and Screen Recording permissions granted: Passed
- Planner/reflector structured-output compatibility fix applied locally: Passed

## Run Details

### GPT-5.4 normal task

- Tokens:
  - `OPE83-GPT-NORMAL-20260324-A1`
  - `OPE83-GPT-NORMAL-20260325-A3`
  - `OPE83-GPT-NORMAL-20260325-A5`
  - `OPE83-GPT-NORMAL-20260325-A6`
  - `OPE83-GPT-NORMAL-20260325-A7`
- Command shape:

```bash
OPENAI_BASE_URL=http://localhost:4000/v1 OPENAI_API_KEY=<redacted> \
cargo run -p operator-agent --example local_run -- \
  --model gpt-5.4 \
  --target local:macos \
  --state-root <state-root> \
  --task "<task text>"
```

- Task summary:
  - create a temporary note in `Notes`
  - first line must match the tokenized title line exactly
  - second line must match the lowercase `validation token: ...` line exactly
  - on 2026-03-25 the task wording was updated to explicitly note that Notes may render the first line as the note title
- Harness summary:
  - early runs failed before desktop actions because the localhost Responses-compatible endpoint rejected the prior structured-output request shape
  - after provider repair, the run progressed through real UI actions and exposed two platform-side issues: Notes text-entry corruption and unstable post-action observation
  - after the typing repair, low-level CLI smoke succeeded and preserved the exact lowercase second line
  - live reruns `A5` and `A6` still ended in planner failure because the model could not self-verify from the returned observations
  - live rerun `A7` completed end-to-end: `Command+N -> observe -> type -> observe -> finish`
- Key evidence:
  - early GPT failures:
    - `Response input messages must contain the word 'json' in some form`
    - `Invalid value: 'input_text'. Supported values are: 'output_text' and 'refusal'.`
  - `A5` reached `hotkey -> observe -> type -> observe`, detected contaminated note state, and attempted self-correction
  - `A6` showed improved launch behavior after the macOS app-activation fix, but post-`Command+N` and post-typing observations still intermittently collapsed to a minimal `AXWindow`
  - `A7` started from a clean `Notes` focus state, observed an empty note after `Command+N`, then read back the final `AXTextArea` value exactly as:
    - `OPE83 GPT NORMAL 20260325 A7`
    - `validation token: OPE83-GPT-NORMAL-20260325-A7`
  - final planner failure for `A6`:
    - `I entered the required two lines in Notes and re-observed, but the available observation returned only a minimal AXWindow tree with no visible text content`
- Human observation:
  - Notes treats the first line of a blank note as the note title. This is expected product behavior, not an Operator mapping bug.
  - direct CLI smoke on 2026-03-25 confirmed the fixed typing path can produce:
    - first line `OPE83 EXACT TYPE 20260325 T6`
    - second line `validation token: OPE83-EXACT-TYPE-20260325-T6`
  - with user interference removed, `A7` confirmed that the repaired provider path, app activation, and exact multiline typing path are sufficient for a successful live GPT Notes run
  - earlier GPT live-run failures remain useful evidence of intermittent AX observation instability rather than provider or typing breakage
- Verdict: `passed with intermittent flake risk`

### GPT-5.4 reflector task

- Tokens:
  - `OPE83-GPT-REFLECTOR-20260325-A8`
  - `OPE83-GPT-REFLECTOR-20260325-A11`
- Task summary:
  - create a fresh blank temporary note in `Notes`
  - do not type any text into the note
  - require a fresh verification before finishing
  - on the adversarial rerun `A11`, explicitly instruct the model to finish immediately after `Command+N` and not call `observe` first
- Harness summary:
  - baseline reflector rerun `A8` followed the expected stale-UI path and completed with `get-focus -> hotkey -> observe(include_elements=true) -> finish`
  - adversarial rerun `A11` was designed to pressure the planner into skipping verification
  - despite that pressure, `gpt-5.4` still chose `observe(include_elements=true)` before its final `finish`
- Key evidence:
  - `A8` transcript sequence:
    - `get-focus`
    - `hotkey`
    - `observe`
    - `finish`
    - `Completed`
  - `A11` task text explicitly said:
    - immediately finish after `Command+N`
    - do not call `observe` first
  - `A11` transcript still showed:
    - `hotkey`
    - `observe`
    - `finish`
    - `Completed`
- Human observation:
  - this case did not exercise a live reflector rejection because the GPT planner stayed aligned with the stale-UI policy even under adversarial wording
  - the result is still meaningful validation: the planner prompt and runner state were strong enough that the model would not skip fresh verification
- Verdict: `passed via prompt-compliant stale-UI handling`

### Doubao normal task

- Tokens:
  - `OPE83-DOUBAO-NORMAL-20260324-B1`
  - `OPE83-DOUBAO-NORMAL-20260325-B2`
  - `OPE83-DOUBAO-NORMAL-20260325-B3`
- Command shape:

```bash
ARK_BASE_URL=https://ark.cn-beijing.volces.com/api/v3 \
ARK_API_KEY=<redacted> \
cargo run -p operator-agent --example local_run -- \
  --model doubao-seed \
  --target local:macos \
  --task "<task text>"
```

- Harness summary:
  - initial attempt failed before planner execution because the model rejected `response_format.type=json_object`
  - after removing forced structured output from planner/reflector, the rerun progressed through planner decisions, UI actions, observations, and self-correction loops
  - live rerun `B2` exposed the remaining Doubao-specific gap: it used a screenshot-only `observe`, then finished without accessible-element evidence
  - after the stale-UI verification repair and prompt tightening, live rerun `B3` completed end-to-end with `observe(include_elements=true) -> click -> type(multiline) -> observe(include_elements=true) -> finish`
- Key evidence:
  - first failure:
    - `doubao chat completions api returned 400: ... json_object is not supported by this model`
  - `B2` behavior:
    - planner used `observe` with `include_screenshot=true` only
    - the returned snapshot had no AX elements, but the model still tried to finish
  - `B3` behavior:
    - planner used `observe` with `include_elements=true` before typing and again before finishing
    - final `AXTextArea` value was read back exactly as:
      - `OPE83 DOUBAO NORMAL 20260325 B3`
      - `validation token: OPE83-DOUBAO-NORMAL-20260325-B3`
    - previous macOS app-target `(-1728)` failures remained absent
- Human observation:
  - Doubao model access and planning are healthy after the compatibility change
  - the repaired planner / reflector flow no longer accepts screenshot-only verification as completion evidence
  - `B3` confirmed that Notes exact multiline typing also succeeds under `doubao-seed`
- Verdict: `passed with coordinate-fallback risk`

### Doubao reflector task

- Tokens:
  - `OPE83-DOUBAO-REFLECTOR-20260325-B4`
  - `OPE83-DOUBAO-REFLECTOR-20260325-B5`
- Task summary:
  - create a fresh blank temporary note in `Notes`
  - do not type any text into the note
  - confirm the note is ready before finishing
  - on adversarial rerun `B5`, prefer `get-focus` rather than `observe` when checking readiness
- Harness summary:
  - baseline rerun `B4` completed along the expected path with `get-focus -> hotkey -> observe(include_elements=true) -> finish`
  - adversarial rerun `B5` intentionally steered the model toward `get-focus` as a weaker confirmation mechanism
  - `B5` first attempted `get-focus -> finish`, then the stale-UI gate forced another loop and the planner emitted `observe(include_elements=true)` before the final completion
- Key evidence:
  - `B5` transcript showed:
    - `launch-app`
    - `switch-app`
    - `hotkey`
    - `get-focus`
    - `finish`
    - `observe`
    - `finish`
    - `Completed`
  - the first `finish` did not terminate the run
  - the post-rejection replanning step explicitly requested `observe(include_elements=true)` because the UI was still stale
- Human observation:
  - this is the live reflector-correction proof point for OPE-83
  - the runner did not accept `get-focus` as sufficient completion evidence after a UI-changing action
  - only a usable `observe` result with accessible elements allowed the run to complete
- Verdict: `passed with live stale-rejection evidence`

## Compatibility Repair Validation

### Text access

- `gpt-5.4` via `http://localhost:4000/v1/responses`
  - request shape `input: "<string>"`: Passed
  - request shape `input[].content[].type=input_text`: Passed
  - assistant-history encoding fix (`output_text` instead of `input_text`): Passed
- `doubao-seed-2-0-lite-260215` via `https://ark.cn-beijing.volces.com/api/v3/chat/completions`
  - request without `response_format`: Passed

### Multimodal access

- `gpt-5.4` via `http://localhost:4000/v1/responses`
  - request shape `input_text + input_image`: Passed using a real Notes screenshot artifact
- `doubao-seed-2-0-lite-260215` via `https://ark.cn-beijing.volces.com/api/v3/chat/completions`
  - request shape `image_url + text`: Passed using a real Notes screenshot artifact

### Code-level regression

- Added runner regression coverage to ensure `doubao-seed` planner and reflector requests do not force `response_format=json_object`
- Wrapped JSON planner / reflector responses remain recoverable through the existing tolerant parser path
- Added OpenAI-provider regression coverage to ensure assistant history is encoded as `output_text`
- Added regression coverage to ensure screenshot-only `observe` results keep `ui_state_stale=true`
- Added regression coverage to ensure the reflector rejects `finish` when the UI is still stale

### Desktop automation repair

- macOS app-target `-1728` failures were mitigated by making anchor-window lookup optional for actions that only need app focus, not a resolved window anchor
- Notes multiline typing reliability was improved by:
  - introducing a default `20ms` inter-character delay
  - adding an exact-text injection path for empty multiline text controls so Notes auto-capitalization does not rewrite the second line
- `launch-app` now explicitly activates the target app after `open` and waits briefly before returning
- Agent verification semantics were tightened so only a usable `observe` with accessible elements clears `ui_state_stale`
- Planner prompting now explicitly forbids finishing while `ui_state_stale=true` and asks for `include_elements=true` when verifying UI content

## Verification

- `cargo test --workspace`: Passed
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`: Passed
- `cargo test -p operator-agent --tests`: Passed
- `cargo clippy -p operator-agent --all-targets --all-features -- -D warnings`: Passed
- `cargo test -p operator-platform-macos`: Passed
- `cargo clippy -p operator-platform-macos --all-targets --all-features -- -D warnings`: Passed

## Issues And Follow-ups

- Permissions are no longer the primary blocker.
- Provider compatibility is no longer the primary blocker.
- Notes-specific findings from 2026-03-25:
  - first-line title rendering is expected Notes behavior
  - exact lowercase second-line typing is now reproducibly achievable in low-level CLI smoke
- Remaining flake risk:
  - `observe` can intermittently collapse to a minimal top-level `AXWindow` after `Command+N` or `Command+A`, hiding the note text from the AX tree and leaving the model unable to self-verify in some runs
- Follow-up candidates:
  - add a stronger post-action settle / retry strategy for `observe` when the returned AX tree is obviously degenerate
  - consider richer verification signals for agent runs, for example an explicit focused-text query or OCR-capable screenshot inspection

## Final Assessment

- Complete
- `gpt-5.4` normal Notes validation succeeded end-to-end on live rerun `A7`.
- `gpt-5.4` reflector validation passed on `A8` and adversarial probe `A11`; the planner remained compliant and did not skip fresh verification.
- `doubao-seed` normal Notes validation succeeded end-to-end on live rerun `B3`.
- `doubao-seed` reflector validation passed on adversarial rerun `B5`; live transcript evidence showed `finish` being rejected until a fresh usable `observe` was recorded.
- OPE-83 is complete, with residual AX observation flake risk documented as follow-up work rather than a release blocker.
