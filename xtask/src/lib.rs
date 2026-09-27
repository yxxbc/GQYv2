//! `xtask` 的库入口：仓库开发工具的检查逻辑都放在库里，二进制（`src/main.rs`）只做参数解析与退出码。
//!
//! 为什么有 lib：`xtask/tests/` 的集成测试要直接调用 `parse_layers` / `check_arch` / `scan_source`
//! （P00-02「门禁自测」），这些函数必须从 crate 外部可见。
//! 设计与规则出处：01 §3（层序）、19 §6.1（门禁清单）、21 §4（xtask 目录约定）。
//! 创建：AI 助手（Cline 会话），2026-09-28 06:31:10。

pub mod arch;
