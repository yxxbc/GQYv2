//! 层序门禁的 fixture：文档解析样例 + mini workspace 的依赖图核对（P00-02「测试与守护」）。
//!
//! 四类层序用例：依赖方向违规、附加约束违规、未知包、合规图；另加解析器的三份样本。
//! 创建：AI 助手（Cline 会话），2026-09-28 06:31:10。
//! 更新：AI 助手（Cline 会话），2026-09-28 22:21:06（P00-03 集成测试放开）。

// 集成测试整文件放开（19 §6.1；P00-03）：测试里允许 unwrap/expect/panic/索引。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::fs;
use std::path::{Path, PathBuf};

use xtask::arch::layers::{LayerTable, check_arch, parse_layers};
use xtask::arch::load_metadata;

/// 仓库根（`xtask/` 的上一级）；测试从任意工作目录运行时都能定位文档与 workspace。
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask 的上一级是仓库根")
        .to_path_buf()
}

/// 文档即真相源：解析仓库里真实的 `docs/designs/01-总体架构.md` §3。
fn real_table() -> LayerTable {
    let doc = fs::read_to_string(repo_root().join("docs/designs/01-总体架构.md"))
        .expect("读 01-总体架构.md");
    parse_layers(&doc).expect("01 §3 的分层表应当可解析")
}

/// 在临时目录里建一个 mini workspace：`members` 为 (包名, 工作区依赖)。
///
/// 依赖图全部用 `path` 依赖，不需要网络；`[workspace]` 显式声明，避免 cargo 往上找父 workspace。
fn write_mini_workspace(dir: &Path, members: &[(&str, &[&str])]) {
    let mut manifest = String::from("[workspace]\nresolver = \"2\"\nmembers = [\n");
    for (name, _) in members {
        manifest.push_str(&format!("  \"{name}\",\n"));
    }
    manifest.push_str("]\n\n[workspace.package]\nversion = \"0.0.0\"\nedition = \"2021\"\n");
    fs::write(dir.join("Cargo.toml"), manifest).expect("写 workspace 清单");

    for (name, deps) in members {
        let crate_dir = dir.join(name);
        fs::create_dir_all(crate_dir.join("src")).expect("建 crate 目录");
        let mut manifest = format!(
            "[package]\nname = \"{name}\"\nversion.workspace = true\nedition.workspace = true\n"
        );
        if !deps.is_empty() {
            manifest.push_str("\n[dependencies]\n");
            for dep in *deps {
                manifest.push_str(&format!("{dep} = {{ path = \"../{dep}\" }}\n"));
            }
        }
        fs::write(crate_dir.join("Cargo.toml"), manifest).expect("写 crate 清单");
        fs::write(crate_dir.join("src/lib.rs"), format!("//! {name}\n")).expect("写 lib.rs");
    }
}

/// 用真实层表核对一个 mini workspace 的依赖图。
fn check_mini(members: &[(&str, &[&str])]) -> Vec<xtask::arch::Violation> {
    let tmp = tempfile::tempdir().expect("建临时目录");
    write_mini_workspace(tmp.path(), members);
    let meta = load_metadata(tmp.path()).expect("cargo metadata 应成功");
    check_arch(&meta, &real_table())
}

#[test]
fn flags_layer_order_violation() {
    // 低层包依赖高层包：gqy-core（L0）→ gqy-store（L1）。
    let violations = check_mini(&[("gqy-core", &["gqy-store"]), ("gqy-store", &[])]);
    let found = violations
        .iter()
        .find(|v| v.rule == "layer-order")
        .unwrap_or_else(|| panic!("应报 layer-order，实际：{violations:?}"));
    assert_eq!(found.location, "gqy-core -> gqy-store");
    assert!(
        found.expected.contains("更低层"),
        "期望值要说明只能指向更低层：{found:?}"
    );
    assert!(!found.actual.is_empty(), "实际值不能为空：{found:?}");
}

#[test]
fn flags_entry_crate_internal_dependency() {
    // 附加约束：入口不得依赖 L1–L4（gqy-tui 只能经 gqy-client / gqy-protocol 访问运行时）。
    let violations = check_mini(&[("gqy-tui", &["gqy-engine"]), ("gqy-engine", &[])]);
    assert!(
        violations.iter().any(|v| v.rule == "entry-layer"),
        "应报 entry-layer，实际：{violations:?}"
    );
}

