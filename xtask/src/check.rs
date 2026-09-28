//! `cargo xtask check`：把已生效的门禁组装成一个命令（19 §6.1、§8.3；P00-03）。
//!
//! 注册表式组装：检查项都在 `CHECKS` 里登记（id、名称、命令、耗时档、归属组、执行方式），
//! 新增检查只加一条，不改执行逻辑（21 §4「加东西只登记，不改中心」）。
//! 三种选择（互斥）：`--fast`（提交前快检：fmt、clippy、arch、size，19 §8.3）；
//! `--docs`（只跑 rustdoc 门禁，CI docs 作业用）；`--no-docs`（全集去掉 rustdoc，CI checks
//! 作业用）；不带参数 = 全集。
//! 全部跑完再汇总：有失败退 1，并把失败项的完整输出打出来；工具自身出错退 2（`main` 统一）。
//! 设计依据：19 §6.1（门禁清单）、§8.1（本地与 CI 同一实现）、21 §4（xtask 目录约定）。
//! 创建：AI 助手（Cline 会话），2026-09-28 22:21:06。

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use clap::Args;

use crate::arch::{self, ArchArgs, find_repo_root};
use crate::size::{self, SizeArgs};

/// 耗时/用途档：`--fast` 只收 `Fast` 档（19 §8.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// 提交前快检包含。
    Fast,
    /// 只在完整 `check` 里跑。
    Slow,
}

/// 归属组：`--docs` 只收 `Docs` 组，`--no-docs` 收非 `Docs` 组。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// 代码类检查。
    Code,
    /// rustdoc 门禁（`cargo doc`）。
    Docs,
}

/// 检查怎么执行。
#[derive(Debug, Clone, Copy)]
pub enum Runner {
    /// 外部命令（`cargo …`）：捕获输出，失败时打印。
    Command {
        /// 程序名。
        program: &'static str,
        /// 参数。
        args: &'static [&'static str],
        /// 额外环境变量。
        env: &'static [(&'static str, &'static str)],
    },
    /// 进程内调用 `cargo xtask arch` 的检查（违规明细由它自己打印）。
    Arch,
    /// 进程内调用 `cargo xtask size` 的检查。
    Size,
}

/// 一项检查的静态描述（注册表条目）。
#[derive(Debug, Clone, Copy)]
pub struct CheckSpec {
    /// 检查 id（摘要与测试用；稳定小写，全表唯一）。
    pub id: &'static str,
    /// 人读名称。
    pub title: &'static str,
    /// 可照着重跑的命令（打印用；环境变量前缀写在这里）。
    pub command: &'static str,
    /// 耗时/用途档。
    pub tier: Tier,
    /// 归属组。
    pub group: Group,
    /// 执行方式。
    pub runner: Runner,
}

/// 门禁注册表：顺序就是执行顺序（照 19 §6.1 清单第 1–6 项）。新增检查只加一行。
pub const CHECKS: &[CheckSpec] = &[
    CheckSpec {
        id: "fmt",
        title: "格式",
        command: "cargo fmt --all --check",
        tier: Tier::Fast,
        group: Group::Code,
        runner: Runner::Command {
            program: "cargo",
            args: &["fmt", "--all", "--check"],
            env: &[],
        },
    },
    CheckSpec {
        id: "clippy",
        title: "静态检查（含 workspace lints）",
        command: "cargo clippy --workspace --all-targets -- -D warnings",
        tier: Tier::Fast,
        group: Group::Code,
        runner: Runner::Command {
            program: "cargo",
            args: &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
            env: &[],
        },
    },
    CheckSpec {
        id: "doc",
        title: "rustdoc 文档完整（含私有项断链）",
        command: "RUSTDOCFLAGS=\"-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links\" cargo doc --workspace --no-deps --document-private-items",
        tier: Tier::Slow,
        group: Group::Docs,
        runner: Runner::Command {
            program: "cargo",
            args: &[
                "doc",
                "--workspace",
                "--no-deps",
                "--document-private-items",
            ],
            env: &[(
                "RUSTDOCFLAGS",
                "-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links",
            )],
        },
    },
    CheckSpec {
        id: "arch",
        title: "层序与静态规则",
        command: "cargo xtask arch",
        tier: Tier::Fast,
        group: Group::Code,
        runner: Runner::Arch,
    },
    CheckSpec {
        id: "size",
        title: "文件体积",
        command: "cargo xtask size",
        tier: Tier::Fast,
        group: Group::Code,
        runner: Runner::Size,
    },
];

