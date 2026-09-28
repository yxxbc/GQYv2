//! L1 持久化：SQLite 单写者线程与读连接池、迁移框架、仓储（repo）、内容寻址 blob 存储、备份与完整性检查。
//! 设计：01 §3、10。
//!
//! 分层规则见 01 §3：本 crate 属 L1，只可依赖 L0；重依赖 rusqlite（bundled）只允许出现在这里。
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
