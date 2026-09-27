//! L0 基础类型：ID 类型、`Clock` trait、规范化 JSON、错误分类 `ErrorKind`、安全文本切片、
//! 领域消息类型（`ContentPart`、`ToolSpec`）、跨 L2 共享的纯值类型（`RequestPlan`、
//! `LedgerEntry`、`ProviderProfile` 等）与 `SubsystemHooks` trait 及其输入输出类型。
//! 设计：01 §3、02 §4、04 §4.5、10 §4、18 §3。
//!
//! 分层规则见 01 §3：本 crate 属 L0，不得依赖任何其它 workspace crate。
