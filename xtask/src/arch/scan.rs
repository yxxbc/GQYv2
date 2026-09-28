//! 静态规则扫描：`task_local!`、可变全局 `static`、`#[instrument]` 缺 `skip_all`（19 §6.1 第 5 项）。
//!
//! 规则出处：铁律 3 与 02 §4（上下文显式，不用 `task_local!` 传业务上下文）、02 §8（禁止可变全局状态）、
//! 19 §3.1（span 必须 `skip_all`）。首版取舍（P00-02「风险与回退」）：字符串/注释里出现同样会报
//! （宁可多报；遇到时先改写那一行，不放宽规则）；`static` 的类型跨行写时不报（少见，等真出现再补）。
//! 扫描范围：`crates/*/src/**/*.rs` 与 `apps/**/*.rs`；`xtask/` 自身跳过——它的常量名单里就有这些模式。
//! 创建：AI 助手（Cline 会话），2026-09-28 06:31:10。

use std::fs;
use std::path::{Path, PathBuf};

use super::Violation;

/// 扫描一段源码，返回违规（行号从 1 起）。
///
/// `path` 只用于组装位置信息，测试可以直接传假路径。首版逐行扫描、不解析语法：
/// 多行 `static` 声明与 `#[instrument]` 之外的多行属性不在范围内。
pub fn scan_source(path: &str, source: &str) -> Vec<Violation> {
    let mut violations = Vec::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut index = 0;

    while let Some(&line) = lines.get(index) {
        let line_no = index + 1;

        if line.contains("task_local!") {
            violations.push(Violation::new(
                "task-local",
                format!("{path}:{line_no}"),
                "不用 task_local! 传业务上下文（铁律 3、02 §4）；上下文走显式参数 TurnContext",
                line.trim().to_string(),
            ));
        }

        if is_mutable_global(line) {
            violations.push(Violation::new(
                "mutable-static",
                format!("{path}:{line_no}"),
                "不做可变全局状态（02 §8）；只读值用 OnceLock",
                line.trim().to_string(),
            ));
        }

        if let Some(start) = instrument_attr_start(line) {
            let mut text = String::from(&line[start..]);
            while !text.contains(']') {
                let Some(next) = lines.get(index + 1) else {
                    break;
                };
                index += 1;
                text.push(' ');
                text.push_str(next);
            }
            if !text.contains("skip_all") {
                violations.push(Violation::new(
                    "instrument-skip-all",
                    format!("{path}:{line_no}"),
                    "#[instrument] 必须带 skip_all（19 §3.1）",
                    text.trim().to_string(),
                ));
            }
        }

        index += 1;
    }

    violations
}

/// 扫描真实仓库：`crates/*/src/**/*.rs` 与 `apps/**/*.rs`（`xtask/` 跳过），
/// 返回（扫描的文件数, 违规列表）。
///
/// # Errors
///
/// 读目录或读文件失败时返回错误（带路径）。
pub fn scan_repo(root: &Path) -> anyhow::Result<(usize, Vec<Violation>)> {
    let mut files = Vec::new();

    let crates_dir = root.join("crates");
    if crates_dir.is_dir() {
        for entry in fs::read_dir(&crates_dir)? {
            let entry = entry?;
            let src = entry.path().join("src");
            if src.is_dir() {
                collect_rs_files(&src, &mut files)?;
            }
        }
    }
    let apps_dir = root.join("apps");
    if apps_dir.is_dir() {
        collect_rs_files(&apps_dir, &mut files)?;
    }
    files.sort();

    let mut violations = Vec::new();
    for file in &files {
        let source = fs::read_to_string(file)
            .map_err(|err| anyhow::anyhow!("读 {} 失败：{err}", file.display()))?;
        let display = file
            .strip_prefix(root)
            .unwrap_or(file)
            .to_string_lossy()
            .replace('\\', "/");
        violations.extend(scan_source(&display, &source));
    }
    Ok((files.len(), violations))
}

/// 递归收集 `.rs` 文件（跳过 `target` 与隐藏目录）。
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name == "target" || name.starts_with('.') {
                continue;
            }
            collect_rs_files(&path, out)?;
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// 行首是不是可变全局 `static` 声明（`OnceLock` 只读值不算，见 P00-02 范围）。
fn is_mutable_global(line: &str) -> bool {
    let trimmed = line.trim_start();
    let is_static_decl = trimmed.starts_with("static ")
        || (trimmed.starts_with("pub") && trimmed.contains(" static "));
    is_static_decl
        && (trimmed.contains("Mutex") || trimmed.contains("RwLock") || trimmed.contains("RefCell"))
        && !trimmed.contains("OnceLock")
}

/// 找到 `#[instrument]` / `#[tracing::instrument]` 属性的起点；后面必须是 `(` 或 `]`，
/// 避免命中 `#[instrumented]` 这类别的标识符。
fn instrument_attr_start(line: &str) -> Option<usize> {
    for needle in ["#[instrument", "#[tracing::instrument"] {
        if let Some(pos) = line.find(needle) {
            let rest = &line[pos + needle.len()..];
            if rest.is_empty() || rest.starts_with('(') || rest.starts_with(']') {
                return Some(pos);
            }
        }
    }
    None
}