/// 检查档位（三个互斥开关的显式化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    /// 全集（默认）。
    All,
    /// 提交前快检（19 §8.3）。
    Fast,
    /// 只跑 rustdoc 门禁。
    Docs,
    /// 全集去掉 rustdoc 门禁。
    NoDocs,
}

impl Selection {
    /// 把三个开关映射成档位（纯函数；互斥由 clap 保证，这里只把优先顺序写死）。
    pub fn of(fast: bool, docs: bool, no_docs: bool) -> Self {
        if fast {
            Self::Fast
        } else if docs {
            Self::Docs
        } else if no_docs {
            Self::NoDocs
        } else {
            Self::All
        }
    }

    /// 摘要里显示的档位标签。
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "全集",
            Self::Fast => "--fast 快检",
            Self::Docs => "--docs 只跑 rustdoc",
            Self::NoDocs => "--no-docs 去掉 rustdoc",
        }
    }
}

/// 按档位筛选注册表，保持注册顺序（纯函数，测试直接调用）。
pub fn select(selection: Selection) -> Vec<&'static CheckSpec> {
    CHECKS
        .iter()
        .filter(|spec| match selection {
            Selection::All => true,
            Selection::Fast => spec.tier == Tier::Fast,
            Selection::Docs => spec.group == Group::Docs,
            Selection::NoDocs => spec.group != Group::Docs,
        })
        .collect()
}

/// `cargo xtask check` 的参数。
#[derive(Debug, Args)]
pub struct CheckArgs {
    /// 只跑提交前快检：fmt、clippy、arch、size（19 §8.3）
    #[arg(long, conflicts_with_all = ["docs", "no_docs"])]
    pub fast: bool,
    /// 只跑 rustdoc 门禁（CI docs 作业）
    #[arg(long, conflicts_with_all = ["no_docs"])]
    pub docs: bool,
    /// 跑全集但去掉 rustdoc 门禁（CI checks 作业）
    #[arg(long)]
    pub no_docs: bool,
    /// 仓库根目录（默认：从当前目录向上找到同时含 `Cargo.toml` 与图纸的目录）
    #[arg(long)]
    pub root: Option<PathBuf>,
}

/// 一项检查的执行结果。
enum CheckOutcome {
    /// 通过。
    Pass,
    /// 失败；`Command` 类带完整输出，进程内检查为空（明细已自行打印）。
    Fail(String),
}

/// 依次跑选中的检查并打印摘要：有失败退 1，全通过退 0。
///
/// # Errors
///
/// 定位仓库根、启动 `cargo` 子进程失败时返回错误（工具自身出错，由 `main` 统一退 2）。
pub fn run(args: &CheckArgs) -> anyhow::Result<ExitCode> {
    let root = match &args.root {
        Some(root) => root.clone(),
        None => find_repo_root(Path::new("."))?,
    };
    let selection = Selection::of(args.fast, args.docs, args.no_docs);
    let checks = select(selection);
    println!(
        "════ cargo xtask check（{} 项：{}）════",
        checks.len(),
        selection.label()
    );

    let mut failures = Vec::new();
    for spec in &checks {
        let started = Instant::now();
        let outcome = run_one(spec, &root)?;
        let elapsed = started.elapsed().as_secs_f32();
        match outcome {
            CheckOutcome::Pass => println!("✔ {:<7} {}（{elapsed:.1}s）", spec.id, spec.title),
            CheckOutcome::Fail(output) => {
                println!(
                    "✘ {:<7} {}（{elapsed:.1}s）——命令：{}",
                    spec.id, spec.title, spec.command
                );
                failures.push((spec.id, output));
            }
        }
    }

    if failures.is_empty() {
        println!("✔ {} 项全部通过", checks.len());
        return Ok(ExitCode::SUCCESS);
    }

    println!("════ 失败输出（{} 项）════", failures.len());
    for (id, output) in &failures {
        if output.is_empty() {
            println!("--- {id}：明细见上方输出 ---");
        } else {
            println!("--- {id} ---");
            print!("{output}");
            if !output.ends_with('\n') {
                println!();
            }
        }
    }
    Ok(ExitCode::from(1))
}

