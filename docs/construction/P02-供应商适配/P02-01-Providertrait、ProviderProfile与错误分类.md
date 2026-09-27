# P02-01 · Provider trait、ProviderProfile 与错误分类

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P01-02（消息类型）、P01-03（配置） |
| 设计依据 | designs/06-模型供应商适配.md §3（trait、流事件、FinishReason）、§4（ProviderProfile）、§5（信封）、§6（确定性编码规则）、§8.1/§8.3（错误分类与溢出识别）、§10（ModelBinding、档位，含 2026-09-28 降级池）、§14（NormalizedUsage）；designs/04-前缀缓存账本.md §4.2–§4.5、§10（RequestRecord 结构）、§11（RequestPurpose）；designs/12-人格配置与场所.md §3、§4（key_ref）；00 §5（2026-09-28 决策） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-core`、`crates/gqy-provider`（新）、`crates/gqy-config` |

## 目标

把供应商层的**数据与接口**一次定型：`gqy-core` 补齐 04/06 共用的值类型（RequestPlan、账本条目、信封、用量、画像、绑定）；新建 `gqy-provider`（L2）承载 `Provider` trait、`StreamEvent`/`StreamSink`、`ProviderError` 分类与溢出识别；`gqy-config` 长出 `[providers]`/`[models]` 配置形状与模板机制。做完之后，mock 与三适配器只是往这些接口里填实现；**没有一件涉及网络 IO**。

## 范围

- 做：
  - `gqy-core` 值类型（照抄设计草案，`#[non_exhaustive]` 慎用、字段齐全即可）：
    - `RequestPlan`（04 §4.5）、`CacheHints`、`SamplingParams`、`StableSection`（类型先立，`surface_hash` 的计算在 P03-04）；
    - `LedgerEntry` / `EntryKind` / `Channel` / `Payload`（04 §4.2–§4.3，**纯数据定义**；三通道语义、payload 阈值与写入检查在 P03-01）；
    - `RequestPurpose` / `AuxKind`（04 §11）、`RequestRecord`（04 §10，结构先立，链哈希在 P03-05 计算）；
    - `AssistantEnvelope` / `EnvelopeItem` / `FieldPresence`（06 §5.1）、`FinishReason`（06 §3.3）；
    - `ProviderProfile`（06 §4 全字段 + `tool_schema_mode`）、`ModelBinding` / `ModelTier` / `ModelRole`（06 §10）；
    - `NormalizedUsage` / `UsageSource` / `UsageDialect`（06 §14；映射逻辑在 P02-07）、`Timing`、`RawUsage`（原始用量 JSON，保插入序）。
  - `gqy-provider`（新 crate）：
    - `Provider` trait（06 §3.1：`profile` / `encode` / `send`）、`WireRequest`、`ProviderResponse`；
    - `StreamEvent` / `StreamSink`（06 §3.2；含“是否已转发内容”的可查状态，供 P02-08 重试判定）；
    - `ProviderError` / `EncodeError`（带 `status`、错误原文前 2 KiB、`key_id` 指纹、`attempt`；分类枚举 `ProviderErrorKind`、`ProtocolError` 照 06 §8.1）；
    - 基础分类函数（纯函数 `classify_http(status, headers, body_excerpt) -> ProviderErrorKind`，含 `retry-after` 系解析与 06 §8.3 的溢出识别规则 1、2；规则 3 的本地估算以入参 `estimated_input_tokens: Option<u32>` 注入，P08 之前由调用方传 `None`）；
    - `EventSink` 的最小定义（供 `ProviderClient::run` 之后的 P02-08 与 P04-06 接线；本单只立 trait + 一个链式测试实现）。
  - `gqy-config`：`[providers.<id>]`、`[[providers.<id>.models]]`、`[models]` 结构体与默认值（06 §16 形状；默认值单一来源）；`template` 机制：模板名 → `ProviderProfilePatch`（**模板数据随对应适配器单加入**：P02-04 DeepSeek/OpenAI、P02-05 Anthropic、P02-06 responses）。
