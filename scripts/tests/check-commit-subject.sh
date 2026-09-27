#!/usr/bin/env bash
# check-commit-subject.sh 的样例测试：有效标题全部放行，无效标题全部拦下。
#
# 用法：bash scripts/tests/check-commit-subject.sh [被测脚本路径]
#   - 不给参数时测仓库里的 scripts/check-commit-subject.sh；给路径可测任意版本（便于红绿对照）。
#   - 只读、可重复、无需网络；不修改仓库与 Git 配置。
#
# 维护：改动 check-commit-subject.sh 的正则或参数时，先让这里的样例（尤其是 "bad" 组）覆盖到新规则。
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
target=${1:-"$repo_root/scripts/check-commit-subject.sh"}

if [[ ! -f "$target" ]]; then
  printf '用法: %s [被测脚本路径]\n错误: 找不到被测脚本（默认 %s）\n' "$0" "$repo_root/scripts/check-commit-subject.sh" >&2
  exit 2
fi

pass=0
fail=0

# check <ok|bad> <标题>
check() {
  local want=$1 subject=$2 rc
  if bash "$target" '样例' "$subject" >/dev/null 2>&1; then rc=0; else rc=$?; fi
  if { [[ "$want" == 'ok' && "$rc" -eq 0 ]] || [[ "$want" == 'bad' && "$rc" -ne 0 ]]; }; then
    printf '  OK   %-4s %s\n' "$want" "$subject"
    pass=$((pass + 1))
  else
    printf '  ✗    %-4s %s（退出码 %s）\n' "$want" "$subject" "$rc"
    fail=$((fail + 1))
  fi
}

printf '── check-commit-subject.sh 样例（被测：%s） ──\n' "$target"

# 有效：类型 + 可选 scope + 可选 ! + 冒号空格描述
check ok 'feat(workspace): 搭建 Cargo workspace 与 17 个库 crate 骨架'
check ok 'fix(scripts): changelog 判定兼容版本段在 [Unreleased] 之前'
check ok 'docs: 更新设计文档'
check ok 'chore(deps): 升级依赖'
check ok 'ci(pr-standards): 调整工作流参数'
check ok 'revert: 回退上一次提交'
check ok 'feat(api)!: 破坏性变更'

# 无效：缺类型、缺描述、中文冒号、大写字母、多余空格……
check bad '更新文档'
check bad 'feat：中文冒号'
check bad 'feat(Workspace): 大写 scope'
check bad 'Feat: 大写类型'
check bad 'feat:'
check bad 'feat:描述前缺空格'
check bad 'feat(): 空 scope'
check bad 'update: 不在允许的类型里'
check bad 'feat(x) : scope 与冒号之间有空格'

printf '\n合计：通过 %s，失败 %s\n' "$pass" "$fail"
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
printf '全部样例符合预期。\n'
