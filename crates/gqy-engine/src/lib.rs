//! L4 回合引擎：Supervisor、会话 actor、准入与排队、回合状态机、压缩、子代理、钩子调度、作业与定时器。
//! 回合上下文走显式的 `TurnContext` 参数，不用 task_local，也不用可变全局状态（02 §4、§8）。
//! 设计：01 §3、02、03、05。
//!
//! 分层规则见 01 §3：本 crate 属 L4，只可依赖 L0–L3。
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
