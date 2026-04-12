#!/usr/bin/env bash
set -euo pipefail

export HOME="/work/assets/tapes/.demo-home"
export USERPROFILE="$HOME"
export LANG="C.UTF-8"
export LC_ALL="C.UTF-8"
cd /work

piz() {
  /work/target-vhs/debug/piz "$@"
}

source /work/assets/tapes/demo-commands.sh
