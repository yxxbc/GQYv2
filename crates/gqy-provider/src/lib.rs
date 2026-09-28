//! L2 供应商适配：`Provider` trait、三种协议适配器（openai-chat / openai-responses / anthropic）、
//! SSE 解析、能力画像、用量归一化、重试与密钥轮转、mock 供应商。
//! 设计：01 §3、06。
//!
//! 分层规则见 01 §3：本 crate 属 L2，只可依赖 L0–L1；重依赖 reqwest（rustls）只允许出现在这里。
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
