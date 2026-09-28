//! L5 协议客户端（UDS / HTTP），供 TUI、CLI 与 mesh 复用。设计：01 §3、13。
//!
//! 分层规则见 01 §3：本 crate 属 L5，只可依赖 L0–L4。
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
