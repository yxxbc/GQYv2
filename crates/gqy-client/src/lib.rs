//! L5 协议客户端（UDS / HTTP），供 TUI、CLI 与 mesh 复用。设计：01 §3、13。
//!
//! 分层规则见 01 §3：本 crate 属 L5，只可依赖 L0–L4。
