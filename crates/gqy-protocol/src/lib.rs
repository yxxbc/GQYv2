//! L0 对外协议类型：API DTO、事件信封、协议版本常量、连接器帧；可选 feature 用 ts-rs 生成 TS 类型。
//! 设计：01 §3、13、16。
//!
//! 分层规则见 01 §3：本 crate 属 L0（与 `gqy-core` 同层，互不依赖），不得依赖任何其它 workspace crate。