- 不做：
  - 一切网络与流处理（P02-02）、mock（P02-03）、协议编码/解码（P02-04–06）。
  - 用量映射与成本（P02-07）；重试、选钥、档位解析（P02-08）。
  - 账本三通道语义、化石化、前缀链（P03）；`surface_hash` 计算（P03-04）。
  - secrets 的存储与读取（P11-02；本单只定义 `key_ref` 形状与 `SecretResolver` 接口签名，P02-08 用测试实现）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `Cargo.toml` | `[workspace.dependencies]` 加 `futures`、`tokio-util`（P02-01 用）、`bytes` |
| `crates/gqy-core/src/{request,ledger,envelope,usage,profile}.rs`（新增） | 上述值类型 |
| `crates/gqy-core/src/lib.rs` | 模块声明与文档 |
| `crates/gqy-provider/Cargo.toml`、`src/lib.rs`（新增） | 新 crate（L2，依赖 gqy-core、gqy-config） |
| `crates/gqy-provider/src/{trait_,events,error,classify,overflow}.rs`（新增） | trait、流事件、错误、分类、溢出特征表 |
| `crates/gqy-provider/tests/classify.rs`、`tests/overflow.rs`（新增） | 分类与溢出夹具 |
| `crates/gqy-config/src/config.rs` | `[providers]`/`[models]` 结构与默认值、模板机制 |
| `docs/designs/01-总体架构.md` | §3 表已列 `gqy-provider`；无需改动（核对即可） |

## 接口草案

草案，以实现为准。主要签名（细节字段照 04/06 的草案抄全）：

```rust
// gqy-provider/src/trait_.rs
pub trait Provider: Send + Sync {
    fn profile(&self) -> &ProviderProfile;
    /// 纯函数：RequestPlan → 线上请求。不做 IO，不读时钟（06 §6）。
    fn encode(&self, plan: &RequestPlan) -> Result<WireRequest, EncodeError>;
    /// 发送一次（不重试），把原生流解析为统一事件；重试在 P02-08 的 client 层。
    fn send<'a>(
        &'a self, wire: &'a WireRequest,
        sink: &'a mut dyn StreamSink,
        cancel: CancellationToken,
    ) -> BoxFuture<'a, Result<ProviderResponse, ProviderError>>;
}

pub struct WireRequest {
    pub url: Url,
    pub headers: Vec<(HeaderName, HeaderValue)>,   // 密钥头在发送时注入，不参与 wire_hash
    pub body: Bytes,
    pub wire_hash: [u8; 32],                       // blake3(body)
}

pub trait StreamSink: Send {
    fn on_event(&mut self, ev: StreamEvent) -> Result<(), ProviderError>;
    /// P02-08 用：一旦转发过 Text/Reasoning 增量，任何错误不再重试（06 §8.2）。
    fn forwarded_content(&self) -> bool;
}
```

```rust
// gqy-provider/src/error.rs（节选）
pub struct ProviderError {
    pub kind: ProviderErrorKind,          // 06 §8.1
    pub status: Option<u16>,
    pub body_excerpt: Option<String>,     // 原文前 2 KiB，字边界安全
    pub key_id: Option<KeyId>,            // 指纹（前 8 hex），不含密钥
    pub attempt: u32,
    pub retry_after: Option<Duration>,
}
```

```rust
// gqy-core/src/profile.rs（节选）
pub struct ProviderProfile { /* 06 §4 全字段 + tool_schema_mode: ToolSchemaMode (Native|PromptJson) */ }
```

```rust
// gqy-config/src/config.rs（节选）
pub struct ProvidersConfig { /* BTreeMap<ProviderId, ProviderEntry>；ProviderEntry { template: Option<TemplateName>, protocol, base_url, key_ref: Vec<SecretRef>, key_selection, keepalive, write_grace_ms, price, models: Vec<ModelEntry> } */ }
pub struct ModelsConfig { pub default: ModelRef, pub roles: BTreeMap<ModelRole, ModelTier> }
```

