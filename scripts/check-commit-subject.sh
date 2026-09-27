#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  printf '用法: %s <来源> <提交标题>\n' "$0" >&2
  exit 2
fi

source_name=$1
subject=$2
subject_pattern='^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-z0-9][a-z0-9._/-]*\))?(!)?: .+$'

if [[ ! "$subject" =~ $subject_pattern ]]; then
  printf '无效的%s: %s\n期望格式: <type>(<scope>)!: <description>\n' "$source_name" "$subject" >&2
  exit 1
fi