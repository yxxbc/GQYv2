#!/usr/bin/env bash
# check-pr-standards.sh 的样例测试：在临时 git 仓库里重放各种 PR 形态，断言退出码。
#
# 用法：bash scripts/tests/check-pr-standards.sh [被测脚本路径]
#   - 不给参数时测仓库里的 scripts/check-pr-standards.sh；给路径可测任意版本（便于红绿对照）。
#   - 覆盖：PR 标题、提交标题、BREAKING CHANGE footer、merge commit、空提交范围、
#     changelog 段判定（Unreleased / 版本段在前 / 版本段在后 / 无条目 / 无关段）与 skip-changelog 两条路径。
#   - 全部在 mktemp -d 的临时仓库里做：不改当前 clone、不读网络、不需要 secrets。
#
# 维护：改动 check-pr-standards.sh 的判定或退出码时，同步改这里的样例；
# 用 `bash scripts/tests/check-pr-standards.sh <旧版脚本>` 应能让对应用例变红（区分能力）。
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
target=${1:-"$repo_root/scripts/check-pr-standards.sh"}

if [[ ! -f "$target" ]]; then
  printf '用法: %s [被测脚本路径]\n错误: 找不到被测脚本（默认 %s）\n' "$0" "$repo_root/scripts/check-pr-standards.sh" >&2
  exit 2
fi

if ! command -v git >/dev/null 2>&1; then
  printf '错误: 需要 git\n' >&2
  exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

cd "$tmp"
git init -q .
git config user.email 'script-test@example.com'
git config user.name 'script test'
git config commit.gpgsign false
default_branch=$(git symbolic-ref --short HEAD)

# 基线：与仓库当前形态一致（有 `## [Unreleased]`，没有版本段）。
printf '# Changelog\n\n说明。\n\n## [Unreleased]\n' > CHANGELOG.md
printf 'base\n' > README.md
git add -A
git commit -qm 'docs: base'
base=$(git rev-parse HEAD)

pass=0
fail=0

# expect <名称> <期望退出码> <PR 标题> <skip 标签> <head>
# 运行被测脚本并把退出码与期望比较；不符时打印脚本输出（含错误信息），便于定位。
expect() {
  local name=$1 want=$2 title=$3 skip=$4 head=$5 rc
  if bash "$target" "$base" "$head" "$title" "$skip" >"$tmp/last.log" 2>&1; then rc=0; else rc=$?; fi
  if [[ "$rc" == "$want" ]]; then
    printf '  OK   %-52s 期望 %-2s 实际 %s\n' "$name" "$want" "$rc"
    pass=$((pass + 1))
  else
    printf '  ✗    %-52s 期望 %-2s 实际 %s\n' "$name" "$want" "$rc"
    sed 's/^/       /' "$tmp/last.log"
    fail=$((fail + 1))
  fi
}

