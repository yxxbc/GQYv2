//! L3 记忆与知识库：记忆存取、整理作业、联想检索、知识库；以 `Tool` 与 `SubsystemHooks` 形式挂载，
//! 引擎不写死对它的调用（18 §3）。设计：01 §3、11。
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
