//! L6 组装层：唯一的 daemon 组装函数 `assemble_daemon(paths, cfg) -> DaemonHandle`，
//! 构造 store、provider、tools、memory、ext、engine、gateway、mesh，并注册 `ToolSource`、
//! `StorageDomain` 与 `SubsystemHooks`。库 crate，不创建 runtime——runtime 只由入口二进制创建（02 §2）。
//! 设计：01 §3、§4。
//!
//! 分层规则见 01 §3：本 crate 属 L6，只可依赖 L0–L5；只有它可以同时依赖 `gqy-engine` 与 `gqy-gateway`。
//!
//! 测试放宽：`cfg(test)` 下允许 unwrap/expect/panic/索引（19 §6.1），非测试代码仍 deny。
//! 更新：AI 助手（Cline 会话），2026-09-28 22:21:06（P00-03）。
//! 更新：AI 助手（Cline 会话），2026-09-28 23:03:25（P00-05：logging 骨架与保留期清理）。
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

pub mod logging;
