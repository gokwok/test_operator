#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT_PATH="$ROOT_DIR/scripts/start-symphony-cli-redesign.sh"
FAKE_BIN="/bin/echo"

if [ ! -x "$SCRIPT_PATH" ]; then
  echo "missing launcher: $SCRIPT_PATH" >&2
  exit 1
fi

OUTPUT="$(
  LINEAR_API_KEY="dummy-linear-key" \
  OPERATOR_SOURCE_REPO="$ROOT_DIR" \
  OPERATOR_DEV_BRANCH="codex/cli-redesign" \
  SYMPHONY_BIN="$FAKE_BIN" \
  SYMPHONY_PORT="4041" \
  "$SCRIPT_PATH" --dry-run
)"

printf '%s\n' "$OUTPUT" | grep -F "source_repo=$ROOT_DIR" >/dev/null
printf '%s\n' "$OUTPUT" | grep -F "dev_branch=codex/cli-redesign" >/dev/null
printf '%s\n' "$OUTPUT" | grep -F "workflow=$ROOT_DIR/WORKFLOW.md" >/dev/null
printf '%s\n' "$OUTPUT" | grep -F "port=4041" >/dev/null
printf '%s\n' "$OUTPUT" | grep -F "binary=$FAKE_BIN" >/dev/null
printf '%s\n' "$OUTPUT" | grep -F "LINEAR_API_KEY=***" >/dev/null
printf '%s\n' "$OUTPUT" | grep -F -- "--port 4041" >/dev/null
printf '%s\n' "$OUTPUT" | grep -F -- "$ROOT_DIR/WORKFLOW.md" >/dev/null