/// 跑一项检查。
///
/// # Errors
///
/// 启动外部命令失败、或进程内检查自身出错（工具类错误）时返回错误。
fn run_one(spec: &CheckSpec, root: &Path) -> anyhow::Result<CheckOutcome> {
    match spec.runner {
        Runner::Command { program, args, env } => {
            let output = Command::new(program)
                .args(args)
                .envs(env.iter().copied())
                .current_dir(root)
                .output()
                .map_err(|err| anyhow::anyhow!("启动 {program} 失败：{err}"))?;
            if output.status.success() {
                return Ok(CheckOutcome::Pass);
            }
            let mut text = String::new();
            text.push_str(&format!("退出码 {}\n", output.status));
            text.push_str(&String::from_utf8_lossy(&output.stdout));
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            Ok(CheckOutcome::Fail(text))
        }
        Runner::Arch => {
            let code = arch::run(&ArchArgs {
                root: Some(root.to_path_buf()),
            })?;
            Ok(outcome_of(code))
        }
        Runner::Size => {
            let code = size::run(&SizeArgs {
                root: Some(root.to_path_buf()),
            })?;
            Ok(outcome_of(code))
        }
    }
}

/// 把子检查的退出码翻译成本层结果（进程内检查的明细已自行打印，失败不再重复携带输出）。
fn outcome_of(code: ExitCode) -> CheckOutcome {
    if code == ExitCode::SUCCESS {
        CheckOutcome::Pass
    } else {
        CheckOutcome::Fail(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 注册表里被选中的 id（保持注册顺序）。
    fn ids(selection: Selection) -> Vec<&'static str> {
        select(selection).iter().map(|spec| spec.id).collect()
    }

    #[test]
    fn fast_selection_skips_doc() {
        assert_eq!(
            ids(Selection::Fast),
            vec!["fmt", "clippy", "arch", "size"],
            "--fast 应含 fmt/clippy/arch/size、不含 doc（19 §8.3）"
        );
    }

    #[test]
    fn docs_selection_only_doc() {
        assert_eq!(ids(Selection::Docs), vec!["doc"], "--docs 只跑 rustdoc");
    }

    #[test]
    fn no_docs_selection_is_full_minus_doc() {
        assert_eq!(
            ids(Selection::NoDocs),
            vec!["fmt", "clippy", "arch", "size"],
            "--no-docs = 全集去掉 cargo doc"
        );
    }

    #[test]
    fn all_selection_keeps_registry_order() {
        assert_eq!(
            ids(Selection::All),
            vec!["fmt", "clippy", "doc", "arch", "size"],
            "默认 = 全集，顺序照 19 §6.1 清单"
        );
    }

    #[test]
    fn doc_check_is_slow_and_only_member_of_docs_group() {
        let docs: Vec<&CheckSpec> = CHECKS
            .iter()
            .filter(|spec| spec.group == Group::Docs)
            .collect();
        assert_eq!(docs.len(), 1, "--docs 组当前应只有 rustdoc 一项");
        assert_eq!(docs[0].id, "doc");
        assert_eq!(docs[0].tier, Tier::Slow, "rustdoc 不进 --fast（19 §8.3）");
    }

    #[test]
    fn registry_ids_are_unique() {
        let mut seen = Vec::new();
        for spec in CHECKS {
            assert!(
                !seen.contains(&spec.id),
                "注册表 id 重复：{}（已有 {seen:?}）",
                spec.id
            );
            seen.push(spec.id);
        }
    }

    #[test]
    fn selection_of_maps_flags() {
        assert_eq!(Selection::of(false, false, false), Selection::All);
        assert_eq!(Selection::of(true, false, false), Selection::Fast);
        assert_eq!(Selection::of(false, true, false), Selection::Docs);
        assert_eq!(Selection::of(false, false, true), Selection::NoDocs);
    }

    #[test]
    fn every_selection_is_non_empty() {
        for selection in [
            Selection::All,
            Selection::Fast,
            Selection::Docs,
            Selection::NoDocs,
        ] {
            assert!(
                !select(selection).is_empty(),
                "{selection:?} 不应选出空集合"
            );
        }
    }
}
