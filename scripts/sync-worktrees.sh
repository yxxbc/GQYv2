#!/usr/bin/env bash
# GitHub Copilot; updated 2026-09-27T22:45:42Z
set -euo pipefail

usage() {
  printf '用法: %s [--apply] [--repo <仓库路径>]\n' "$0"
  printf '默认只 fetch 并报告；--apply 才会快进 main 或 rebase 干净的本地分支。\n'
}

apply=false
repo_override=''
while [[ $# -gt 0 ]]; do
  case "$1" in
    --apply)
      apply=true
      ;;
    --repo)
      if [[ $# -lt 2 ]]; then
        usage >&2
        exit 2
      fi
      repo_override=$2
      shift
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

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
if [[ -n "$repo_override" ]]; then
  if ! repo_root=$(git -C "$repo_override" rev-parse --show-toplevel 2>/dev/null); then
    printf '错误: 不是 Git 工作树: %s\n' "$repo_override" >&2
    exit 2
  fi
else
  if ! repo_root=$(git -C "$script_dir/.." rev-parse --show-toplevel 2>/dev/null); then
    printf '错误: 无法从脚本位置找到 Git 仓库。\n' >&2
    exit 2
  fi
fi

remote=origin
base_ref=origin/main
base_full=refs/remotes/origin/main
base_branch=main

if ! git -C "$repo_root" remote get-url "$remote" >/dev/null 2>&1; then
  printf '错误: 仓库没有名为 %s 的 remote。\n' "$remote" >&2
  exit 2
fi
if ! git -C "$repo_root" fetch "$remote"; then
  printf '错误: 无法 fetch %s；未修改任何工作树。\n' "$remote" >&2
  exit 2
fi
if ! git -C "$repo_root" show-ref --verify --quiet "$base_full"; then
  printf '错误: 找不到同步基线 %s。\n' "$base_ref" >&2
  exit 2
fi

printf '基线: %s\n模式: %s\n' "$base_ref" "$([[ "$apply" == true ]] && printf apply || printf report)"

inspect_worktree() {
  local worktree_path=$1 branch_ref=$2 branch_name counts ahead behind status_output upstream_ref branch_remote_ref result

  if [[ -z "$branch_ref" ]]; then
    printf 'worktree=%s branch=DETACHED ahead=- behind=- result=skip-detached\n' "$worktree_path"
    return 0
  fi

  branch_name=${branch_ref#refs/heads/}
  if ! counts=$(git -C "$repo_root" rev-list --left-right --count "$branch_ref...$base_full"); then
    printf '错误: 无法比较工作树 %s 与 %s。\n' "$worktree_path" "$base_ref" >&2
    return 1
  fi
  read -r ahead behind <<< "$counts"
  if ! status_output=$(git -C "$worktree_path" status --porcelain --untracked-files=all); then
    printf '错误: 无法读取工作树状态: %s\n' "$worktree_path" >&2
    return 1
  fi
  if [[ -n "$status_output" ]]; then
    printf 'worktree=%s branch=%s ahead=%s behind=%s result=skip-dirty\n' \
      "$worktree_path" "$branch_name" "$ahead" "$behind"
    return 0
  fi

  if [[ "$ahead" == 0 && "$behind" == 0 ]]; then
    printf 'worktree=%s branch=%s ahead=0 behind=0 result=up-to-date\n' \
      "$worktree_path" "$branch_name"
    return 0
  fi

  if [[ "$branch_name" == "$base_branch" ]]; then
    if [[ "$ahead" != 0 ]]; then
      result=skip-main-diverged
    elif [[ "$apply" == true ]]; then
      if ! git -C "$worktree_path" merge --ff-only "$base_ref"; then
        printf '错误: main 快进失败，停止同步。\n' >&2
        return 1
      fi
      result=fast-forwarded
    else
      result=would-fast-forward
    fi
    printf 'worktree=%s branch=%s ahead=%s behind=%s result=%s\n' \
      "$worktree_path" "$branch_name" "$ahead" "$behind" "$result"
    return 0
  fi

  if [[ "$behind" == 0 ]]; then
    printf 'worktree=%s branch=%s ahead=%s behind=0 result=up-to-date\n' \
      "$worktree_path" "$branch_name" "$ahead"
    return 0
  fi

  branch_remote_ref="refs/remotes/origin/$branch_name"
  if [[ "$branch_remote_ref" != "$base_full" ]] && git -C "$repo_root" show-ref --verify --quiet "$branch_remote_ref"; then
    printf 'worktree=%s branch=%s ahead=%s behind=%s upstream=%s result=skip-upstream\n' \
      "$worktree_path" "$branch_name" "$ahead" "$behind" "$branch_remote_ref"
    return 0
  fi

  upstream_ref=$(git -C "$repo_root" for-each-ref --format='%(upstream)' "$branch_ref")
  if [[ -n "$upstream_ref" && "$upstream_ref" != "$base_full" ]]; then
    if git -C "$repo_root" show-ref --verify --quiet "$upstream_ref"; then
      result=skip-upstream
    else
      result=skip-upstream-gone
    fi
    printf 'worktree=%s branch=%s ahead=%s behind=%s upstream=%s result=%s\n' \
      "$worktree_path" "$branch_name" "$ahead" "$behind" "$upstream_ref" "$result"
    return 0
  fi

  if [[ "$ahead" == 0 ]]; then
    if [[ "$apply" == true ]]; then
      if ! git -C "$worktree_path" merge --ff-only "$base_ref"; then
        printf '错误: %s 快进失败，停止同步。\n' "$branch_name" >&2
        return 1
      fi
      result=fast-forwarded
    else
      result=would-fast-forward
    fi
  elif [[ "$apply" == true ]]; then
    if ! git -C "$worktree_path" rebase "$base_ref"; then
      if git -C "$worktree_path" rev-parse --verify REBASE_HEAD >/dev/null 2>&1; then
        if ! git -C "$worktree_path" rebase --abort; then
          printf '错误: rebase 失败且自动 abort 失败；请手动检查 %s。\n' "$worktree_path" >&2
        fi
      fi
      printf '冲突: 已停止同步；失败分支已恢复到 rebase 前状态（若 abort 成功）。\n' >&2
      return 1
    fi
    result=rebased
  else
    result=would-rebase
  fi

  printf 'worktree=%s branch=%s ahead=%s behind=%s result=%s\n' \
    "$worktree_path" "$branch_name" "$ahead" "$behind" "$result"
}

worktrees=$(git -C "$repo_root" worktree list --porcelain) || {
  printf '错误: 无法列出 Git worktree。\n' >&2
  exit 2
}
worktree_path=''
branch_ref=''
while IFS= read -r line; do
  case "$line" in
    'worktree '*)
      if [[ -n "$worktree_path" ]]; then
        if ! inspect_worktree "$worktree_path" "$branch_ref"; then
          exit 1
        fi
      fi
      worktree_path=${line#worktree }
      branch_ref=''
      ;;
    'branch '*)
      branch_ref=${line#branch }
      ;;
    '')
      if [[ -n "$worktree_path" ]]; then
        if ! inspect_worktree "$worktree_path" "$branch_ref"; then
          exit 1
        fi
      fi
      worktree_path=''
      branch_ref=''
      ;;
  esac
done <<< "$worktrees"
if [[ -n "$worktree_path" ]]; then
  if ! inspect_worktree "$worktree_path" "$branch_ref"; then
    exit 1
  fi
fi