# commit_head <提交标题> [正文]：提交当前工作区改动，打印新 HEAD。
commit_head() {
  git add -A
  if [[ $# -ge 2 ]]; then
    git commit -qm "$1" -m "$2"
  else
    git commit -qm "$1"
  fi
  git rev-parse HEAD
}

# 切到一条侧支做「污染历史」的用例，用完切回主线（否则后续用例都会被它带红）。
side_branch() {
  git checkout -q -b "$1" "$default_branch"
}
back_to_default() {
  git checkout -q "$default_branch"
}

# ── 用例 ──────────────────────────────────────────────────────────────────

printf '── PR 标题、提交范围与 skip 标签 ──\n'
printf 'a\n' >> README.md
head_readme=$(commit_head 'docs: 改 README')
expect 'PR 标题非法 → 拦下' 1 '更新文档' 'true' "$head_readme"
expect 'CHANGELOG 未动 + skip=true → 放行' 0 'docs: 改 README' 'true' "$head_readme"
expect 'CHANGELOG 未动 + skip=false → 拦下' 1 'docs: 改 README' 'false' "$head_readme"
expect 'head 等于 base（空提交范围）→ 拦下' 1 'docs: 空范围' 'true' "$base"

printf '── 提交标题（侧支，不污染主线） ──\n'
side_branch bad-subject
printf 'b\n' >> README.md
head_bad_subject=$(commit_head '更新文档')
back_to_default
expect '提交标题非法 → 拦下' 1 'docs: 检查' 'true' "$head_bad_subject"

printf '── 破坏性提交的 BREAKING CHANGE footer ──\n'
side_branch breaking-ok
printf 'c\n' >> README.md
git add -A
git commit -qm 'feat(api)!: 调整接口' -m 'BREAKING CHANGE: 旧调用方式不再支持'
head_breaking_ok=$(git rev-parse HEAD)
back_to_default
expect '含 BREAKING CHANGE footer → 放行（skip）' 0 'feat(api)!: 调整接口' 'true' "$head_breaking_ok"

side_branch breaking-bad
printf 'd\n' >> README.md
head_breaking_bad=$(commit_head 'feat(api)!: 调整接口')
back_to_default
expect 'feat! 缺 BREAKING CHANGE footer → 拦下' 1 'feat(api)!: 调整接口' 'true' "$head_breaking_bad"

# ── changelog 段判定 ──────────────────────────────────────────────────────

cl_unreleased_entry() {
  cat > CHANGELOG.md <<'EOF'
# Changelog

说明。

## [Unreleased]

### Added

* 新功能（#12）

## [0.1.0](https://example.com/compare/v0.0.1...v0.1.0) (2026-01-01)

### Added

* 旧功能
EOF
}

cl_release_first() {
  cat > CHANGELOG.md <<'EOF'
# Changelog

说明。

## [0.2.0](https://example.com/compare/v0.1.0...v0.2.0) (2026-09-27)

### Added

* **workspace:** 搭建 Cargo workspace（#6）

## [Unreleased]
EOF
}

cl_release_after() {
  cat > CHANGELOG.md <<'EOF'
# Changelog

说明。

## [Unreleased]

## [0.2.0](https://example.com/compare/v0.1.0...v0.2.0) (2026-09-27)

### Added

* **workspace:** 搭建 Cargo workspace（#6）
EOF
}

cl_category_no_entry() {
  cat > CHANGELOG.md <<'EOF'
# Changelog

说明。

## [Unreleased]

### Added

## [0.1.0](https://example.com/compare/v0.0.1...v0.1.0) (2026-01-01)
EOF
}

cl_other_section_entry() {
  cat > CHANGELOG.md <<'EOF'
# Changelog

说明。

## [Unreleased]

## Notes

### Added

* 条目在无关段下
EOF
}

printf '── changelog 段判定 ──\n'
cl_unreleased_entry
h_unreleased=$(commit_head 'docs: changelog Unreleased 下有分类与条目')
expect 'Unreleased 下有分类标题与条目 → 放行' 0 'docs: 检查 changelog' 'false' "$h_unreleased"

cl_release_first
h_release_first=$(commit_head 'docs: changelog 版本段在 Unreleased 之前')
expect '版本段在 Unreleased 之前（Release Please 形态）→ 放行' 0 'chore: release 0.2.0' 'false' "$h_release_first"

cl_release_after
h_release_after=$(commit_head 'docs: changelog 版本段在 Unreleased 之后')
expect '版本段在 Unreleased 之后 → 放行' 0 'docs: 检查 changelog' 'false' "$h_release_after"

cl_category_no_entry
h_no_entry=$(commit_head 'docs: changelog 分类标题下没有条目')
expect '分类标题下没有条目 → 拦下' 1 'docs: 检查 changelog' 'false' "$h_no_entry"

cl_other_section_entry
h_other=$(commit_head 'docs: changelog 条目在无关段下')
expect '条目在无关段（## Notes）下 → 拦下' 1 'docs: 检查 changelog' 'false' "$h_other"

# ── 分支形态（会产生 merge commit，污染主线，放最后） ──────────────────────

printf '── 分支形态 ──\n'
side_branch side-merge
printf 's\n' > side.txt
head_side=$(commit_head 'chore: 侧支改动')
back_to_default
git merge -q --no-ff side-merge -m 'chore: 合并侧支'
head_merged=$(git rev-parse HEAD)
expect '提交范围含 merge commit → 拦下' 1 'chore: 合并侧支' 'true' "$head_merged"

printf '\n合计：通过 %s，失败 %s（被测脚本：%s）\n' "$pass" "$fail" "$target"
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
printf '全部样例符合预期。\n'
