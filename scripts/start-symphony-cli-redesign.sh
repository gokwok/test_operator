#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ENV_FILE="$ROOT_DIR/.env.local"
WORKFLOW_PATH="$ROOT_DIR/WORKFLOW.md"
SYMPHONY_BIN="${SYMPHONY_BIN:-/tmp/symphony.zLrD39/elixir/bin/symphony}"
SYMPHONY_PORT="${SYMPHONY_PORT:-4041}"
OPERATOR_SOURCE_REPO="${OPERATOR_SOURCE_REPO:-$ROOT_DIR}"
OPERATOR_DEV_BRANCH="${OPERATOR_DEV_BRANCH:-codex/cli-redesign}"
DRY_RUN=0

if [ "${1:-}" = "--dry-run" ]; then
  DRY_RUN=1
  shift
fi

if [ $# -ne 0 ]; then
  echo "usage: $0 [--dry-run]" >&2
  exit 1
fi

if [ -f "$ENV_FILE" ]; then
  set -a
  # shellcheck disable=SC1090
  . "$ENV_FILE"
  set +a
fi

: "${LINEAR_API_KEY:?LINEAR_API_KEY is required; set it in .env.local or the environment}"

if [ ! -x "$SYMPHONY_BIN" ]; then
  echo "symphony binary not found or not executable: $SYMPHONY_BIN" >&2
  exit 1
fi

if [ ! -f "$WORKFLOW_PATH" ]; then
  echo "workflow file not found: $WORKFLOW_PATH" >&2
  exit 1
fi

if ! git -C "$OPERATOR_SOURCE_REPO" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "not a git checkout: $OPERATOR_SOURCE_REPO" >&2
  exit 1
fi

if git -C "$OPERATOR_SOURCE_REPO" show-ref --verify --quiet "refs/heads/$OPERATOR_DEV_BRANCH"; then
  git -C "$OPERATOR_SOURCE_REPO" switch "$OPERATOR_DEV_BRANCH" >/dev/null 2>&1
else
  git -C "$OPERATOR_SOURCE_REPO" switch -c "$OPERATOR_DEV_BRANCH" >/dev/null 2>&1
fi

CMD=(
  "$SYMPHONY_BIN"
  --port "$SYMPHONY_PORT"
  --i-understand-that-this-will-be-running-without-the-usual-guardrails
  "$WORKFLOW_PATH"
)

if [ "$DRY_RUN" -eq 1 ]; then
  printf 'LINEAR_API_KEY=***\n'
  printf 'source_repo=%s\n' "$OPERATOR_SOURCE_REPO"
  printf 'dev_branch=%s\n' "$OPERATOR_DEV_BRANCH"
  printf 'workflow=%s\n' "$WORKFLOW_PATH"
  printf 'port=%s\n' "$SYMPHONY_PORT"
  printf 'binary=%s\n' "$SYMPHONY_BIN"
  printf 'command='
  printf '%q ' "${CMD[@]}"
  printf '\n'
  exit 0
fi

export LINEAR_API_KEY
export OPERATOR_SOURCE_REPO
export OPERATOR_DEV_BRANCH

cd "$ROOT_DIR"
exec "${CMD[@]}"
