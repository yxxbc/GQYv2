//! `cargo xtask test`：跑测试、解析 libtest 输出、打印摘要、写报告、跑测试计数门禁（19 §5；P00-04）。
//!
//! 行为（19 §5.1）：跑 `cargo test --no-fail-fast`，逐行透传并解析；打印可读摘要（19 §5.2 版式）、
//! 写 `target/gqy-test-report/{report.json,report.md}`；跑测试计数门禁（19 §6.1 第 11 项）。
//! 退出码：通过 0；任一失败或门禁不过 1；解析失败 2（报告指出无法识别的行）。
//! 形态：`cargo xtask test [--workspace | -p <crate>] [--filter <pat>] [--include-ignored[=<tag>]]
//! [--unit] [--no-count-gate]`；`--include-ignored=<tag>` 的映射机制见 19 §4.1 与 `ignore_tags`。
//! 逐用例耗时：stable 的 libtest 不提供（见施工单实施记录），“最慢”按 suite 粒度。
//! 创建：AI 助手（Cline 会话），2026-09-28 22:42:25。

pub mod count_gate;
pub mod ignore_tags;
pub mod parse;
pub mod report;

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::time::Instant;

use clap::Args;

use crate::arch::find_repo_root;

/// `cargo xtask test` 的参数。
#[derive(Debug, Args)]
pub struct TestArgs {
    /// 跑整个 workspace（默认就是它；显式给出时与 `-p` 互斥）
    #[arg(long, conflicts_with = "package")]
    pub workspace: bool,
    /// 只跑指定包（可多次）
    #[arg(short = 'p', long = "package")]
    pub package: Vec<String>,
    /// 传给 libtest 的过滤串（`cargo test -- <filter>`）
    #[arg(long)]
    pub filter: Option<String>,
    /// `--include-ignored` 全跑 ignored；`--include-ignored=<tag>` 只跑该标签与常规用例（19 §4.1）
    #[arg(long, num_args = 0..=1, require_equals = true, value_name = "TAG")]
    pub include_ignored: Option<Option<String>>,
    /// 只跑单元测试（`--lib --bins`；Windows 作业用，19 §8.2）
    #[arg(long)]
    pub unit: bool,
    /// 跳过测试计数门禁（19 §6.1 第 11 项）
    #[arg(long)]
    pub no_count_gate: bool,
    /// 仓库根目录（默认：从当前目录向上找到同时含 `Cargo.toml` 与图纸的目录）
    #[arg(long)]
    pub root: Option<PathBuf>,
}

