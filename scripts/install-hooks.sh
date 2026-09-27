#!/usr/bin/env bash
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(git -C "$script_dir/.." rev-parse --show-toplevel 2>/dev/null) || {
  printf '当前目录不在 Git 仓库中，无法安装 hooks。\n' >&2
  exit 1
}

git -C "$repo_root" config --local core.hooksPath .githooks
printf '已启用本地 Git hooks: %s/.githooks\n' "$repo_root"