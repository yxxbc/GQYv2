#!/usr/bin/env bash
# 版本一致性检查：`version.txt`、`Cargo.toml` 的 `[workspace.package].version`、
# `.release-please-manifest.json` 三处必须一致，且 `release-please-config.json` 的
# `extra-files` 必须真的指向 Cargo.toml 里存在的版本键。
#
# 用法：bash scripts/check-version-consistency.sh [仓库根目录]
#   默认检查脚本所在仓库；给目录可检查任意副本（测试与排查用）。
# 退出码：0 通过；1 不一致（打印期望值与实际值）；2 用法或工具错误。
#
# 为什么需要它：发布 PR 由 GitHub 上的 Release Please 生成，本地没有别的门禁能发现
# 「extra-files 被改坏 / 版本文件被手改」——坏掉的表现是 Rust 侧版本静默停更。
# 设计依据：docs/release-versioning.md「为什么不是 rust release type」。
# 注意：Cargo.lock 里工作区成员的版本允许落后一步（由下一次 cargo 命令刷新），本脚本不检查它。
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=${1:-$(cd -- "$script_dir/.." && pwd)}

version_file="$root/version.txt"
cargo_toml="$root/Cargo.toml"
manifest="$root/.release-please-manifest.json"
config="$root/release-please-config.json"

for f in "$version_file" "$cargo_toml" "$manifest" "$config"; do
  if [[ ! -f "$f" ]]; then
    printf '用法: %s [仓库根目录]\n错误: 找不到 %s\n' "$0" "$f" >&2
    exit 2
  fi
done

fail=0

report() { # report <说明> <期望> <实际>
  printf '不一致：%s\n  期望 %s\n  实际 %s\n' "$1" "$2" "$3" >&2
  fail=1
}

# 读 TOML 某张表下的一个键（只认字面量标量，够用即可）。
read_toml_key() { # <文件> <点分表头> <键>
  awk -v want_table="[$2]" -v want_key="$3" '
    /^[[:space:]]*\[/ { section = $0; gsub(/[[:space:]]/, "", section); next }
    section == want_table && $0 ~ ("^[[:space:]]*" want_key "[[:space:]]*=") {
      line = $0
      sub(/^[^=]*=[[:space:]]*/, "", line)
      sub(/[[:space:]]*#.*$/, "", line)
      gsub(/"/, "", line)
      sub(/[[:space:]]+$/, "", line)
      print line
      exit
    }
  ' "$1"
}

version=$(tr -d '[:space:]' < "$version_file")
if [[ -z "$version" ]]; then
  printf '错误: %s 里没有版本号\n' "$version_file" >&2
  exit 2
fi

manifest_version=$(tr -d '[:space:]' < "$manifest" | sed -n 's/.*"\."[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')
if [[ -z "$manifest_version" ]]; then
  printf '错误: 无法从 %s 解析根包版本（期望形如 {".": "x.y.z"}）\n' "$manifest" >&2
  exit 2
fi

cargo_version=$(read_toml_key "$cargo_toml" 'workspace.package' 'version')
if [[ -z "$cargo_version" ]]; then
  printf '错误: 在 %s 的 [workspace.package] 里找不到 version\n' "$cargo_toml" >&2
  exit 2
fi

[[ "$version" == "$manifest_version" ]] || report '.release-please-manifest.json 的根包版本与 version.txt 不一致' "$version" "$manifest_version"
[[ "$version" == "$cargo_version" ]] || report 'Cargo.toml 的 [workspace.package].version 与 version.txt 不一致' "$version" "$cargo_version"

# extra-files：确认配置里仍有「type=toml + path=Cargo.toml」的条目，且它的 jsonpath 指向真实存在的键。
# JSON 逐键匹配（不依赖键顺序）；找不到条目时给出配置名，方便定位。
compact_config=$(tr -d '[:space:]' < "$config")
if [[ "$compact_config" != *'"type":"toml"'* || "$compact_config" != *'"path":"Cargo.toml"'* ]]; then
  report 'release-please-config.json 缺少 extra-files 的 toml 条目（发布 PR 将不再同步 Cargo.toml）' \
    '"extra-files": [{"type": "toml", "path": "Cargo.toml", "jsonpath": "…"}]' "$(printf '%s' "$compact_config" | sed -n 's/.*"packages".*"extra-files".*/（有 extra-files 但没有 toml+Cargo.toml 条目）/p')"
else
  jsonpath=$(printf '%s' "$compact_config" | sed -n 's/.*"jsonpath":"\([^"]*\)".*/\1/p' | head -1)
  if [[ "$jsonpath" != '$.'* ]]; then
    report 'release-please-config.json 的 extra-files jsonpath 不是以 $. 开头' '$.<表>.<键>' "$jsonpath"
  else
    path_only=${jsonpath#\$.}
    key=${path_only##*.}
    table=${path_only%.*}
    if [[ -z "$(read_toml_key "$cargo_toml" "$table" "$key")" ]]; then
      report "release-please-config.json 的 jsonpath 指向 Cargo.toml 里不存在的键（发布 PR 会静默不同步）" \
        "[$table] $key" '（不存在）'
    fi
  fi
fi

if [[ "$fail" -ne 0 ]]; then
  exit 1
fi

printf '版本一致：version.txt = Cargo.toml = manifest = %s；extra-files 指向 [workspace.package].version ✔\n' "$version"
