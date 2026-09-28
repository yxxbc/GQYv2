//! 静态规则扫描器的 fixture：三条规则各一组违规/合规样例（P00-02「测试与守护」）。
//!
//! 直接喂源码字符串，不编译被测代码；去掉扫描器里的任一条模式，对应断言必然红。
//! 创建：AI 助手（Cline 会话），2026-09-28 06:31:10。
//! 更新：AI 助手（Cline 会话），2026-09-28 22:21:06（P00-03 集成测试放开）。

// 集成测试整文件放开（19 §6.1；P00-03）：测试里允许 unwrap/expect/panic/索引。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::path::Path;

use xtask::arch::scan::{scan_repo, scan_source};

#[test]
fn flags_task_local() {
    let violations = scan_source(
        "crates/gqy-core/src/lib.rs",
        "task_local! {\n    static CTX: u32 = 0;\n}\n",
    );
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(violations[0].rule, "task-local");
    assert!(
        violations[0].location.ends_with(":1"),
        "要带行号：{:?}",
        violations[0]
    );
}

#[test]
fn reports_line_numbers() {
    let source = "// 注释\nfn a() {}\n\ntask_local! { static X: u32 = 0; }\n";
    let violations = scan_source("crates/gqy-core/src/lib.rs", source);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].location.ends_with(":4"),
        "行号应为 4：{:?}",
        violations[0]
    );
}

#[test]
fn allows_plain_code() {
    let source = "pub fn add(a: u32, b: u32) -> u32 {\n    a + b\n}\n";
    assert!(scan_source("crates/gqy-core/src/lib.rs", source).is_empty());
}

#[test]
fn flags_mutable_static() {
    let source = "static CACHE: Mutex<Vec<u8>> = Mutex::new(Vec::new());\n";
    let violations = scan_source("crates/gqy-store/src/lib.rs", source);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(violations[0].rule, "mutable-static");
    assert!(
        violations[0].expected.contains("可变全局"),
        "期望值要说明禁的是可变全局状态：{:?}",
        violations[0]
    );
}

#[test]
fn flags_mutable_static_with_visibility() {
    let source = "pub(crate) static REGISTRY: RwLock<BTreeMap<String, u32>> = RwLock::new(BTreeMap::new());\n";
    let violations = scan_source("crates/gqy-engine/src/lib.rs", source);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(violations[0].rule, "mutable-static");
}

#[test]
fn allows_once_lock_static() {
    let source = "static LIMITS: OnceLock<Limits> = OnceLock::new();\n";
    assert!(scan_source("crates/gqy-core/src/lib.rs", source).is_empty());
}

#[test]
fn flags_instrument_without_skip_all() {
    let violations = scan_source(
        "crates/gqy-engine/src/run.rs",
        "#[instrument]\nfn run() {}\n",
    );
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert_eq!(violations[0].rule, "instrument-skip-all");
    assert!(
        violations[0].expected.contains("skip_all"),
        "期望值要说明要带 skip_all：{:?}",
        violations[0]
    );
}

#[test]
fn flags_multiline_instrument_without_skip_all() {
    let source = "#[instrument(\n    name = \"turn\"\n)]\nfn run() {}\n";
    let violations = scan_source("crates/gqy-engine/src/run.rs", source);
    assert_eq!(violations.len(), 1, "{violations:?}");
    assert!(
        violations[0].location.ends_with(":1"),
        "多行属性按起始行报：{:?}",
        violations[0]
    );
}

#[test]
fn allows_instrument_with_skip_all() {
    let source = "#[instrument(skip_all, fields(turn = %id))]\nfn run() {}\n";
    assert!(scan_source("crates/gqy-engine/src/run.rs", source).is_empty());
}

#[test]
fn real_repo_is_clean() {
    // 仓库现状必须是干净的：任何真实代码里的被禁模式都会在这里变红。
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 的上一级是仓库根");
    let (_files, violations) = scan_repo(root).expect("扫描真实仓库");
    assert!(violations.is_empty(), "{violations:?}");
}
