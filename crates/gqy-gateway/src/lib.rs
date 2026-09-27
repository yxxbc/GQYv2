//! L5 网关：axum HTTP/SSE/WS、UDS、鉴权、事件总线、连接器端点与 Web 静态资源内嵌。
//! 设计：01 §3、13、15。
//!
//! 分层规则见 01 §3：本 crate 属 L5，只可依赖 L0–L4；重依赖 axum / rust-embed 只允许出现在这里。
