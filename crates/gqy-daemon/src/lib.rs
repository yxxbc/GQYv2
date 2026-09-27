//! L6 组装层：唯一的 daemon 组装函数 `assemble_daemon(paths, cfg) -> DaemonHandle`，
//! 构造 store、provider、tools、memory、ext、engine、gateway、mesh，并注册 `ToolSource`、
//! `StorageDomain` 与 `SubsystemHooks`。库 crate，不创建 runtime——runtime 只由入口二进制创建（02 §2）。
//! 设计：01 §3、§4。
//!
//! 分层规则见 01 §3：本 crate 属 L6，只可依赖 L0–L5；只有它可以同时依赖 `gqy-engine` 与 `gqy-gateway`。
