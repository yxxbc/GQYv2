//! L5 设备共享：设备发现、全员同意配对、能力委派、同步合并与跨设备前缀交接。设计：01 §3、17。
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