/// 跑测试并产出摘要、报告与退出码；退出码语义：0 通过、1 失败或门禁不过、2 解析失败。
///
/// # Errors
///
/// 定位仓库根、读基线、启动 `cargo` 子进程、写报告失败时返回错误（工具自身出错，由 `main` 退 2）。
pub fn run(args: &TestArgs) -> anyhow::Result<ExitCode> {
    let root = match &args.root {
        Some(root) => root.clone(),
        None => find_repo_root(Path::new("."))?,
    };
    let started_at = report::now_local();
    let started = Instant::now();

    // 标签筛选计划：只有 `--include-ignored=<tag>` 需要扫描源码建立映射（19 §4.1）。
    let plan = match args.include_ignored.as_ref().and_then(|tag| tag.as_deref()) {
        Some(tag) => Some(prepare_tag_filter(&root, tag)?),
        None => None,
    };
    if let Some(plan) = &plan {
        for warning in &plan.warnings {
            eprintln!("警告：{warning}");
        }
        eprintln!(
            "══ 标签筛选 ══ 只跑 needs:{tag} 标记的 ignored 用例与全部常规用例",
            tag = plan.tag
        );
    }

    // 跑测试（通常一次；名字冲突时多次），逐行透传到终端并收集。
    let mut all_lines = Vec::new();
    for command in build_commands(&root, args, plan.as_ref()) {
        let (lines, _status) = run_capturing(command)?;
        all_lines.extend(lines);
    }
    let elapsed_ms = started.elapsed().as_millis() as u64;

    // 解析；解析失败时仍写报告（指出无法识别的行，19 §5.1）。
    let (mut parsed, parse_error) = match parse::parse_stream(&all_lines) {
        parse::ParseOutcome::Parsed(parsed) => (parsed, None),
        parse::ParseOutcome::Unrecognized { line, at } => {
            (parse::Parsed::default(), Some((line, at)))
        }
    };

    // 跳过原因：从源码扫描结果补（libtest 自己不输出理由）。
    if let Some(plan) = &plan {
        for skipped in &mut parsed.skipped {
            if let Some(case) = plan
                .cases
                .iter()
                .find(|case| skipped.name.ends_with(&case.function))
            {
                skipped.reason = Some(format!("needs:{}", case.tag));
            }
        }
    }

    let mut gates = Vec::new();
    if let Some((line, at)) = &parse_error {
        gates.push(report::GateResult {
            name: "parse".to_string(),
            ok: false,
            detail: format!("无法识别测试输出（第 {at} 行：{line}）；可能编译失败，见上方原始输出"),
        });
    }
    // 计数门禁只对「整个 workspace 的全量运行」有意义；`-p` / `--filter` / `--unit` 是子集，跳过并说明。
    let subset_run = !args.package.is_empty() || args.filter.is_some() || args.unit;
    if args.no_count_gate {
        gates.push(report::GateResult {
            name: "test-count".to_string(),
            ok: true,
            detail: "已用 --no-count-gate 跳过（19 §6.1 第 11 项）".to_string(),
        });
    } else if subset_run {
        gates.push(report::GateResult {
            name: "test-count".to_string(),
            ok: true,
            detail:
                "子集运行（-p / --filter / --unit），计数门禁不适用（基线按整个 workspace 记录）"
                    .to_string(),
        });
    } else {
        gates.push(run_count_gate(&root, &parsed)?);
    }
    let gate_failed = gates.iter().any(|gate| !gate.ok);

    let meta = report::RunMeta {
        started_at,
        elapsed_ms,
        commit: report::current_commit(&root),
        toolchain: report::toolchain_version(),
    };
    let test_report =
        report::TestReport::assemble(meta, parsed.suites, parsed.failures, parsed.skipped, gates);
    print_summary(&test_report);
    let (json_path, md_path) = report::write_reports(&root, &test_report)?;
    println!(
        "报告：{}、{}",
        display_path(&root, &json_path),
        display_path(&root, &md_path)
    );

    if parse_error.is_some() {
        return Ok(ExitCode::from(2));
    }
    if test_report.totals.failed > 0 || gate_failed {
        return Ok(ExitCode::from(1));
    }
    Ok(ExitCode::SUCCESS)
}

/// 标签筛选计划（`--include-ignored=<tag>`，19 §4.1）。
struct TagFilterPlan {
    /// 目标标签。
    tag: String,
    /// 源码扫描出的全部带标签用例（补跳过原因用）。
    cases: Vec<ignore_tags::IgnoredCase>,
    /// 传给 libtest 的 `--skip` 名单（名字冲突时只列不冲突的那些）。
    skip: Vec<String>,
    /// 名字冲突回退：为真时用「常规一次 + 目标过滤一次」的两次运行。
    conflict_fallback: bool,
    /// 扫描告警与冲突提示（打印给作者）。
    warnings: Vec<String>,
}

/// 扫描测试源码、算出筛选计划。
///
/// # Errors
///
/// 遍历或读测试源码失败时返回错误。
fn prepare_tag_filter(root: &Path, tag: &str) -> anyhow::Result<TagFilterPlan> {
    let sources = read_test_sources(root)?;
    let map = ignore_tags::scan_ignore_tags(&sources);
    let mut warnings = map.warnings.clone();
    let (run, skip) = ignore_tags::partition_by_tag(&map.cases, tag);
    if map.cases.is_empty() {
        warnings.push(format!(
            "没有找到任何 needs:<tag> 用例；--include-ignored={tag} 只会跑常规用例"
        ));
    }
    let conflicts = ignore_tags::find_name_conflicts(&run, &skip);
    for (run_name, skip_name) in &conflicts {
        warnings.push(format!(
            "用例名子串冲突：{run_name} 与 {skip_name}；libtest 的 --skip 是子串匹配，已回退为逐用例运行，请改名避免误跑/误跳"
        ));
    }
    let conflict_fallback = !conflicts.is_empty();
    // 冲突时不 skip 与目标构成子串关系的用例（它们会误跑，但在输出里可见，提示作者改名）。
    let kept_skip: Vec<ignore_tags::IgnoredCase> = if conflict_fallback {
        skip.iter()
            .filter(|case| !conflicts.iter().any(|(_, other)| other == &case.function))
            .cloned()
            .collect()
    } else {
        skip.clone()
    };
    Ok(TagFilterPlan {
        tag: tag.to_string(),
        cases: map.cases,
        skip: kept_skip.iter().map(|case| case.function.clone()).collect(),
        conflict_fallback,
        warnings,
    })
}

