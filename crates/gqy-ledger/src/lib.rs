//! L2 前缀缓存账本：账本条目模型、请求规划（`RequestPlan`）、化石化、前缀哈希链、工具面序列化。
//! 规划与编码都是不碰 I/O 的纯函数——字节契约要求确定性输出（04 §7、§12）。
//! 设计：01 §3、04。
//!
//! 分层规则见 01 §3：本 crate 属 L2，只可依赖 L0–L1；不得做 I/O。
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
