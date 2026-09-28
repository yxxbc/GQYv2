#!/usr/bin/env bash
# CI 与本地共用的薄封装：跑测试并产出报告（19 §8.2、§8.3；P00-06）。
#   bash scripts/ci-test.sh          全量：`cargo xtask test`（单元 + 集成 + doc，含计数门禁）
#   bash scripts/ci-test.sh --unit   Windows 作业：`cargo build --workspace` + `cargo xtask test --unit`
# 创建：AI 助手（Cline 会话），2026-09-29 00:22:26。
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/.." && pwd)
cd "$repo_root"

usage() {
  printf '用法: %s [--unit]\n' "$0"
}

unit=false
while [[ $# -gt 0 ]]; do
  case "$1" in
    --unit)
      unit=true
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      exit 2
      ;;
  esac
  shift
done

if [[ ! -f Cargo.toml ]]; then
  printf '错误: %s 下没有 Cargo.toml，请把脚本留在仓库的 scripts/ 目录内。\n' "$repo_root" >&2
  exit 2
fi

if [[ "$unit" == true ]]; then
  # Windows 作业（19 §8.2）：先整包构建确认能编译，再只跑单元测试（--lib --bins）。
  cargo build --workspace
  cargo xtask test --unit
else
  cargo xtask test
fi
