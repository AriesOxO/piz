#!/usr/bin/env bash
set -euo pipefail

demo_pipe_list() {
  echo "Request: list all rust files in src"
  printf 'list all rust files in src\n' | piz --pipe
}

demo_detail() {
  echo "Request: show rust files in src"
  piz -d "show rust files in src"
}

demo_multi() {
  echo "Request: find large files with multiple options"
  piz -n 3 "find large files"
}

demo_regenerate() {
  echo "Request: list project files"
  piz "list project files"
}

demo_fix_flow() {
  echo "Request: fix the last failed command"
  piz fix
}
