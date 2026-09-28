//! 体积门禁的 fixture（P00-03「测试与守护」）：在临时目录造 800 / 801 / 1501 / 2001 行的
//! 代码文件与几类应被忽略的文件，断言 `check_size` 的定档、排序与扫描范围。
//! 去掉 `Severity::of` 的任一分档、去掉扩展名过滤或目录过滤，对应断言必然红。
//!
//! 测试放开（P00-03）：集成测试允许 unwrap/expect/panic/索引。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::Path;

use xtask::size::{EXPLAIN_LINES, FAIL_LINES, Severity, WARN_LINES, check_size};

/// 写一个 `lines` 行的文件（每行 `// filler`，带尾换行符）；父目录不存在时一并创建。
fn write_file(path: &Path, lines: usize) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("建父目录");
    }
    fs::write(path, "// filler\n".repeat(lines)).expect("写文件");
}

/// 在临时目录里扫描。
fn scan(dir: &tempfile::TempDir) -> Vec<xtask::size::SizeFinding> {
    check_size(dir.path()).expect("扫描临时目录")
}

#[test]
fn passes_at_the_target_line_count() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join("a.rs"), WARN_LINES);
    let findings = scan(&tmp);
    assert!(findings.is_empty(), "{WARN_LINES} 行整应通过：{findings:?}");
}

#[test]
fn warns_just_over_the_target() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join("a.rs"), WARN_LINES + 1);
    let findings = scan(&tmp);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].severity, Severity::Warn, "{findings:?}");
    assert_eq!(findings[0].lines, WARN_LINES + 1);
}

#[test]
fn needs_split_plan_just_over_the_limit() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join("a.rs"), EXPLAIN_LINES + 1);
    let findings = scan(&tmp);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].severity, Severity::Explain, "{findings:?}");
}

#[test]
fn fails_just_over_the_red_line() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join("a.rs"), FAIL_LINES + 1);
    let findings = scan(&tmp);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(findings[0].severity, Severity::Fail, "{findings:?}");
}

#[test]
fn ranks_fail_before_explain_before_warn() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join("warn.rs"), WARN_LINES + 5);
    write_file(&tmp.path().join("fail.rs"), FAIL_LINES + 100);
    write_file(&tmp.path().join("explain.rs"), EXPLAIN_LINES + 5);
    let findings = scan(&tmp);
    let order: Vec<(&str, Severity)> = findings
        .iter()
        .map(|finding| (finding.path.as_str(), finding.severity))
        .collect();
    assert_eq!(
        order,
        vec![
            ("fail.rs", Severity::Fail),
            ("explain.rs", Severity::Explain),
            ("warn.rs", Severity::Warn),
        ],
        "报告要按严重度降序：{findings:?}"
    );
}

#[test]
fn ignores_other_extensions() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join("big.md"), FAIL_LINES + 1);
    write_file(&tmp.path().join("big.txt"), FAIL_LINES + 1);
    write_file(&tmp.path().join("big.json"), FAIL_LINES + 1);
    let findings = scan(&tmp);
    assert!(findings.is_empty(), "只有 .rs/.ts/.css 受管：{findings:?}");
}

#[test]
fn checks_ts_and_css_too() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join("big.ts"), WARN_LINES + 1);
    write_file(&tmp.path().join("big.css"), WARN_LINES + 1);
    let findings = scan(&tmp);
    assert_eq!(findings.len(), 2, "{findings:?}");
    assert!(
        findings.iter().all(|f| f.severity == Severity::Warn),
        "{findings:?}"
    );
}

#[test]
fn skips_hidden_target_and_node_modules() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join(".hidden/big.rs"), FAIL_LINES + 1);
    write_file(&tmp.path().join("target/big.rs"), FAIL_LINES + 1);
    write_file(&tmp.path().join("node_modules/big.ts"), FAIL_LINES + 1);
    let findings = scan(&tmp);
    assert!(
        findings.is_empty(),
        "隐藏目录 / target / node_modules 要跳过（.worktrees 是隐藏目录）：{findings:?}"
    );
}

#[test]
fn counts_last_line_without_trailing_newline() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    let path = tmp.path().join("a.rs");
    let mut text = "// filler\n".repeat(WARN_LINES);
    text.push_str("// filler"); // 末行无换行符，仍然是第 801 行
    fs::write(&path, text).expect("写文件");
    let findings = scan(&tmp);
    assert_eq!(findings.len(), 1, "末行无换行也要计入：{findings:?}");
    assert_eq!(findings[0].lines, WARN_LINES + 1, "{findings:?}");
}

#[test]
fn reports_paths_relative_to_root() {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_file(&tmp.path().join("crates/gqy-x/src/big.rs"), FAIL_LINES + 1);
    let findings = scan(&tmp);
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert_eq!(
        findings[0].path, "crates/gqy-x/src/big.rs",
        "路径要相对仓库根、以 / 分隔：{findings:?}"
    );
}
