//! L2 工具系统：`Tool` trait、注册表、工具面编译、参数收口与内置核心工具（文件、编辑、补丁、搜索、命令、PTY）。
//! 设计：01 §3、07。
//!
//! 分层规则见 01 §3：本 crate 属 L2，只可依赖 L0–L1。
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