/// 收集测试源码：`crates/`、`apps/`、`xtask/`、根 `tests/` 下的 `.rs`（相对路径, 内容）。
///
/// # Errors
///
/// 遍历目录或读文件失败时返回错误（带路径）。
fn read_test_sources(root: &Path) -> anyhow::Result<Vec<(String, String)>> {
    let mut files = Vec::new();
    for dir in ["crates", "apps", "xtask", "tests"] {
        let path = root.join(dir);
        if path.is_dir() {
            collect_rs_files(&path, &mut files)?;
        }
    }
    files.sort();
    let mut sources = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path)
            .map_err(|err| anyhow::anyhow!("读 {} 失败：{err}", path.display()))?;
        sources.push((display_path(root, &path), text));
    }
    Ok(sources)
}

/// 递归收集 `.rs` 文件（跳过隐藏目录、`target/`、`node_modules/`）。
fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            collect_rs_files(&path, out)?;
        } else if name.ends_with(".rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// 组装要跑的 `cargo test` 命令序列（通常一条；名字冲突回退时两条）。
fn build_commands(root: &Path, args: &TestArgs, plan: Option<&TagFilterPlan>) -> Vec<Command> {
    match plan {
        Some(plan) if plan.conflict_fallback => {
            // 回退：常规用例一次（ignored 自然跳过）；目标 ignored 用例一次（正向过滤，避免子串误跳）。
            let (run, _) = ignore_tags::partition_by_tag(&plan.cases, &plan.tag);
            let targets: Vec<String> = run.iter().map(|case| case.function.clone()).collect();
            vec![
                build_command(root, args, false, &[], &[]),
                build_command(root, args, true, &targets, &plan.skip),
            ]
        }
        Some(plan) => vec![build_command(root, args, true, &[], &plan.skip)],
        None => {
            let include_all_ignored = args.include_ignored.is_some();
            vec![build_command(root, args, include_all_ignored, &[], &[])]
        }
    }
}

/// 组装一条 `cargo test` 命令：`include_ignored` 加 `--include-ignored`；
/// `filters` 是正向过滤串（libtest 的位置参数）；`skip` 是 `--skip` 名单。
fn build_command(
    root: &Path,
    args: &TestArgs,
    include_ignored: bool,
    filters: &[String],
    skip: &[String],
) -> Command {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command.current_dir(root).arg("test");
    if args.package.is_empty() {
        command.arg("--workspace");
    } else {
        for package in &args.package {
            command.args(["-p", package]);
        }
    }
    command.arg("--no-fail-fast");
    if args.unit {
        command.args(["--lib", "--bins"]);
    }

    let mut libtest: Vec<String> = Vec::new();
    if let Some(filter) = &args.filter {
        libtest.push(filter.clone());
    }
    libtest.extend(filters.iter().cloned());
    if include_ignored {
        libtest.push("--include-ignored".to_string());
    }
    for name in skip {
        libtest.push("--skip".to_string());
        libtest.push(name.clone());
    }
    if !libtest.is_empty() {
        command.arg("--");
        command.args(&libtest);
    }
    command
}

/// 跑一条命令：逐行透传到终端并收集，返回（行序列, 退出状态）。
///
/// # Errors
///
/// 启动子进程、取管道、读线程或等待进程失败时返回错误。
fn run_capturing(mut command: Command) -> anyhow::Result<(Vec<String>, std::process::ExitStatus)> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| anyhow::anyhow!("启动 cargo test 失败：{err}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("拿不到子进程的 stdout"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("拿不到子进程的 stderr"))?;

    let (sender, receiver) = std::sync::mpsc::channel::<String>();
    let sender_out = sender.clone();
    let reader_out = std::thread::spawn(move || -> std::io::Result<()> {
        for line in BufReader::new(stdout).lines() {
            let text = line?;
            if sender_out.send(text).is_err() {
                break;
            }
        }
        Ok(())
    });
    let reader_err = std::thread::spawn(move || -> std::io::Result<()> {
        for line in BufReader::new(stderr).lines() {
            let text = line?;
            if sender.send(text).is_err() {
                break;
            }
        }
        Ok(())
    });

    let mut lines = Vec::new();
    for line in receiver {
        println!("{line}");
        lines.push(line);
    }
    let status = child
        .wait()
        .map_err(|err| anyhow::anyhow!("等待 cargo test 失败：{err}"))?;
    for handle in [reader_out, reader_err] {
        handle
            .join()
            .map_err(|_| anyhow::anyhow!("读测试输出的线程失败"))?
            .map_err(|err| anyhow::anyhow!("读测试输出失败：{err}"))?;
    }
    Ok((lines, status))
}

/// 跑测试计数门禁（19 §6.1 第 11 项）：读平台基线、比较执行数，产出报告里的一条门禁结果。
///
/// # Errors
///
/// 读基线失败（缺失或格式不对）时返回错误（工具自身出错，由 `main` 退 2）。
fn run_count_gate(root: &Path, parsed: &parse::Parsed) -> anyhow::Result<report::GateResult> {
    let baseline = count_gate::read_baseline(root)?;
    let executed = report::Totals::from_suites(&parsed.suites).executed;
    let gate = match count_gate::check_count_gate(executed, baseline) {
        Ok(count_gate::GateVerdict::Equal) => report::GateResult {
            name: "test-count".to_string(),
            ok: true,
            detail: format!("执行数 {executed} = 基线 {baseline}"),
        },
        Ok(count_gate::GateVerdict::Higher { executed, baseline }) => report::GateResult {
            name: "test-count".to_string(),
            ok: true,
            detail: format!("执行数 {executed} > 基线 {baseline}（高于基线，请在本提交更新基线）"),
        },
        Err(violation) => report::GateResult {
            name: "test-count".to_string(),
            ok: false,
            detail: format!("{}；{}", violation.expected, violation.actual),
        },
    };
    Ok(gate)
}

/// 打印摘要（19 §5.2 的版式；期望/实际直接取自断言原文，解析器不猜测）。
fn print_summary(test_report: &report::TestReport) {
    println!(
        "══ 测试报告 ══ {}  commit {}  平台 {}  用时 {:.1}s",
        test_report.started_at,
        test_report.commit.as_deref().unwrap_or("（未知）"),
        test_report.platform,
        test_report.elapsed_ms as f64 / 1000.0
    );
    println!(
        "{:<30}{:>6}{:>6}{:>6}{:>9}",
        "suite", "通过", "失败", "跳过", "用时"
    );
    for suite in &test_report.suites {
        println!(
            "{:<30}{:>6}{:>6}{:>6}{:>8.1}s",
            suite.name,
            suite.passed,
            suite.failed,
            suite.ignored,
            suite.elapsed_ms as f64 / 1000.0
        );
    }
    println!(
        "{:<30}{:>6}{:>6}{:>6}",
        "合计", test_report.totals.passed, test_report.totals.failed, test_report.totals.ignored
    );

    if !test_report.failures.is_empty() {
        println!("── 失败 ──");
        for (index, failure) in test_report.failures.iter().enumerate() {
            println!("{}) {}", index + 1, failure.name);
            let location = match (&failure.file, failure.line) {
                (Some(file), Some(line)) => format!("{file}:{line}"),
                (Some(file), None) => file.clone(),
                _ => "（未知）".to_string(),
            };
            println!("   位置  {location}");
            println!("   复跑  {}", failure.rerun);
            for line in failure.message.lines().take(6) {
                println!("   输出  {line}");
            }
        }
    }

    let mut slowest: Vec<&parse::SuiteResult> = test_report.suites.iter().collect();
    slowest.sort_by_key(|suite| std::cmp::Reverse(suite.elapsed_ms));
    let top: Vec<String> = slowest
        .iter()
        .take(5)
        .map(|suite| format!("{} {:.1}s", suite.name, suite.elapsed_ms as f64 / 1000.0))
        .collect();
    if !top.is_empty() {
        println!("── 最慢 5 个 suite ──  {}", top.join("，"));
    }

    if !test_report.skipped.is_empty() {
        let mut by_reason: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for skipped in &test_report.skipped {
            let key = skipped
                .reason
                .clone()
                .unwrap_or_else(|| "（源码未标注理由）".to_string());
            *by_reason.entry(key).or_insert(0) += 1;
        }
        let summary: Vec<String> = by_reason
            .iter()
            .map(|(reason, count)| format!("{reason} ×{count}"))
            .collect();
        println!(
            "── 跳过 ──  共 {} 个：{}",
            test_report.skipped.len(),
            summary.join("，")
        );
    }

    println!("── 门禁 ──");
    for gate in &test_report.gates {
        let mark = if gate.ok { "✔" } else { "✘" };
        println!("   {mark} {}：{}", gate.name, gate.detail);
    }
}

/// 相对仓库根的展示路径（`/` 分隔；不在根下时原样）。
fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
