#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
import time
from dataclasses import dataclass, field
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Dict, List, Optional


JsonObject = Dict[str, Any]


def _now_ms() -> int:
    return int(time.time() * 1000)


def _read_json(req: BaseHTTPRequestHandler) -> JsonObject:
    length = int(req.headers.get("content-length") or 0)
    raw = req.rfile.read(length) if length > 0 else b"{}"
    try:
        parsed = json.loads(raw.decode("utf-8") or "{}")
    except Exception:
        return {}
    return parsed if isinstance(parsed, dict) else {}


def _json_response(req: BaseHTTPRequestHandler, status: int, payload: JsonObject) -> None:
    body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
    req.send_response(status)
    req.send_header("content-type", "application/json; charset=utf-8")
    req.send_header("content-length", str(len(body)))
    req.end_headers()
    req.wfile.write(body)


def _extract_text(parts: Any) -> str:
    if not isinstance(parts, list):
        return ""
    for part in parts:
        if not isinstance(part, dict):
            continue
        if part.get("kind") == "text":
            text = part.get("text")
            if isinstance(text, str):
                return text
    return ""


def _is_truthy_reply(text: str) -> bool:
    lower = text.strip().lower()
    return lower in {"yes", "y", "ok", "okay", "同意", "确认", "好"}


def _build_task(
    task_id: str,
    context_id: str,
    state: str,
    message_text: str,
    action: Optional[JsonObject] = None,
) -> JsonObject:
    message: JsonObject = {"parts": [{"text": message_text}]}
    if action is not None:
        message["metadata"] = {"action": action}
    return {
        "id": task_id,
        "contextId": context_id,
        "status": {
            "state": state,
            "message": message,
        },
    }


@dataclass
class TaskRecord:
    task_id: str
    context_id: str
    awaiting_input: bool = False
    created_ms: int = field(default_factory=_now_ms)


class TaskStore:
    def __init__(self) -> None:
        self.tasks: Dict[str, TaskRecord] = {}

    def create(self, context_id: str) -> TaskRecord:
        task_id = f"task_{_now_ms()}"
        record = TaskRecord(task_id=task_id, context_id=context_id)
        self.tasks[task_id] = record
        return record

    def get(self, task_id: str) -> Optional[TaskRecord]:
        return self.tasks.get(task_id)


