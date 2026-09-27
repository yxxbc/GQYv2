//! `cargo xtask` 的命令行入口：解析子命令、统一退出码（通过 0、违规 1、工具错误 2）。
//!
//! 用法：`cargo xtask <子命令>`（别名见 `.cargo/config.toml`）。检查逻辑在库里（`src/lib.rs`），
//! 这里只做分派与退出码——新增子命令只登记，不改中心（21 §4）。
//! 创建：AI 助手（Cline 会话），2026-09-28 06:31:10。

use std::process::ExitCode;

use clap::Parser;

/// 命令行参数（目前只有 `arch` 一个子命令，见 P00-02 的范围）。
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
}

/// 执行子命令；违规的退出码由子命令返回（1），工具自身出错统一退 2。
fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Arch(args) => xtask::arch::run(&args),
    };
    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("工具错误：{err:#}");
            ExitCode::from(2)
        }
    }
}
