//! 测试报告：JSON（schema 1）与 Markdown（19 §5.3；P00-04）。
//!
//! 报告固定写到 `<仓库根>/target/gqy-test-report/`（`report.json`、`report.md`；CI 作为构件上传，
//! 19 §5.3）。JSON 的字段与 19 §5.3 的 schema 1 对齐：schema / started_at / elapsed_ms /
//! commit / platform / toolchain / suites / totals / failures / skipped / gates。
//! `executed = passed + failed`（不含 ignored）——它同时是计数门禁的输入（19 §6.1 第 11 项）。
//! 时间用 `jiff`（00 §5 定的唯一时间库），启动时间按本地时区写成人读格式。
//! 创建：AI 助手（Cline 会话），2026-09-28 22:42:25。

use std::path::{Path, PathBuf};

use serde::Serialize;

use super::parse::{Failure, Skipped, SuiteResult};

/// 报告的 schema 版本（19 §5.3）。
pub const SCHEMA: u32 = 1;

/// 报告目录（相对仓库根）。
pub const REPORT_DIR: &str = "target/gqy-test-report";

/// 合计；`executed = passed + failed`（不含 ignored）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Totals {
    /// 通过的用例数。
    pub passed: usize,
    /// 失败的用例数。
    pub failed: usize,
    /// 跳过的用例数。
    pub ignored: usize,
    /// 实际执行数（计数门禁的输入）。
    pub executed: usize,
}

impl Totals {
    /// 从各 suite 汇总。
    pub fn from_suites(suites: &[SuiteResult]) -> Self {
        let passed = suites.iter().map(|suite| suite.passed).sum();
        let failed = suites.iter().map(|suite| suite.failed).sum();
        let ignored = suites.iter().map(|suite| suite.ignored).sum();
        Self {
            passed,
            failed,
            ignored,
            executed: passed + failed,
        }
    }
}

/// 一条门禁的结果（当前只有测试计数门禁，19 §6.1 第 11 项）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GateResult {
    /// 门禁名（如 `test-count`）。
    pub name: String,
    /// 是否通过。
    pub ok: bool,
    /// 人读说明（含期望值与实际值）。
    pub detail: String,
}

/// 测试报告本体（19 §5.3 的 schema 1）。
#[derive(Debug, Clone, Serialize)]
pub struct TestReport {
    /// schema 版本（`SCHEMA`）。
    pub schema: u32,
    /// 启动时间（本地时区，`YYYY-MM-DD HH:MM:SS`）。
    pub started_at: String,
    /// 整个 `cargo test` 运行的用时（毫秒）。
    pub elapsed_ms: u64,
    /// 当前 commit（取不到时为 `None`）。
    pub commit: Option<String>,
    /// 平台（`<os>-<arch>`，如 `macos-aarch64`）。
    pub platform: String,
    /// 工具链（`rustc -V` 的输出）。
    pub toolchain: String,
    /// 每个测试二进制一项。
    pub suites: Vec<SuiteResult>,
    /// 合计。
    pub totals: Totals,
    /// 失败用例。
    pub failures: Vec<Failure>,
    /// 被跳过的用例。
    pub skipped: Vec<Skipped>,
    /// 门禁结果。
    pub gates: Vec<GateResult>,
}

/// 一次运行的元数据（`TestReport::assemble` 的输入）。
#[derive(Debug, Clone)]
pub struct RunMeta {
    /// 启动时间（本地时区，`YYYY-MM-DD HH:MM:SS`）。
    pub started_at: String,
    /// 整个 `cargo test` 运行的用时（毫秒）。
    pub elapsed_ms: u64,
    /// 当前 commit（取不到时为 `None`）。
    pub commit: Option<String>,
    /// 工具链（`rustc -V` 的输出）。
    pub toolchain: String,
}

impl TestReport {
    /// 组装报告：`totals` 由 suites 推导，平台取当前进程的 `os-arch`，其余元数据来自 `meta`。
    pub fn assemble(
        meta: RunMeta,
        suites: Vec<SuiteResult>,
        failures: Vec<Failure>,
        skipped: Vec<Skipped>,
        gates: Vec<GateResult>,
    ) -> Self {
        Self {
            schema: SCHEMA,
            started_at: meta.started_at,
            elapsed_ms: meta.elapsed_ms,
            commit: meta.commit,
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            toolchain: meta.toolchain,
            totals: Totals::from_suites(&suites),
            suites,
            failures,
            skipped,
            gates,
        }
    }
}

