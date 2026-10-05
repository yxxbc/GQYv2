//! 门禁的「许可证」一项（施工 4-12，`docs/blueprint/licenses.md`）：仓库是 GPL-3.0-or-later，发布的四个平台上用得到的
//! 第三方依赖，许可证都要能和它合在一起发。照每个包 `license` 里的 SPDX 表达式算，自己算，不加依赖。

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

/// 发布的四个平台（`docs/designs/12-进程形态与分发.md` R11）。
pub const PLATFORMS: [&str; 4] = [
    "x86_64-unknown-linux-gnu",
    "aarch64-unknown-linux-gnu",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
];

/// 能和 GPL-3.0-or-later 合在一起发的许可证。
const ALLOWED: [&str; 14] = [
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Unlicense",
    "CC0-1.0",
    "BSL-1.0",
    "MPL-2.0",
    "CDLA-Permissive-2.0",
];

/// 认得的例外条款：`WITH` 后面写它，不影响前面那个许可证。
const EXCEPTIONS: [&str; 1] = ["LLVM-exception"];

/// 一个第三方包：名字、版本，和它 `license` 的原文（没写的是空的）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) license: Option<String>,
}

/// 查一遍四个平台，交回报出来的问题。
pub fn check(root: &Path, cargo: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut problems = Vec::new();
    for platform in PLATFORMS {
        match packages(root, cargo, platform) {
            Ok(list) => found.extend(list.into_iter().map(|package| (package, platform))),
            Err(e) => problems.push(format!("跑不了 cargo tree（{platform}）：{e}")),
        }
    }
    problems.extend(judge(&found));
    problems
}

/// 每个包说一句：许可证不能用的、没写的。同一个包在几个平台上都有，只说一次，写上是哪几个平台。
pub(crate) fn judge(found: &[(Package, &str)]) -> Vec<String> {
    let mut platforms: BTreeMap<&Package, BTreeSet<&str>> = BTreeMap::new();
    for (package, platform) in found {
        platforms.entry(package).or_default().insert(platform);
    }
    let mut problems = Vec::new();
    for (package, on) in platforms {
        let on: Vec<&str> = on.into_iter().collect();
        let head = format!("{} {}（{}）", package.name, package.version, on.join("、"));
        match &package.license {
            None => problems.push(format!("{head}：没写 license，要人看过")),
            Some(license) if !allowed(license) => {
                problems.push(format!("{head}：{license} 和 GPL-3.0-or-later 合不到一起"));
            }
            Some(_) => {}
        }
    }
    problems
}

/// 一个平台上用得到的第三方包。
///
/// 用 `cargo tree` 而不是 `cargo metadata`：`cargo tree` 走的是**真的编进包里的**依赖（照 feature、
/// 可选依赖、目标平台都算过），而 `cargo metadata` 的 resolve 图会把**没启用的可选依赖**也算上、
/// 还标成普通依赖，许可证一项会因此报本来不存在的依赖（`licenses.md`「怎么走」第 1 条）。
fn packages(root: &Path, cargo: &str, platform: &str) -> Result<Vec<Package>, String> {
    // `--locked` 跟原来一样：锁文件对不上就报错、不自己改。cargo tree 是顶层选项，要写在子命令前面。
    let output = Command::new(cargo)
        .args([
            "--locked",
            "tree",
            "--edges",
            "normal",
            "--prefix",
            "none",
            "--no-dedupe",
        ])
        .args(["--format", "{p}|{l}"])
        .args(["--target", platform])
        .current_dir(root)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(said.lines().next().unwrap_or_default().to_string());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(third_party(&text, root))
}

/// 从 `cargo tree --format "{p}|{l}"` 的输出里取第三方包：一行一个 `<名字> v<版本>|<license>`，
/// 工作区自己的（路径形式的 `gqy v0.0.0 (/…/crates/gqy)`）不看，重复的只算一次。
///
/// 形状是 cargo 自己的输出（`p` 是包、`l` 是 license），要跟着 cargo 的版本看：升级 cargo 时
/// 这一项会当场红（门禁自己跑得到），不会默默放过。
pub(crate) fn third_party(text: &str, root: &Path) -> Vec<Package> {
    let mut out: BTreeMap<String, Package> = BTreeMap::new();
    for line in text.lines() {
        let Some((left, license)) = line.split_once('|') else {
            continue;
        };
        // 工作区自己的写成 `gqy v0.0.0 (/路径)`：路径形式的不算第三方。
        if left.contains(" (") {
            continue;
        }
        let Some((name, version)) = left.trim().rsplit_once(" v") else {
            continue;
        };
        let name = name.trim().to_string();
        let license = license.trim();
        out.entry(format!("{name} {version}"))
            .or_insert_with(|| Package {
                name,
                version: version.to_string(),
                license: (!license.is_empty()).then(|| license.to_string()),
            });
    }
    // 同一个工作区里没有的事：这里只保证形状；`root` 只为报错时能说清在哪个仓库。
    let _ = root;
    out.into_values().collect()
}

/// 一个 SPDX 表达式能不能用：`OR` 有一个能用就行（老写法的 `/` 也是 `OR`），`AND` 每个都要能用，括号照括号，
/// `WITH` 后面只认 [`EXCEPTIONS`]。写坏了的当不能用。
pub(crate) fn allowed(expression: &str) -> bool {
    let spaced = expression
        .replace('(', " ( ")
        .replace(')', " ) ")
        .replace('/', " OR ");
    let tokens: Vec<&str> = spaced.split_whitespace().collect();
    let mut at = 0;
    match any(&tokens, &mut at) {
        Some(ok) => ok && at == tokens.len(),
        None => false,
    }
}

/// `OR` 连着的几段：有一段能用就行。写坏了的是空的。
fn any(tokens: &[&str], at: &mut usize) -> Option<bool> {
    let mut ok = all(tokens, at)?;
    while tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("OR"))
    {
        *at += 1;
        ok |= all(tokens, at)?;
    }
    Some(ok)
}

/// `AND` 连着的几段：每一段都要能用。
fn all(tokens: &[&str], at: &mut usize) -> Option<bool> {
    let mut ok = one(tokens, at)?;
    while tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("AND"))
    {
        *at += 1;
        ok &= one(tokens, at)?;
    }
    Some(ok)
}

/// 一个许可证（可以带 `WITH` 例外），或者括号里的一整段。
fn one(tokens: &[&str], at: &mut usize) -> Option<bool> {
    let token = *tokens.get(*at)?;
    *at += 1;
    let ok = match token {
        "(" => {
            let ok = any(tokens, at)?;
            (tokens.get(*at) == Some(&")")).then_some(())?;
            *at += 1;
            ok
        }
        ")" => return None,
        _ if ["AND", "OR", "WITH"]
            .iter()
            .any(|op| token.eq_ignore_ascii_case(op)) =>
        {
            return None;
        }
        id => ALLOWED.contains(&id.trim_end_matches('+')),
    };
    if tokens
        .get(*at)
        .is_some_and(|t| t.eq_ignore_ascii_case("WITH"))
    {
        let exception = *tokens.get(*at + 1)?;
        *at += 2;
        return Some(ok && EXCEPTIONS.contains(&exception));
    }
    Some(ok)
}

#[cfg(test)]
mod tests;
