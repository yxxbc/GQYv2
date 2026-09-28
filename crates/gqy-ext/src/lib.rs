//! L3 扩展体系：技能、脚本工具、MCP 客户端、扩展包管理；全部适配为 `Tool` / 提示片段。
//! 设计：01 §3、18。
//!
//! 分层规则见 01 §3：本 crate 属 L3，只可依赖 L0–L2。
//!
//! 测试放宽：`cfg(test)` 下允许 unwrap/expect/panic/索引（19 §6.1），非测试代码仍 deny。
//! 更新：AI 助手（Cline 会话），2026-09-28 22:21:06（P00-03）。
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]
