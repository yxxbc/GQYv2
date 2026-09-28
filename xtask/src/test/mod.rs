//! `cargo xtask test`：跑测试、解析 libtest 输出、打印摘要、写报告、跑测试计数门禁（19 §5；P00-04）。
//!
//! 模块分工（21 §4）：`parse` 解析 libtest 文本输出；`report` 产出 JSON/Markdown 报告；
//! `count_gate` 跑测试计数门禁；`ignore_tags` 扫描 `#[ignore = "needs:<tag>"]` 建立「用例 → 标签」映射。
//! 创建：AI 助手（Cline 会话），2026-09-28 22:42:25。

pub mod parse;
