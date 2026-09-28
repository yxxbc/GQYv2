//! `cargo xtask` 的命令行入口：解析子命令、统一退出码（通过 0、违规 1、工具错误 2）。
//!
//! 用法：`cargo xtask <子命令>`（别名见 `.cargo/config.toml`）。检查逻辑在库里（`src/lib.rs`），
//! 这里只做分派与退出码——新增子命令只登记，不改中心（21 §4）。
//! 创建：AI 助手（Cline 会话），2026-09-28 06:31:10。
//! 更新：AI 助手（Cline 会话），2026-09-28 22:21:06 —— P00-03：登记 `size` 与 `check` 子命令。

// 测试代码放开（19 §6.1；P00-03）：单元测试允许 unwrap/expect/panic/索引，非测试代码仍 deny。
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use std::process::ExitCode;

use clap::Parser;

/// 命令行参数（子命令都挂在 `Cmd` 上；新增子命令只加一个变体）。
#[derive(Debug, Parser)]
#[command(name = "xtask", about = "GQY v2 仓库开发工具（cargo xtask）")]
struct Cli {
    /// 要运行的子命令
    #[command(subcommand)]
    command: Cmd,
}

/// 子命令集合。
#[derive(Debug, clap::Subcommand)]
enum Cmd {
    /// 层序门禁与静态规则扫描（01 §3、19 §6.1 第 4、5 项）
    Arch(xtask::arch::ArchArgs),
    /// 文件体积门禁（00 §3、19 §6.1 第 6 项）
    Size(xtask::size::SizeArgs),
    /// 统一门禁入口：fmt / clippy / rustdoc / arch / size（19 §6.1、§8.3）
    Check(xtask::check::CheckArgs),
    /// 测试运行器与报告：`cargo test` + 解析 + 摘要 + 报告 + 计数门禁（19 §5）
    Test(xtask::test::TestArgs),
}

/// 执行子命令；违规的退出码由子命令返回（1），工具自身出错统一退 2。
fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Arch(args) => xtask::arch::run(&args),
        Cmd::Size(args) => xtask::size::run(&args),
        Cmd::Check(args) => xtask::check::run(&args),
        Cmd::Test(args) => xtask::test::run(&args),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("工具错误：{err:#}");
            ExitCode::from(2)
        }
    }
}
