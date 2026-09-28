//! clippy 门禁的违规 fixture（P00-03「测试与守护」）：在临时目录建无依赖 mini workspace，
//! 验证 workspace lints 的规则真的会让违规变红、把 lint 从配置里去掉后同一份代码不报
//! （证明红是配置在起作用），以及测试放宽样板只覆盖 `cfg(test)`。
//! 这是慢测试（每次跑一次 `cargo clippy`）：无依赖 crate，实测单次约 0.3–1 秒，默认运行；
//! 若将来超过施工单约定的 15 秒，按 P00-03「风险与回退」改 `#[ignore]` 并在 CI 显式运行。
//!
//! 测试放开（P00-03）：集成测试允许 unwrap/expect/panic/索引。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 跑 `cargo clippy --all-targets -- -D warnings`，返回（是否成功, 合并输出）。
///
/// 隔离环境：清掉 `CARGO_TARGET_DIR`（免得与真实仓库的 target 互相阻塞）与 `RUSTFLAGS`
/// （fixture 要按自己的配置判定，不受调用者的额外 flag 影响）。
fn clippy(dir: &Path) -> (bool, String) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["clippy", "--all-targets", "--", "-D", "warnings"])
        .current_dir(dir)
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTFLAGS")
        .output()
        .expect("启动 cargo clippy");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

/// 建 mini workspace：`lints` 是根清单 `[workspace.lints.clippy]` 段的内容，`source` 是成员源码。
fn write_fixture(dir: &Path, lints: &str, source: &str) {
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [\"m\"]\n\n\
             [workspace.package]\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
             [workspace.lints.clippy]\n{lints}\n"
        ),
    )
    .expect("写 workspace 清单");
    fs::create_dir_all(dir.join("m/src")).expect("建成员目录");
    fs::write(
        dir.join("m/Cargo.toml"),
        "[package]\nname = \"m\"\nversion.workspace = true\nedition.workspace = true\n\
         publish = false\n\n[lints]\nworkspace = true\n",
    )
    .expect("写成员清单");
    fs::write(dir.join("m/src/lib.rs"), source).expect("写成员源码");
}

/// 仓库根（`xtask/` 的上一级）。
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 的上一级是仓库根")
        .to_path_buf()
}

/// 提取 TOML 里 `[name]` 段的内容（到下一个 `[` 开头的行为止）；段为空则返回 `None`。
fn extract_section(manifest: &str, name: &str) -> Option<String> {
    let header = format!("[{name}]");
    let mut out = String::new();
    let mut in_section = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if trimmed == header {
                in_section = true;
                continue;
            }
            if in_section {
                break;
            }
        }
        if in_section {
            out.push_str(line);
            out.push('\n');
        }
    }
    (!out.trim().is_empty()).then_some(out)
}

/// 读真实仓库 `Cargo.toml` 的 `[workspace.lints.clippy]` 段：fixture 验证的就是这份配置。
///
/// 这是刻意的：测试不另造一份 lint 清单，配置被改坏（删掉某条 lint）时这里的断言必然变红。
fn real_clippy_lints() -> String {
    let manifest = fs::read_to_string(repo_root().join("Cargo.toml")).expect("读仓库根 Cargo.toml");
    extract_section(&manifest, "workspace.lints.clippy")
        .expect("Cargo.toml 里应当有非空的 [workspace.lints.clippy]")
}

/// 从一段 lints 文本里删掉某条规则（用于「去掉配置后不报」的对照）。
fn without_rule(lints: &str, rule: &str) -> String {
    lints
        .lines()
        .filter(|line| !line.trim_start().starts_with(rule))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 生产代码里的 `unwrap()`（有依赖时的典型违规写法）。
///
/// 带 `# Panics` 段落，让 `missing_panics_doc`（同一份真实配置里的另一条 lint）不介入，
/// 「去掉 `unwrap_used` 就不报」的对照才是单变量的。
const UNWRAP_SOURCE: &str = r#"/// 生产代码里的 unwrap。
///
/// # Panics
/// 输入不是数字时 panic。
pub fn prod() -> u32 {
    "1".parse::<u32>().unwrap()
}
"#;

/// 测试模块里的 `unwrap()`（模拟真实 crate 的 `#[cfg(test)] mod tests`）。
const TEST_UNWRAP_SOURCE: &str = r#"pub fn prod() -> u32 {
    7
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {
        let value: Result<u32, _> = "1".parse::<u32>();
        assert_eq!(value.unwrap(), 1);
    }
}
"#;