/// 当前本地时间（`YYYY-MM-DD HH:MM:SS`；jiff 是唯一时间库，00 §5）。
pub fn now_local() -> String {
    let now = jiff::Zoned::now();
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

/// 仓库当前短 commit（git 不可用时为 `None`，报告照写）。
pub fn current_commit(root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// 工具链版本（`rustc -V`；取不到时为 `unknown`）。
pub fn toolchain_version() -> String {
    std::process::Command::new("rustc")
        .arg("-V")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

/// 写 `report.json` 与 `report.md`，返回（JSON 路径, Markdown 路径）。
///
/// # Errors
///
/// 建目录、序列化 JSON、写文件失败时返回错误。
pub fn write_reports(root: &Path, report: &TestReport) -> anyhow::Result<(PathBuf, PathBuf)> {
    let dir = root.join(REPORT_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|err| anyhow::anyhow!("建目录 {} 失败：{err}", dir.display()))?;

    let json_path = dir.join("report.json");
    let json = serde_json::to_string_pretty(report)
        .map_err(|err| anyhow::anyhow!("序列化报告失败：{err}"))?;
    std::fs::write(&json_path, format!("{json}\n"))
        .map_err(|err| anyhow::anyhow!("写 {} 失败：{err}", json_path.display()))?;

    let md_path = dir.join("report.md");
    std::fs::write(&md_path, render_markdown(report))
        .map_err(|err| anyhow::anyhow!("写 {} 失败：{err}", md_path.display()))?;

    Ok((json_path, md_path))
}

/// 渲染 Markdown 报告（人读版；CI 把 `report.md` 写入作业摘要，19 §5.3）。
pub fn render_markdown(report: &TestReport) -> String {
    let mut out = String::new();
    out.push_str("# 测试报告\n\n");
    out.push_str(&format!(
        "- 开始：{}（用时 {:.1}s）\n- commit：{}；平台：{}；工具链：{}\n- 合计：通过 {} / 失败 {} / 跳过 {}（执行 {} = passed + failed）\n\n",
        report.started_at,
        report.elapsed_ms as f64 / 1000.0,
        report.commit.as_deref().unwrap_or("（未知）"),
        report.platform,
        report.toolchain,
        report.totals.passed,
        report.totals.failed,
        report.totals.ignored,
        report.totals.executed,
    ));

    out.push_str("| suite | 通过 | 失败 | 跳过 | 用时 |\n| --- | --- | --- | --- | --- |\n");
    for suite in &report.suites {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {:.1}s |\n",
            suite.name,
            suite.passed,
            suite.failed,
            suite.ignored,
            suite.elapsed_ms as f64 / 1000.0
        ));
    }

    out.push_str("\n## 失败\n\n");
    if report.failures.is_empty() {
        out.push_str("（无）\n");
    } else {
        for (index, failure) in report.failures.iter().enumerate() {
            let location = match (&failure.file, failure.line) {
                (Some(file), Some(line)) => format!("{file}:{line}"),
                (Some(file), None) => file.clone(),
                _ => "（未知）".to_string(),
            };
            out.push_str(&format!(
                "{}. `{}`\n   - 位置：{location}\n   - 复跑：`{}`\n\n   ```text\n{}\n   ```\n\n",
                index + 1,
                failure.name,
                failure.rerun,
                failure.message,
            ));
        }
    }

    out.push_str("## 跳过\n\n");
    if report.skipped.is_empty() {
        out.push_str("（无）\n");
    } else {
        for skipped in &report.skipped {
            let reason = skipped.reason.as_deref().unwrap_or("（源码未标注理由）");
            out.push_str(&format!("- `{}`（{reason}）\n", skipped.name));
        }
    }

    out.push_str("\n## 门禁\n\n");
    if report.gates.is_empty() {
        out.push_str("（未运行）\n");
    } else {
        for gate in &report.gates {
            let mark = if gate.ok { "✔" } else { "✘" };
            out.push_str(&format!("- {}：{mark} {}\n", gate.name, gate.detail));
        }
    }

    out
}