class A2AMockHandler(BaseHTTPRequestHandler):
    server_version = "a2a-mock/0.1"

    def log_message(self, fmt: str, *args: Any) -> None:
        sys.stderr.write("[a2a_mock] " + (fmt % args) + "\n")

    def do_GET(self) -> None:  # noqa: N802
        if self.path.rstrip("/") == "/health":
            return _json_response(self, 200, {"status": "ok"})
        _json_response(self, 404, {"error": "not_found"})

    def do_POST(self) -> None:  # noqa: N802
        path = self.path.rstrip("/")
        if path == "/a2a/v1/message:stream":
            return self._handle_stream()
        if path != "/a2a/v1/message:send":
            return _json_response(self, 404, {"error": "not_found"})

        body = _read_json(self)
        message = body.get("message")
        if not isinstance(message, dict):
            return _json_response(self, 400, {"error": "missing message"})

        context_id = str(message.get("contextId") or "").strip()
        if not context_id:
            return _json_response(self, 400, {"error": "missing contextId"})

        prompt = _extract_text(message.get("parts"))
        task_id = str(message.get("taskId") or "").strip()

        store: TaskStore = getattr(self.server, "task_store")  # type: ignore[attr-defined]
        mode: str = getattr(self.server, "mode")  # type: ignore[attr-defined]
        keywords: List[str] = getattr(self.server, "keywords")  # type: ignore[attr-defined]
        delay_ms: int = getattr(self.server, "delay_ms")  # type: ignore[attr-defined]

        if delay_ms > 0:
            time.sleep(delay_ms / 1000.0)

        if task_id:
            record = store.get(task_id)
            if not record:
                task = _build_task(task_id, context_id, "failed", "unknown taskId")
                return _json_response(self, 200, {"task": task})
            reply_text = prompt
            if _is_truthy_reply(reply_text):
                task = _build_task(task_id, context_id, "completed", "已确认，任务完成")
                record.awaiting_input = False
            else:
                task = _build_task(task_id, context_id, "failed", "用户拒绝或回复无效")
                record.awaiting_input = False
            return _json_response(self, 200, {"task": task})

        record = store.create(context_id)
        should_input = False
        if mode == "input":
            should_input = True
        elif mode == "complete":
            should_input = False
        elif mode == "fail":
            task = _build_task(record.task_id, context_id, "failed", "mock failed")
            return _json_response(self, 200, {"task": task})
        else:
            lower = prompt.lower()
            for key in keywords:
                if key and key in lower:
                    should_input = True
                    break

        if should_input:
            record.awaiting_input = True
            action = {"name": "interact", "options": ["yes", "no"]}
            task = _build_task(record.task_id, context_id, "input-required", "需要用户确认，是否继续？", action)
            return _json_response(self, 200, {"task": task})

        task = _build_task(record.task_id, context_id, "completed", "任务已完成（mock）")
        return _json_response(self, 200, {"task": task})

    def _handle_stream(self) -> None:
        body = _read_json(self)
        message = body.get("message")
        if not isinstance(message, dict):
            return _json_response(self, 400, {"error": "missing message"})

        context_id = str(message.get("contextId") or "").strip()
        if not context_id:
            return _json_response(self, 400, {"error": "missing contextId"})

        prompt = _extract_text(message.get("parts"))
        mode: str = getattr(self.server, "mode")  # type: ignore[attr-defined]
        keywords: List[str] = getattr(self.server, "keywords")  # type: ignore[attr-defined]
        delay_ms: int = getattr(self.server, "delay_ms")  # type: ignore[attr-defined]

        should_input = False
        if mode == "input":
            should_input = True
        elif mode == "complete":
            should_input = False
        elif mode == "fail":
            should_input = False
        else:
            lower = prompt.lower()
            for key in keywords:
                if key and key in lower:
                    should_input = True
                    break

        self.send_response(200)
        self.send_header("content-type", "text/event-stream; charset=utf-8")
        self.send_header("cache-control", "no-cache")
        self.end_headers()

        def send_event(payload: JsonObject) -> None:
            data = json.dumps(payload, ensure_ascii=False)
            self.wfile.write(f"data: {data}\n\n".encode("utf-8"))
            self.wfile.flush()
            if delay_ms > 0:
                time.sleep(delay_ms / 1000.0)

        send_event({"statusUpdate": {"state": "working", "message": {"parts": [{"text": "mock working"}]}}})
        send_event({
            "artifactUpdate": {
                "append": True,
                "artifact": {"artifactId": "thinkflow.delta", "data": {"text": "mock delta"}},
            }
        })
        send_event({
            "artifactUpdate": {
                "append": True,
                "artifact": {"artifactId": "tool.trace", "data": {"tool": "screenshot", "status": "started"}},
            }
        })

        if mode == "fail":
            send_event({"statusUpdate": {"state": "failed", "final": True, "message": {"parts": [{"text": "mock failed"}]}}})
            return

        if should_input:
            send_event({
                "statusUpdate": {
                    "state": "input-required",
                    "final": True,
                    "message": {"parts": [{"text": "需要用户确认（mock）"}]},
                }
            })
            return

        send_event({
            "statusUpdate": {
                "state": "completed",
                "final": True,
                "message": {"parts": [{"text": "mock completed"}]},
            }
        })
        return


def main() -> int:
    parser = argparse.ArgumentParser(description="Mock A2A message:send server (blocking only).")
    parser.add_argument("--host", default="0.0.0.0")
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--mode", default="auto", choices=["auto", "input", "complete", "fail"])
    parser.add_argument(
        "--input-keywords",
        default="input,confirm,确认,同意,允许",
        help="comma-separated keywords that trigger input-required when mode=auto",
    )
    parser.add_argument("--delay-ms", type=int, default=0)
    args = parser.parse_args()

    keywords = [item.strip().lower() for item in args.input_keywords.split(",")]
    server = ThreadingHTTPServer((args.host, int(args.port)), A2AMockHandler)
    server.task_store = TaskStore()  # type: ignore[attr-defined]
    server.mode = args.mode  # type: ignore[attr-defined]
    server.keywords = keywords  # type: ignore[attr-defined]
    server.delay_ms = max(0, int(args.delay_ms))  # type: ignore[attr-defined]

    sys.stderr.write(f"[a2a_mock] listening on http://{args.host}:{args.port}\n")
    sys.stderr.write(f"[a2a_mock] mode={args.mode} keywords={keywords}\n")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
