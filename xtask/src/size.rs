//! `cargo xtask size`：文件体积门禁（00 §3；19 §6.1 第 6 项）。
//!
//! 分档（00 §3）：`.rs` / `.ts` / `.css` 文件 ≤ 800 行通过；超过 800 警告（超目标）；
//! 超过 1500 警告 + 提示需在施工单写明拆分计划（超上限）；超过 2000 红（CI 拦下）。
//! 扫描范围：仓库内所有 `.rs` / `.ts` / `.css`；跳过隐藏目录（含 `.worktrees/`，避免把别的
//! 工作树算进来）、`target/` 与 `node_modules/`。行数按字节数 `\n`（末行没有换行符时补 1）：
//! 与编辑器显示一致，也不受编码影响（非 UTF-8 文件不会让工具报错）。
//! 输出与退出码沿用 `cargo xtask arch` 的约定：通过 0、有红 1、工具自身出错 2。
//! 创建：AI 助手（Cline 会话），2026-09-28 22:21:06。

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;

use crate::arch::find_repo_root;

/// 目标行数：超过它就警告（00 §3）。
pub const WARN_LINES: usize = 800;
/// 上限行数：超过它要在施工单里写明拆分计划（00 §3）。
pub const EXPLAIN_LINES: usize = 1500;
/// 红线行数：超过它门禁报红（00 §3）。
pub const FAIL_LINES: usize = 2000;

/// 超限严重度。`Ord` 的顺序（Warn < Explain < Fail）就是报告的排序依据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// 超过目标：建议拆分。
    Warn,
    /// 超过上限：需在施工单写明拆分计划。
    Explain,
    /// 超过红线：必须拆分。
    Fail,
}

impl Severity {
    /// 按行数定档；返回 `None` 表示通过。
    pub fn of(lines: usize) -> Option<Self> {
        if lines > FAIL_LINES {
            Some(Self::Fail)
        } else if lines > EXPLAIN_LINES {
            Some(Self::Explain)
        } else if lines > WARN_LINES {
            Some(Self::Warn)
        } else {
            None
        }
    }

    /// 报告里的档位标签。
    pub fn label(self) -> &'static str {
        match self {
            Self::Warn => "警告",
            Self::Explain => "需说明",
            Self::Fail => "红",
        }
    }

    /// 该档的完整说明（打印给人看，出处 00 §3）。
    pub fn note(self) -> &'static str {
        match self {
            Self::Warn => "超过 800 行目标，建议拆分",
            Self::Explain => "超过 1500 行上限，需在施工单写明拆分计划",
            Self::Fail => "超过 2000 行红线，必须拆分",
        }
    }
}

/// 一条体积发现：哪个文件、多少行、多严重。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SizeFinding {
    /// 相对仓库根的路径（`/` 分隔，跨平台一致）。
    pub path: String,
    /// 文件行数。
    pub lines: usize,
    /// 严重度（00 §3 的三档）。
    pub severity: Severity,
}

/// `cargo xtask size` 的参数。
#[derive(Debug, Args)]
pub struct SizeArgs {
    /// 仓库根目录（默认：从当前目录向上找到同时含 `Cargo.toml` 与图纸的目录）
    #[arg(long)]
    pub root: Option<PathBuf>,
}

/// 扫描仓库，返回按严重度降序、同行数降序、再按路径升序排列的发现。
///
/// # Errors
///
/// 读目录或读文件失败时返回错误（带路径）；这些属于“工具自身出错”，由 `main` 统一退 2。
pub fn check_size(root: &Path) -> anyhow::Result<Vec<SizeFinding>> {
    let mut files = Vec::new();
    collect_code_files(root, root, &mut files)?;
    files.sort();

    let mut findings = Vec::new();
    for (relative, absolute) in &files {
        let lines = count_lines(absolute)?;
        if let Some(severity) = Severity::of(lines) {
            findings.push(SizeFinding {
                path: relative.clone(),
                lines,
                severity,
            });
        }
    }
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| b.lines.cmp(&a.lines))
            .then_with(|| a.path.cmp(&b.path))
    });
    Ok(findings)
}

/// 跑体积门禁并打印报告：有“红”退 1；只有警告退 0（警告的说明义务在 PR 里处理）。
///
/// # Errors
///
/// 定位仓库根或读文件失败时返回错误（工具自身出错，由 `main` 统一退 2）。
pub fn run(args: &SizeArgs) -> anyhow::Result<ExitCode> {
    let root = match &args.root {
        Some(root) => root.clone(),
        None => find_repo_root(Path::new("."))?,
    };
    let findings = check_size(&root)?;
    let failed = findings
        .iter()
        .any(|finding| finding.severity == Severity::Fail);

    if findings.is_empty() {
        println!("════ cargo xtask size ════");
        println!("体积：阈值 {WARN_LINES} / {EXPLAIN_LINES} / {FAIL_LINES} 行（00 §3）");
        println!("✔ 全部通过");
        return Ok(ExitCode::SUCCESS);
    }

    let headline = if failed {
        "：有文件超过红线"
    } else {
        ""
    };
    println!(
        "════ cargo xtask size{headline}：{} 处超限 ════",
        findings.len()
    );
    for finding in &findings {
        println!(
            "{}  {} {} 行：{}（00 §3）",
            finding.severity.label(),
            finding.path,
            finding.lines,
            finding.severity.note()
        );
    }
    if failed {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

/// 递归收集 `.rs` / `.ts` / `.css` 文件，输出（相对仓库根路径, 绝对路径）。
fn collect_code_files(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, PathBuf)>,
) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            collect_code_files(root, &path, out)?;
        } else if is_code_file(&name) {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((relative, path));
        }
    }
    Ok(())
}

/// 是不是体积规则管的文件：`.rs` / `.ts` / `.css`（00 §3）。
fn is_code_file(name: &str) -> bool {
    [".rs", ".ts", ".css"]
        .iter()
        .any(|extension| name.ends_with(extension))
}

/// 数行数：按 `\n` 计，末行没有换行符时也算一行。按字节读，不看编码。
fn count_lines(path: &Path) -> anyhow::Result<usize> {
    let bytes =
        fs::read(path).map_err(|err| anyhow::anyhow!("读 {} 失败：{err}", path.display()))?;
    let mut lines = bytes.iter().filter(|byte| **byte == b'\n').count();
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        lines += 1;
    }
    Ok(lines)
}