#[test]
fn flags_unknown_package() {
    // workspace 里出现层表外的包：设计事实不允许悄悄多出 crate。
    let violations = check_mini(&[("gqy-core", &[]), ("gqy-unknown", &[])]);
    let found = violations
        .iter()
        .find(|v| v.rule == "unknown-package")
        .unwrap_or_else(|| panic!("应报 unknown-package，实际：{violations:?}"));
    assert!(found.location.contains("gqy-unknown"), "{found:?}");
}

#[test]
fn parses_real_layer_table() {
    let table = real_table();
    assert_eq!(table.layer_of("gqy-core"), Some(0));
    assert_eq!(table.layer_of("gqy-store"), Some(1));
    assert_eq!(table.layer_of("gqy-engine"), Some(4));
    assert_eq!(table.layer_of("gqy-connector-sdk"), Some(5));
    assert_eq!(table.layer_of("gqy-daemon"), Some(6));
    assert_eq!(table.layer_of("gqy-tui"), Some(7));
    assert_eq!(
        table.layer_of("gqy-connector-onebot"),
        Some(7),
        "连接器按前缀归属 L7"
    );

    let provider = table
        .entry_of("gqy-provider")
        .expect("gqy-provider 在层表里");
    assert!(
        provider.allowed_heavy_deps.iter().any(|d| d == "reqwest"),
        "gqy-provider 的允许重依赖应含 reqwest：{:?}",
        provider.allowed_heavy_deps
    );
}

const SAMPLE_OK: &str = "\
前置说明。

## 3 crate 分层与依赖方向

| 层 | crate | 职责 | 允许的外部重依赖 |
| --- | --- | --- | --- |
| L0 基础类型 | `gqy-core` | 类型 | serde、tokio（仅 `rt`，供 `blocking::run`） |
| L1 系统 | `gqy-sys` | FFI | libc、nix |
| L5 服务 | `gqy-gateway` | 网关 | axum、rust-embed |
| L7 入口 | `gqy-desktop`、连接器 | 宿主 | tauri 等 |

附加约束（同样由 `cargo xtask arch` 检查）：
";

#[test]
fn parses_embedded_sample() {
    let table = parse_layers(SAMPLE_OK).expect("合规样本应当可解析");
    assert_eq!(table.layer_of("gqy-core"), Some(0));
    assert_eq!(table.layer_of("gqy-sys"), Some(1));
    assert_eq!(table.layer_of("gqy-gateway"), Some(5));
    assert_eq!(table.layer_of("gqy-desktop"), Some(7));
    assert_eq!(table.layer_of("gqy-connector-qq"), Some(7));

    let core = table.entry_of("gqy-core").expect("gqy-core");
    assert!(
        core.allowed_heavy_deps.iter().any(|d| d == "tokio"),
        "括号里的说明应被剥掉：{:?}",
        core.allowed_heavy_deps
    );
}

#[test]
fn reports_missing_column_with_line_number() {
    let sample = "\
## 3 crate 分层与依赖方向

| 层 | crate | 职责 |
| --- | --- | --- |
| L0 基础类型 | `gqy-core` | 类型 |
";
    let err = parse_layers(sample).expect_err("缺列必须报错");
    assert!(err.line() >= 3, "错误要带文档行号：{err:?}");
    assert!(err.to_string().contains("列"), "错误要说明列数不对：{err}");
}

#[test]
fn reports_bad_layer_number() {
    let sample = "\
## 3 crate 分层与依赖方向

| 层 | crate | 职责 | 允许的外部重依赖 |
| --- | --- | --- | --- |
| 基础类型 | `gqy-core` | 类型 | serde |
";
    let err = parse_layers(sample).expect_err("层号缺失必须报错");
    assert!(err.line() >= 5, "错误要指向数据行：{err:?}");
}

#[test]
fn accepts_table_consistent_graph() {
    // 合规图：每一处依赖都指向更低层，附加约束也满足。
    let violations = check_mini(&[
        ("gqy-core", &[]),
        ("gqy-protocol", &[]),
        ("gqy-store", &["gqy-core"]),
        ("gqy-engine", &["gqy-store"]),
        ("gqy-gateway", &["gqy-engine"]),
        ("gqy-client", &[]),
        ("gqy-daemon", &["gqy-engine", "gqy-gateway"]),
        ("gqy-tui", &["gqy-client", "gqy-protocol"]),
    ]);
    assert!(violations.is_empty(), "合规图不应有违规：{violations:?}");
}
