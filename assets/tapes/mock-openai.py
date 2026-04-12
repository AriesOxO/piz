#!/usr/bin/env python3
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

COUNTERS = {}


def build_content(system_prompt: str, user_prompt: str) -> str:
    if "raw JSON array" in system_prompt:
        return json.dumps(
            [
                {
                    "command": "find . -type f -size +100M | sort",
                    "danger": "safe",
                    "explanation": "Files larger than 100MB",
                },
                {
                    "command": "du -ah . | sort -rh | head -20",
                    "danger": "safe",
                    "explanation": "Top 20 largest files and directories",
                },
                {
                    "command": "find . -type f -printf '%s %p\\n' | sort -nr | head -20",
                    "danger": "safe",
                    "explanation": "Largest files with exact sizes",
                },
            ]
        )

    if "provide a working fix" in system_prompt or "Analyze a failed command" in system_prompt:
        return json.dumps(
            {
                "diagnosis": "The current Python environment is missing the flask package required by app.py",
                "command": "pip install flask && python app.py",
                "danger": "warning",
            }
        )

    if "explain shell commands clearly and precisely" in system_prompt:
        return """**Command overview**: Lists Rust source files under src.

**Breakdown**:
  `find` — walk the directory tree
  `src` — restrict the search to the src directory
  `-name '*.rs'` — only match Rust source files
  `-type f` — include files only

**What it does step by step**:
1. Traverses the src directory recursively.
2. Filters entries to regular files.
3. Keeps only files ending in .rs.
4. Prints each matching path.
"""

    if user_prompt == "list all rust files in src":
        return json.dumps(
            {
                "command": "find src -type f -name '*.rs'",
                "danger": "safe",
            }
        )

    if user_prompt == "show rust files in src":
        return json.dumps(
            {
                "command": "find src -type f -name '*.rs'",
                "danger": "safe",
                "explanation": "`find` - walk the directory tree\n`src` - limit the search to src\n`-type f` - files only\n`-name '*.rs'` - only Rust source files",
            }
        )

    if user_prompt == "list project files":
        count = COUNTERS.get(user_prompt, 0)
        COUNTERS[user_prompt] = count + 1
        if count == 0:
            return json.dumps(
                {
                    "command": "find . -maxdepth 1 -type f | sort",
                    "danger": "safe",
                    "explanation": "List top-level files only",
                }
            )
        return json.dumps(
            {
                "command": "find . -maxdepth 2 | sort",
                "danger": "safe",
                "explanation": "List files and directories up to depth 2",
            }
        )

    return json.dumps({"command": "pwd", "danger": "safe"})


class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != "/v1/chat/completions":
            self.send_response(404)
            self.end_headers()
            return

        length = int(self.headers.get("Content-Length", "0"))
        payload = json.loads(self.rfile.read(length).decode("utf-8"))
        messages = payload.get("messages", [])
        system_prompt = messages[0].get("content", "") if messages else ""
        user_prompt = ""
        for msg in reversed(messages):
            if msg.get("role") == "user":
                user_prompt = msg.get("content", "")
                break

        content = build_content(system_prompt, user_prompt)
        response = {
            "choices": [
                {
                    "message": {
                        "content": content,
                    }
                }
            ]
        }
        body = json.dumps(response).encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        return


if __name__ == "__main__":
    server = ThreadingHTTPServer(("127.0.0.1", 18080), Handler)
    server.serve_forever()
