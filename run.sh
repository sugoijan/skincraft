#!/usr/bin/env bash
set -euo pipefail
repo_dir="$(cd "$(dirname "$0")" && pwd)"
export DDI_STUDIO_HOME="${DDI_STUDIO_HOME:-$repo_dir/.local/skincraft}"
export PHOTOCRAFT_CONFIG_DIR="$DDI_STUDIO_HOME/photocraft"
export DDI_HOME="$DDI_STUDIO_HOME/game"
export DDI_STUDIO_OUTPUT="${DDI_STUDIO_OUTPUT:-$DDI_STUDIO_HOME/export}"
mkdir -p "$DDI_STUDIO_HOME"
cd "$repo_dir"
exec cargo run --release --locked --bin skincraft -- "$@"
