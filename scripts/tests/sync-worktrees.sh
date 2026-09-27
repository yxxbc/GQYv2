#!/usr/bin/env bash
# GitHub Copilot; updated 2026-09-27T22:48:24Z
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
target=${1:-"$repo_root/scripts/sync-worktrees.sh"}
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fail() {
  printf '失败: %s\n' "$1" >&2
  exit 1
}

commit_all() {
  local worktree=$1 message=$2
  git -C "$worktree" add --all
  git -C "$worktree" commit -q -m "$message"
}

remote="$tmp/origin.git"
repo="$tmp/repo"
git init --quiet --bare --initial-branch=main "$remote"
git clone --quiet "$remote" "$repo"
git -C "$repo" config user.name "Worktree sync test"
git -C "$repo" config user.email "worktree-sync@example.invalid"
printf 'base\n' > "$repo/shared.txt"
commit_all "$repo" "initial commit"
git -C "$repo" push --quiet --set-upstream origin main

git -C "$repo" worktree add --quiet -b clean "$tmp/clean" main
printf 'clean change\n' > "$tmp/clean/clean.txt"
commit_all "$tmp/clean" "clean local change"

git -C "$repo" worktree add --quiet -b dirty "$tmp/dirty" main
printf 'untracked change\n' > "$tmp/dirty/untracked.txt"
git -C "$repo" worktree add --quiet --detach "$tmp/detached" main

git -C "$repo" worktree add --quiet -b conflict "$tmp/conflict" main
printf 'branch version\n' > "$tmp/conflict/shared.txt"
commit_all "$tmp/conflict" "conflicting local change"

git -C "$repo" checkout --quiet -b published-seed main
printf 'published base\n' > "$repo/published.txt"
commit_all "$repo" "published branch base"
git -C "$repo" push --quiet origin HEAD:refs/heads/published
git -C "$repo" checkout --quiet main
git -C "$repo" branch -D published-seed >/dev/null
git -C "$repo" fetch --quiet origin
git -C "$repo" worktree add --quiet --track -b published-local "$tmp/published" origin/published
printf 'published local change\n' > "$tmp/published/local.txt"
commit_all "$tmp/published" "published local change"

git -C "$repo" checkout --quiet -b same-name-seed main
printf 'same name base\n' > "$repo/same-name.txt"
commit_all "$repo" "same-name branch base"
git -C "$repo" push --quiet origin HEAD:refs/heads/same-name
git -C "$repo" checkout --quiet main
git -C "$repo" branch -D same-name-seed >/dev/null
git -C "$repo" fetch --quiet origin
git -C "$repo" branch --no-track same-name origin/same-name
git -C "$repo" worktree add --quiet "$tmp/same-name" same-name
printf 'same name local change\n' > "$tmp/same-name/local.txt"
commit_all "$tmp/same-name" "same-name local change"

git -C "$repo" checkout --quiet -b gone-seed main
printf 'gone base\n' > "$repo/gone.txt"
commit_all "$repo" "gone branch base"
git -C "$repo" push --quiet origin HEAD:refs/heads/gone
git -C "$repo" checkout --quiet main
git -C "$repo" branch -D gone-seed >/dev/null
git -C "$repo" fetch --quiet origin
git -C "$repo" worktree add --quiet --track -b gone "$tmp/gone" origin/gone
printf 'gone local change\n' > "$tmp/gone/local.txt"
commit_all "$tmp/gone" "gone local change"
git -C "$repo" push --quiet origin --delete gone
git -C "$repo" update-ref -d refs/remotes/origin/gone

printf 'main version\n' > "$repo/shared.txt"
commit_all "$repo" "advance main"
git -C "$repo" push --quiet origin main

main_before=$(git -C "$repo" rev-parse HEAD)
clean_before=$(git -C "$tmp/clean" rev-parse HEAD)
if ! bash "$target" --repo "$repo" > "$tmp/report.log" 2>&1; then
  fail "报告模式应成功，输出：$(cat "$tmp/report.log")"
fi
if [[ "$(git -C "$repo" rev-parse HEAD)" != "$main_before" ]]; then
  fail "默认报告模式快进了 main"
fi
if [[ "$(git -C "$tmp/clean" rev-parse HEAD)" != "$clean_before" ]]; then
  fail "默认报告模式改写了 clean 分支"
fi
grep -q 'branch=clean .*result=would-rebase' "$tmp/report.log" || fail "未报告 clean 分支可 rebase"
grep -q 'branch=dirty .*result=skip-dirty' "$tmp/report.log" || fail "未跳过脏工作树"
grep -q 'branch=DETACHED .*result=skip-detached' "$tmp/report.log" || fail "未跳过 detached worktree"
grep -q 'branch=published-local .*result=skip-upstream' "$tmp/report.log" || fail "未跳过配置了远端 upstream 的分支"
grep -q 'branch=same-name .*result=skip-upstream' "$tmp/report.log" || fail "未跳过同名远端分支"
grep -q 'branch=gone .*result=skip-upstream-gone' "$tmp/report.log" || fail "未跳过 upstream 已删除的分支"

conflict_before=$(git -C "$tmp/conflict" rev-parse HEAD)
dirty_before=$(git -C "$tmp/dirty" rev-parse HEAD)
published_before=$(git -C "$tmp/published" rev-parse HEAD)
same_name_before=$(git -C "$tmp/same-name" rev-parse HEAD)
gone_before=$(git -C "$tmp/gone" rev-parse HEAD)
detached_before=$(git -C "$tmp/detached" rev-parse HEAD)
sync_exit=0
bash "$target" --repo "$repo" --apply > "$tmp/apply.log" 2>&1 || sync_exit=$?
if [[ "$sync_exit" -eq 0 ]]; then
  fail "冲突分支应使 apply 停止并返回非零"
fi
[[ "$(git -C "$repo" rev-parse HEAD)" == "$(git -C "$repo" rev-parse origin/main)" ]] || fail "apply 模式未快进干净 main"
git -C "$tmp/clean" merge-base --is-ancestor origin/main clean || fail "clean 分支未基于最新 main"
[[ "$(git -C "$tmp/clean" log -1 --format=%s)" == "clean local change" ]] || fail "clean 本地提交丢失"
[[ "$(git -C "$tmp/dirty" rev-parse HEAD)" == "$dirty_before" ]] || fail "脏分支 HEAD 被改写"
[[ -f "$tmp/dirty/untracked.txt" ]] || fail "脏工作树的未跟踪文件被移除"
[[ "$(git -C "$tmp/published" rev-parse HEAD)" == "$published_before" ]] || fail "已发布分支 HEAD 被改写"
[[ "$(git -C "$tmp/same-name" rev-parse HEAD)" == "$same_name_before" ]] || fail "无 upstream 的同名远端分支 HEAD 被改写"
[[ "$(git -C "$tmp/gone" rev-parse HEAD)" == "$gone_before" ]] || fail "upstream 已删除分支 HEAD 被改写"
[[ "$(git -C "$tmp/detached" rev-parse HEAD)" == "$detached_before" ]] || fail "detached worktree HEAD 被改写"
[[ "$(git -C "$tmp/conflict" rev-parse HEAD)" == "$conflict_before" ]] || fail "冲突分支未恢复到 rebase 前提交"
[[ -z "$(git -C "$tmp/conflict" status --porcelain)" ]] || fail "冲突分支 abort 后仍有未清理状态"
grep -q '已停止同步' "$tmp/apply.log" || fail "未报告冲突导致停止"

printf '全部样例通过：只报告、干净本地分支 rebase、脏/已发布/upstream 丢失分支跳过、冲突 abort。\n'