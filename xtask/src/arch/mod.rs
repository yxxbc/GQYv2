//! `cargo xtask arch`：层序门禁（01 §3）与静态规则扫描（19 §6.1 第 5 项）。
//!
//! 分工：`layers` 解析图纸并核对 `cargo metadata` 的依赖图；`scan` 扫源码里的被禁模式；
//! 本模块只做编排、报告与退出码。**不设白名单**：允许什么由 01 §3 的表格说了算，违规即红（铁律 4）。
//! 退出码（与仓库其他检查一致）：通过 0、有违规 1、工具自身出错 2。
//! 创建：AI 助手（Cline 会话），2026-09-28 06:31:10。

pub mod layers;
pub mod scan;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use clap::Args;

/// 检查结果里的一条违规：规则名、位置、期望值与实际值（19 §4.1：失败输出要能定位）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// 规则名，例如 `layer-order` / `unknown-package` / `heavy-dep` / `task-local`。
    pub rule: &'static str,
    /// 位置：`gqy-core -> gqy-store` 或 `crates/gqy-core/src/lib.rs:12`。
    pub location: String,
    /// 期望值（图纸或规则怎么说）。
    pub expected: String,
    /// 实际值（现在是什么）。
    pub actual: String,
}

impl Violation {
    /// 组装一条违规；规则名用静态字符串，测试按 `rule` 筛选。
    pub fn new(
        rule: &'static str,
        location: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            rule,
            location: location.into(),
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

/// 层表的真相源：图纸路径（相对仓库根）。解析器读它，不另存副本。
pub const LAYER_DOC: &str = "docs/designs/01-总体架构.md";

/// `cargo xtask arch` 的参数。
#[derive(Debug, Args)]
pub struct ArchArgs {
    /// 仓库根目录（默认：从当前目录向上找到同时含 `Cargo.toml` 与图纸的目录）
    #[arg(long)]
    pub root: Option<PathBuf>,
}

/// 跑两类检查并打印报告：有违规退 1，全通过退 0，工具错误（读不到图纸、`cargo metadata` 失败）向上抛。
///
/// # Errors
///
/// 找不到仓库根、读不到图纸、图纸解析失败、`cargo metadata` 失败或输出不是 JSON 时返回错误；
/// 这些都属于“工具自身出错”，由 `main` 统一退 2。
pub fn run(args: &ArchArgs) -> anyhow::Result<ExitCode> {
    let root = match &args.root {
        Some(root) => root.clone(),
        None => find_repo_root(Path::new("."))?,
    };
    let doc = std::fs::read_to_string(root.join(LAYER_DOC))?;
    let table =
        layers::parse_layers(&doc).map_err(|err| anyhow::anyhow!("{LAYER_DOC} 解析失败：{err}"))?;
    let metadata = load_metadata(&root)?;
    let package_count = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let mut violations = layers::check_arch(&metadata, &table);
    let (scanned_files, scan_violations) = scan::scan_repo(&root)?;
    violations.extend(scan_violations);

    if violations.is_empty() {
        println!("════ cargo xtask arch ════");
        println!(
            "层序：层表 {} 条目标，workspace {} 个包（xtask 是开发工具，按规则豁免）",
            table.entry_count(),
            package_count
        );
        println!(
            "静态规则：扫描 {scanned_files} 个文件（task_local! / 可变全局 static / #[instrument] 缺 skip_all）"
        );
        println!("✔ 全部通过");
        return Ok(ExitCode::SUCCESS);
    }

    println!("════ cargo xtask arch：{} 条违规 ════", violations.len());
    for violation in &violations {
        println!("违规 {}  {}", violation.rule, violation.location);
        println!("  期望 {}", violation.expected);
        println!("  实际 {}", violation.actual);
    }
    Ok(ExitCode::from(1))
}

/// 从 `start` 向上找仓库根：同时有 `Cargo.toml` 与图纸目录。
///
/// # Errors
///
/// 一直找到文件系统根都没有这两个文件时返回错误。
pub fn find_repo_root(start: &Path) -> anyhow::Result<PathBuf> {
    let start = std::fs::canonicalize(start)?;
    for dir in start.ancestors() {
        if dir.join("Cargo.toml").is_file() && dir.join(LAYER_DOC).is_file() {
            return Ok(dir.to_path_buf());
        }
    }
    anyhow::bail!(
        "从 {} 向上找不到仓库根（需要同时有 Cargo.toml 与 {LAYER_DOC}）",
        start.display()
    )
}

/// 跑 `cargo metadata --no-deps` 并解析 JSON：只列 workspace 成员与它们声明的依赖，离线可用。
///
/// # Errors
///
/// 起不了 `cargo` 子进程、退出码非 0、或输出不是合法 JSON 时返回错误（带 stderr 内容）。
pub fn load_metadata(root: &Path) -> anyhow::Result<serde_json::Value> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let output = Command::new(cargo)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|err| anyhow::anyhow!("跑 cargo metadata 失败（{}）：{err}", root.display()))?;
    if !output.status.success() {
        anyhow::bail!(
            "cargo metadata 退出码 {}：{}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|err| anyhow::anyhow!("cargo metadata 输出不是合法 JSON：{err}"))
}
