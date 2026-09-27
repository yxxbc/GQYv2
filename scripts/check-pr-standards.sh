#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 5 ]]; then
  printf '用法: %s <base-sha> <head-sha> <pr-title> <skip-changelog-label> <large-commit-approved-label>\n' "$0" >&2
  exit 2
fi

base_sha=$1
head_sha=$2
pr_title=$3
skip_changelog_label=$4
large_commit_approved_label=$5
script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
max_commit_changed_lines=500
max_commit_changed_files=10

check_subject() {
  bash "$script_dir/check-commit-subject.sh" "$1" "$2"
}

check_subject 'PR 标题' "$pr_title"

commit_range="$base_sha..$head_sha"
merge_commits=$(git rev-list --merges "$commit_range")
if [[ -n "$merge_commits" ]]; then
  printf 'PR 分支不得包含 merge commit，请 rebase 整理提交。\n' >&2
  exit 1
fi

commit_shas=$(git rev-list "$commit_range")
if [[ -z "$commit_shas" ]]; then
  printf 'PR 分支没有可检查的提交。\n' >&2
  exit 1
fi

while IFS= read -r commit_sha; do
  subject=$(git show -s --format=%s "$commit_sha")
  commit_body=$(git show -s --format=%b "$commit_sha")
  check_subject '提交标题' "$subject"

  change_stats=$(git show --format= --numstat "$commit_sha")
  changed_files=$(printf '%s\n' "$change_stats" | awk 'NF >= 3 { count++ } END { print count + 0 }')
  changed_lines=$(printf '%s\n' "$change_stats" | awk -F '\t' '$1 ~ /^[0-9]+$/ && $2 ~ /^[0-9]+$/ { total += $1 + $2 } END { print total + 0 }')
  if [[ ( "$changed_files" -gt "$max_commit_changed_files" || "$changed_lines" -gt "$max_commit_changed_lines" ) && "$large_commit_approved_label" != 'true' ]]; then
    printf '提交改动过大：%s 个文件、%s 行增删；上限为 %s 个文件或 %s 行。拆分提交，或由维护者添加 large-commit-approved 标签。\n' \
      "$changed_files" "$changed_lines" "$max_commit_changed_files" "$max_commit_changed_lines" >&2
    exit 1
  fi

  if [[ "$subject" == *'!: '* ]] && ! printf '%s\n' "$commit_body" | grep -Eq '^BREAKING CHANGE: .+'; then
    printf '破坏性提交必须在提交正文中包含非空的 BREAKING CHANGE: footer: %s\n' "$subject" >&2
    exit 1
  fi
done <<< "$commit_shas"

if git diff --quiet "$base_sha...$head_sha" -- CHANGELOG.md; then
  if [[ "$skip_changelog_label" != 'true' ]]; then
    printf '此 PR 必须更新 CHANGELOG.md；纯内部变更请由维护者添加 skip-changelog 标签。\n' >&2
    exit 1
  fi
  printf '提交标题通过；已按 skip-changelog 标签豁免变更日志。\n'
  exit 0
fi

if ! awk '
  # 段状态：unreleased = ## [Unreleased]；release = 版本标题（## [0.2.0](…) 之类）；other = 其它二级标题。
  # 版本标题不要求先出现 ## [Unreleased]：Release Please 把新版本段插在文件最前（自动发布 PR 的形态）。
  /^## \[Unreleased\]$/ { section = "unreleased"; next }
  /^## / {
    if (/^## \[v?[0-9]+\.[0-9]+\.[0-9]+/) {
      section = "release"
    } else {
      section = "other"
    }
    in_category = 0
    next
  }
  (section == "unreleased" || section == "release") && /^### (Added|Changed|Deprecated|Removed|Fixed|Security)$/ {
    in_category = 1
    next
  }
  (section == "unreleased" || section == "release") && /^### / { in_category = 0 }
  section == "unreleased" && in_category && /^[*-] .+/ { found_unreleased_entry = 1 }
  section == "release" && in_category && /^[*-] .+/ { found_release_entry = 1 }
  END { exit !(found_unreleased_entry || found_release_entry) }
' CHANGELOG.md; then
  printf 'CHANGELOG.md 必须在 ## [Unreleased] 或最新版本标题下包含分类标题及至少一条列表记录。\n' >&2
  exit 1
fi

printf '提交标题和变更日志检查通过。\n'