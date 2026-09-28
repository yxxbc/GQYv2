#!/usr/bin/env bash
# CI 与本地共用的薄封装：rustdoc 文档门禁（19 §8.2；P00-06）。
# 调用 `cargo xtask check --docs`。
# 创建：AI 助手（Cline 会话），2026-09-29 00:22:26。
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
cd "$repo_root"

if [[ $# -ne 0 ]]; then
  printf '用法: %s（不接受参数）\n' "$0" >&2
  exit 2
fi

if [[ ! -f Cargo.toml ]]; then
  printf '错误: %s 下没有 Cargo.toml，请把脚本留在仓库的 scripts/ 目录内。\n' "$repo_root" >&2
  exit 2
fi

cargo xtask check --docs
