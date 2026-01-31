#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARKTS_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
OP_ROOT="$(cd "${ARKTS_ROOT}/.." && pwd)"

AGENT_V3_DIR="${AGENT_V3_DIR:-${OP_ROOT}/agent_v3}"
MCP_PROXY_DIR="${MCP_PROXY_DIR:-${OP_ROOT}/harmony_mcp_proxy}"

RUN_DIR="${ARKTS_ROOT}/.phase4_env"
LOG_DIR="${RUN_DIR}/logs"
PID_MCP="${RUN_DIR}/mcp_proxy.pid"
PID_A2A="${RUN_DIR}/a2a_runtime.pid"

PYTHON_BIN="${PYTHON_BIN:-python}"

HOST="${HOST:-0.0.0.0}"
A2A_PORT="${A2A_PORT:-8080}"
MCP_WS_PORT="${MCP_WS_PORT:-7001}"
MCP_PORT="${MCP_PORT:-7002}"
LPU_UPSTREAM="${LPU_UPSTREAM:-http://localhost:4000/v1}"
MODEL="${MODEL:-doubao-seed-1-8}"

usage() {
  cat <<EOF
Usage: $(basename "$0") {up|down|status}

Env overrides:
  PYTHON_BIN=python3
  HOST=0.0.0.0
  A2A_PORT=8080
  MCP_WS_PORT=7001
  MCP_PORT=7002
  LPU_UPSTREAM=http://localhost:4000/v1
  MODEL=doubao-seed-1-8
  AGENT_V3_DIR=/path/to/agent_v3
  MCP_PROXY_DIR=/path/to/harmony_mcp_proxy
EOF
}

is_running() {
  local pid_file="$1"
  [[ -f "$pid_file" ]] || return 1
  local pid
  pid="$(cat "$pid_file")"
  [[ -n "$pid" ]] || return 1
  kill -0 "$pid" >/dev/null 2>&1
}

find_pids_by_port() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -ti tcp:"$port" -sTCP:LISTEN 2>/dev/null || true
    return
  fi
  if command -v netstat >/dev/null 2>&1; then
    netstat -anv 2>/dev/null | awk -v p=":$port" '$0 ~ p && $6 == "LISTEN" {print $9}' | tr -d '/' | xargs -r ps -p 2>/dev/null | awk 'NR>1 {print $1}' || true
    return
  fi
}

kill_pids() {
  local pids=("$@")
  if [[ ${#pids[@]} -eq 0 ]]; then
    return
  fi
  for pid in "${pids[@]}"; do
    if [[ -n "$pid" ]]; then
      kill "$pid" >/dev/null 2>&1 || true
    fi
  done
  sleep 0.3
  for pid in "${pids[@]}"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" >/dev/null 2>&1; then
      kill -9 "$pid" >/dev/null 2>&1 || true
    fi
  done
}

stop_by_port() {
  local name="$1"
  local port="$2"
  local pids
  pids="$(find_pids_by_port "$port")"
  if [[ -z "$pids" ]]; then
    return
  fi
  echo "[$name] stopping by port $port (pids=$pids)"
  # shellcheck disable=SC2206
  kill_pids ${pids}
}

start_cmd() {
  local name="$1"
  local logfile="$2"
  local pidfile="$3"
  local cwd="$4"
  shift 4
  if is_running "$pidfile"; then
    echo "[$name] already running (pid=$(cat "$pidfile"))"
    return 0
  fi
  mkdir -p "$LOG_DIR"
  (cd "$cwd" && nohup "$@" >"$logfile" 2>&1 & echo $! > "$pidfile")
  echo "[$name] started (pid=$(cat "$pidfile"), log=$logfile)"
}

stop_cmd() {
  local name="$1"
  local pidfile="$2"
  if ! is_running "$pidfile"; then
    echo "[$name] not running"
    rm -f "$pidfile"
    return 0
  fi
  local pid
  pid="$(cat "$pidfile")"
  kill "$pid" >/dev/null 2>&1 || true
  sleep 0.3
  if kill -0 "$pid" >/dev/null 2>&1; then
    kill -9 "$pid" >/dev/null 2>&1 || true
  fi
  rm -f "$pidfile"
  echo "[$name] stopped"
}

status_cmd() {
  if is_running "$PID_MCP"; then
    echo "[mcp_proxy] running pid=$(cat "$PID_MCP")"
  else
    echo "[mcp_proxy] stopped"
  fi
  if is_running "$PID_A2A"; then
    echo "[a2a_runtime] running pid=$(cat "$PID_A2A")"
  else
    echo "[a2a_runtime] stopped"
  fi
  local mcp_ws_pid
  mcp_ws_pid="$(find_pids_by_port "$MCP_WS_PORT")"
  if [[ -n "$mcp_ws_pid" ]]; then
    echo "[mcp_proxy] port $MCP_WS_PORT listening pid(s)=$mcp_ws_pid"
  fi
  local mcp_pid
  mcp_pid="$(find_pids_by_port "$MCP_PORT")"
  if [[ -n "$mcp_pid" ]]; then
    echo "[mcp_proxy] port $MCP_PORT listening pid(s)=$mcp_pid"
  fi
  local a2a_pid
  a2a_pid="$(find_pids_by_port "$A2A_PORT")"
  if [[ -n "$a2a_pid" ]]; then
    echo "[a2a_runtime] port $A2A_PORT listening pid(s)=$a2a_pid"
  fi
  echo "Logs: $LOG_DIR"
}

up() {
  start_cmd "mcp_proxy" \
    "${LOG_DIR}/mcp_proxy.log" \
    "$PID_MCP" \
    "$OP_ROOT" \
    env PYTHONPATH="$OP_ROOT" \
    "$PYTHON_BIN" -m harmony_mcp_proxy --host "$HOST" --ws-port "$MCP_WS_PORT" --mcp-port "$MCP_PORT"

  start_cmd "a2a_runtime" \
    "${LOG_DIR}/a2a_runtime.log" \
    "$PID_A2A" \
    "$OP_ROOT" \
    env GUI_AGENT_MODEL="$MODEL" PYTHONPATH="$OP_ROOT/thinkflow/src:$OP_ROOT" \
    "$PYTHON_BIN" -m thinkflow.runtime \
      --package agent_v3 \
      --host "$HOST" --port "$A2A_PORT" \
      --tools-upstream-url "http://127.0.0.1:${MCP_PORT}/mcp" \
      --lpu-upstream-url "$LPU_UPSTREAM"

  if command -v hdc >/dev/null 2>&1; then
    hdc rport "tcp:${A2A_PORT}" "tcp:${A2A_PORT}" || true
    hdc rport "tcp:${MCP_WS_PORT}" "tcp:${MCP_WS_PORT}" || true
  else
    echo "[hdc] not found, skip rport"
  fi

  echo "Ready:"
  echo "  A2A: http://${HOST}:${A2A_PORT}"
  echo "  MCP WS: ws://${HOST}:${MCP_WS_PORT}/ws"
  echo "  MCP HTTP: http://${HOST}:${MCP_PORT}/mcp"
}

down() {
  stop_cmd "a2a_runtime" "$PID_A2A"
  stop_cmd "mcp_proxy" "$PID_MCP"
  stop_by_port "a2a_runtime" "$A2A_PORT"
  stop_by_port "mcp_proxy" "$MCP_WS_PORT"
  stop_by_port "mcp_proxy" "$MCP_PORT"
}

case "${1:-}" in
  up|"")
    up
    ;;
  down)
    down
    ;;
  status)
    status_cmd
    ;;
  *)
    usage
    exit 1
    ;;
esac