/// 同上，但带 P00-03 的测试放宽样板（只影响 `cfg(test)` 编译）。
const TEST_UNWRAP_WITH_ALLOW_SOURCE: &str = r#"#![cfg_attr(test, allow(clippy::unwrap_used))]
pub fn prod() -> u32 {
    7
}

#[cfg(test)]
mod tests {
    #[test]
    fn t() {
        let value: Result<u32, _> = "1".parse::<u32>();
        assert_eq!(value.unwrap(), 1);
    }
}
"#;

#[test]
fn unwrap_in_production_code_fails() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_fixture(tmp.path(), &real_clippy_lints(), UNWRAP_SOURCE);
    let (ok, output) = clippy(tmp.path());
    assert!(!ok, "生产代码 unwrap 应当报红：{output}");
    assert!(
        output.contains("unwrap_used"),
        "要报 clippy::unwrap_used：{output}"
    );
}

#[test]
fn same_code_passes_when_lint_not_configured() {
    // 关键对照：同一份代码、同一份真实配置，只去掉 unwrap_used 这一条就不报——
    // 证明上一条的红来自配置，而不是别的巧合。
    let tmp = tempfile::tempdir().expect("建临时目录");
    let lints = without_rule(&real_clippy_lints(), "unwrap_used");
    write_fixture(tmp.path(), &lints, UNWRAP_SOURCE);
    let (ok, output) = clippy(tmp.path());
    assert!(ok, "去掉 lint 后同一份代码不应报：{output}");
}

#[test]
fn dropped_result_fails() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_fixture(
        tmp.path(),
        &real_clippy_lints(),
        "pub fn drop_it() {\n    let _ = std::fs::read_to_string(\"/nonexistent-gqy-fixture\");\n}\n",
    );
    let (ok, output) = clippy(tmp.path());
    assert!(!ok, "`let _ = result` 应当报红：{output}");
    assert!(
        output.contains("let_underscore_must_use"),
        "要报 clippy::let_underscore_must_use：{output}"
    );
}

#[test]
fn squashed_result_fails() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_fixture(
        tmp.path(),
        &real_clippy_lints(),
        "pub fn squash_it() {\n    std::fs::read_to_string(\"/nonexistent-gqy-fixture\").ok();\n}\n",
    );
    let (ok, output) = clippy(tmp.path());
    assert!(!ok, "`result.ok();` 压掉 must_use 应当报红：{output}");
    assert!(
        output.contains("unused_result_ok"),
        "要报 clippy::unused_result_ok：{output}"
    );
}

#[test]
fn cfg_test_allow_keeps_test_unwrap_green() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_fixture(
        tmp.path(),
        &real_clippy_lints(),
        TEST_UNWRAP_WITH_ALLOW_SOURCE,
    );
    let (ok, output) = clippy(tmp.path());
    assert!(ok, "带 cfg_attr 放宽的测试代码应当通过：{output}");
}

#[test]
fn test_unwrap_still_fails_without_cfg_attr() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_fixture(tmp.path(), &real_clippy_lints(), TEST_UNWRAP_SOURCE);
    let (ok, output) = clippy(tmp.path());
    assert!(!ok, "没有放宽样板时测试里的 unwrap 也要报红：{output}");
    assert!(output.contains("unwrap_used"), "{output}");
}

#[test]
fn workspace_lints_cover_the_required_rules() {
    // 19 §6.1 第 2 项的清单不许悄悄缩水：少一条这里就红。
    let lints = real_clippy_lints();
    for rule in [
        "unwrap_used",
        "expect_used",
        "panic",
        "indexing_slicing",
        "let_underscore_must_use",
        "unused_result_ok",
        "await_holding_lock",
        "large_futures",
        "missing_errors_doc",
        "missing_panics_doc",
        "disallowed_methods",
    ] {
        assert!(
            lints
                .lines()
                .any(|line| line.trim_start().starts_with(rule)),
            "Cargo.toml 的 [workspace.lints.clippy] 缺 {rule}：\n{lints}"
        );
    }
}

#[test]
fn clippy_toml_lists_four_disallowed_methods_with_reasons() {
    // 02 §2 的四条禁止调用（P00-03 完成判据）；每条都要带 reason。
    let toml = fs::read_to_string(repo_root().join("clippy.toml")).expect("读 clippy.toml");
    for method in [
        "tokio::runtime::Runtime::new",
        "tokio::runtime::Handle::block_on",
        "futures::executor::block_on",
        "tokio::sync::mpsc::unbounded_channel",
    ] {
        assert!(toml.contains(method), "clippy.toml 缺 {method}：\n{toml}");
    }
    assert_eq!(
        toml.matches("reason =").count(),
        4,
        "disallowed-methods 的 4 条都要带 reason：\n{toml}"
    );
}
