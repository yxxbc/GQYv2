#!/usr/bin/env bash
# check-version-consistency.sh 的样例测试：真实仓库通过；版本不一致、extra-files 被改坏、
# 缺文件等破坏必须分别变红。
#
# 用法：bash scripts/tests/check-version-consistency.sh [被测脚本路径]
#   - 不给参数时测仓库里的 scripts/check-version-consistency.sh；给路径可测任意版本（红绿对照）。
#   - 每个用例在 mktemp -d 的临时目录里复制一份版本相关文件再破坏，不碰当前 clone。
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd -- "$script_dir/../.." && pwd)
target=${1:-"$repo_root/scripts/check-version-consistency.sh"}

if [[ ! -f "$target" ]]; then
  printf '用法: %s [被测脚本路径]\n错误: 找不到被测脚本（默认 %s）\n' "$0" "$repo_root/scripts/check-version-consistency.sh" >&2
  exit 2
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

pass=0
fail=0

# new_root <名称>：复制版本相关的四个文件到 $tmp/<名称>，打印目录。
new_root() {
  local dir="$tmp/$1"
  mkdir -p "$dir"
  cp "$repo_root/version.txt" "$repo_root/.release-please-manifest.json" \
    "$repo_root/release-please-config.json" "$repo_root/Cargo.toml" "$dir/"
  printf '%s' "$dir"
}

# expect <名称> <期望退出码> <仓库根目录>
expect() {
  local name=$1 want=$2 root=$3 rc
  if bash "$target" "$root" >"$tmp/last.log" 2>&1; then rc=0; else rc=$?; fi
  if [[ "$rc" == "$want" ]]; then
    printf '  OK   %-46s 期望 %-2s 实际 %s\n' "$name" "$want" "$rc"
    pass=$((pass + 1))
  else
    printf '  ✗    %-46s 期望 %-2s 实际 %s\n' "$name" "$want" "$rc"
    sed 's/^/       /' "$tmp/last.log"
    fail=$((fail + 1))
  fi
}

printf '── check-version-consistency.sh 样例（被测：%s） ──\n' "$target"

printf '── 真实仓库 ──\n'
expect '真实仓库 → 通过' 0 "$repo_root"

printf '── 版本不一致 ──\n'
r=$(new_root cargo-drift)
sed -i.bak 's/^version = "[^"]*"/version = "0.9.9"/' "$r/Cargo.toml" && rm -f "$r/Cargo.toml.bak"
expect 'Cargo.toml 版本漂移 → 拦下' 1 "$r"

r=$(new_root manifest-drift)
sed -i.bak 's/"\.": "[^"]*"/".": "0.9.9"/' "$r/.release-please-manifest.json" && rm -f "$r/.release-please-manifest.json.bak"
expect 'manifest 版本漂移 → 拦下' 1 "$r"

printf '── extra-files 被改坏 ──\n'
r=$(new_root extras-removed)
sed -i.bak 's/"jsonpath": "\$\.workspace\.package\.version"//' "$r/release-please-config.json" && rm -f "$r/release-please-config.json.bak"
expect 'extra-files 的 jsonpath 被删 → 拦下' 1 "$r"

r=$(new_root extras-wrong-path)
sed -i.bak 's/"path": "Cargo.toml"/"path": "Cargo.lock"/' "$r/release-please-config.json" && rm -f "$r/release-please-config.json.bak"
expect 'extra-files 指向 Cargo.lock → 拦下' 1 "$r"

r=$(new_root extras-bad-key)
sed -i.bak 's/\$\.workspace\.package\.version/$.workspace.package.nope/' "$r/release-please-config.json" && rm -f "$r/release-please-config.json.bak"
expect 'jsonpath 指向不存在的键 → 拦下' 1 "$r"

printf '── 缺文件 ──\n'
r=$(new_root missing-version-file)
rm -f "$r/version.txt"
expect '缺 version.txt → 用法错误（退 2）' 2 "$r"

printf '\n合计：通过 %s，失败 %s（被测脚本：%s）\n' "$pass" "$fail" "$target"
if [[ "$fail" -gt 0 ]]; then
  exit 1
fi
printf '全部样例符合预期。\n'
