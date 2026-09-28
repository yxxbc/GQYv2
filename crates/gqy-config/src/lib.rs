//! L1 配置与路径：`GqyPaths`、配置模式与默认值（唯一来源）、加载/校验/原子写、persona 清单解析。
//! 设计：01 §3、10 §3、12。
//!
//! 分层规则见 01 §3：本 crate 属 L1，只可依赖 L0；可调数值的默认值只在这里定义，别的 crate 不复制。
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
