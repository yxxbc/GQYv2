//! L0 基础类型：ID 类型、`Clock` trait、规范化 JSON、错误分类 `ErrorKind`、安全文本切片、
//! 领域消息类型（`ContentPart`、`ToolSpec`）、跨 L2 共享的纯值类型（`RequestPlan`、
//! `LedgerEntry`、`ProviderProfile` 等）与 `SubsystemHooks` trait 及其输入输出类型。
//! 设计：01 §3、02 §4、04 §4.5、10 §4、18 §3。
//!
//! 分层规则见 01 §3：本 crate 属 L0，不得依赖任何其它 workspace crate。
//!
//! 测试放宽：`cfg(test)` 下允许 unwrap/expect/panic/索引（19 §6.1），非测试代码仍 deny。
//! 更新：AI 助手（Cline 会话），2026-09-28 22:21:06（P00-03）。
//! 更新：AI 助手（Cline 会话），2026-09-28 23:03:25（P00-05：错误分类、Secret、blocking 与日志骨架）。
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod blocking;
pub mod error;
pub mod secret;