## 实施步骤

1. `gqy-core` 的五个新模块按 04/06 草案写类型（**先写 serde 往返与字段齐全性测试**，草案缺字段的地方回设计文档补齐再写代码）。
2. `gqy-provider` 骨架 + trait + 错误类型；`cargo xtask arch` 核对层序（L2 → L0/L1）。
3. `classify` 与 `overflow`：先写夹具表（06 §8.1/§8.3 逐条；每条特征带来源注释），再实现；`retry-after` 支持 `retry-after-ms`、秒（可小数）、`try again in 20s`/`500ms` 文案（参考 miyu 3-4（下））。
4. `gqy-config` 的 `[providers]`/`[models]`：默认值（06 §16）与模板 patch 机制；密封测试。
5. `cargo xtask check --fast`、`cargo test -p gqy-core -p gqy-provider -p gqy-config`；提交：`feat(provider): trait、画像与错误分类`。

## 测试与守护

- **类型往返**：`ProviderProfile`/`ModelBinding`/`RequestPlan`/`LedgerEntry` 的 serde 往返（规范 JSON 写出器）逐字段；`FieldPresence` 的 `Absent` / `Empty` 各一例。
- **分类表（夹具）**：401/403→`Auth`；402→`QuotaExhausted`；429 带 `retry-after: 5`/`retry-after-ms: 250`/文案 `try again in 20s` → `RateLimited{retry_after}`；529/503→`Overloaded`；400 空体（低占用）→`BadRequest`；413 空体+估算 ≥0.9 窗口 → `ContextOverflow`；数字 `code` 不当状态码（miyu 教训）；限速文案不被误判为溢出（06 §8.3 规则 1 的“排除优先”案例：`Too many tokens, please wait`）。
- **溢出特征表**：06 §8.3 每条特征一个用例；表项带来源注释；去掉“排除优先”步骤，限速用例必红。
- **配置**：缺省全默认；`template` 解析（未知模板报错并给编辑距离提示）；`key_ref` 只接受 `secret:` 前缀（明文报错，12 §4）；`PerRequest` + `keepalive=true` 校验失败（06 §17）。
- 先红后绿对照（PR 贴输出）：改坏“排除优先”、去掉 `retry-after-ms` 分支各一次。

## 验收流程

```sh
cargo test -p gqy-core -p gqy-provider -p gqy-config   # 全绿
cargo xtask arch                                        # 层序绿（gqy-provider L2）
cargo xtask check --fast                                # 全绿
# 手检（可选）：把 06 §16 的 TOML 例子写进临时配置文件，load 后打印解析出的 ProviderProfile（不含密钥）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 04/06 草案中本单范围内的类型全部落地，字段与出处一一对应（PR 里列对照）
- [ ] 分类与溢出夹具覆盖 06 §8.1/§8.3 全部条目，先红后绿有证据
- [ ] `[providers]`/`[models]` 默认值与 06 §16 一致且只在 `Default` 一处
- [ ] `gqy-provider` 无网络依赖（reqwest 在 P02-02 引入）；层序门禁绿

## 风险与回退

- **草案与实现偏差**：`RequestPlan`/`LedgerEntry` 的字段若在实现中必须调整，先改 04 文档再改类型（铁律：设计先行）；P03 起会在这里追加字段（append-only 思想）而不是重排。
- **`Payload` 的形态**：04 §4.3 是“逻辑内容（规范 JSON）”；本单定义为可规范序列化的枚举 + 原始 blob 引用两种形态，阈值判断留 P03-01（`store.blob_inline_max`）。
- **`RawUsage` 保序**：供应商字段顺序进入取证，用保序结构（`Vec<(String, RawValue)>` 或 `serde_json` 的保序特性），不用 `HashMap`。
- **回退**：纯新增类型与 crate，revert 即可。

