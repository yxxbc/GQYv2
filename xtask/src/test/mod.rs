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
