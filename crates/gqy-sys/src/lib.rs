//! L1 系统边界：C/libc FFI——PTY、信号、进程组与资源限制、OS 探测、沙盒后端（Landlock / macOS）。
//! 平台差异只在这里出现；调用方问“这里能不能做”，不问“这是不是某个系统”（AGENTS.md 平台一节）。
//! 设计：01 §3、08、09。
//!
//! 分层规则见 01 §3：本 crate 属 L1，只可依赖 L0。
//! `unsafe_code` 的区域性豁免在本 crate 内以模块级 `allow` + 每处 `// SAFETY:` 说明落地（01 §6，P07 起）。
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
