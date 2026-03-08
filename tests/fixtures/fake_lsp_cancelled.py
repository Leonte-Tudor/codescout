#!/usr/bin/env python3
"""Fake LSP server that returns -32800 (RequestCancelled) for the first N requests,
then succeeds. Used to test retry logic in LspClient::request().

Protocol: JSON-RPC 2.0 over stdio with Content-Length headers (LSP transport).

Usage: python3 fake_lsp_cancelled.py [cancel_count]
  cancel_count: how many requests to cancel before succeeding (default: 2)
"""

import json
import sys
import struct


def read_message():
    """Read a JSON-RPC message with Content-Length header from stdin."""
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        line = line.decode("utf-8").strip()
        if line == "":
            break
        if ":" in line:
            key, value = line.split(":", 1)
            headers[key.strip()] = value.strip()

    content_length = int(headers.get("Content-Length", 0))
    if content_length == 0:
        return None

    body = sys.stdin.buffer.read(content_length)
    return json.loads(body.decode("utf-8"))


def write_message(msg):
    """Write a JSON-RPC message with Content-Length header to stdout."""
    body = json.dumps(msg).encode("utf-8")
    header = f"Content-Length: {len(body)}\r\n\r\n".encode("utf-8")
    sys.stdout.buffer.write(header + body)
    sys.stdout.buffer.flush()


def main():
    cancel_count = int(sys.argv[1]) if len(sys.argv) > 1 else 2
    cancelled_so_far = 0

    while True:
        msg = read_message()
        if msg is None:
            break

        method = msg.get("method")
        msg_id = msg.get("id")

        # Notifications (no id) — ignore
        if msg_id is None:
            continue

        if method == "initialize":
            write_message({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "capabilities": {
                        "documentSymbolProvider": True,
                    },
                },
            })
        elif method == "shutdown":
            write_message({
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": None,
            })
        else:
            # For any other request: cancel the first N, then succeed
            if cancelled_so_far < cancel_count:
                cancelled_so_far += 1
                write_message({
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "error": {
                        "code": -32800,
                        "message": "cancelled",
                    },
                })
            else:
                # Return a minimal valid documentSymbol response
                write_message({
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "result": [
                        {
                            "name": "fakeSymbol",
                            "kind": 12,
                            "range": {
                                "start": {"line": 0, "character": 0},
                                "end": {"line": 5, "character": 1},
                            },
                            "selectionRange": {
                                "start": {"line": 0, "character": 4},
                                "end": {"line": 0, "character": 14},
                            },
                        }
                    ],
                })


if __name__ == "__main__":
    main()
