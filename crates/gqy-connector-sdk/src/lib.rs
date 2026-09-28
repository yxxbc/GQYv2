//! L5 连接器共享客户端：WS 连接、握手、心跳、重连、`max_inflight` 窗口与水位推进。
//! 设计：01 §3、16 §4.1。
//!
//! 分层规则见 01 §3：本 crate 只依赖 `gqy-protocol` 与 WS 客户端库，不得依赖 L1–L4。
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
