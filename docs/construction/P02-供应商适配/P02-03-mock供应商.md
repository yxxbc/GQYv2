# P02-03 · mock 供应商

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P02-02 |
| 设计依据 | designs/06-模型供应商适配.md §13（MockServer 行为清单）、§12.2 无涉；designs/19-可观测性与测试.md §4.2（4.1 的 mock 规则）、§11（用量三处一致）；designs/04-前缀缓存账本.md §12.2（mock 与字节门禁的关系） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-provider`（`mock` feature）、`tests/` |

## 目标

一个**真的 HTTP 服务**上的假供应商：三种协议的端点与流格式、脚本化应答、故障注入、请求取证与可选缓存模拟。它是 P03 字节门禁、P04/P05 场景测试与端到端测试的共同底座——所有测试都走真实的 `reqwest` 与 SSE 解析路径，不用进程内替身（19 §4.1 的规定）。

## 范围

- 做（照 06 §13 清单）：
  - `MockServer`：`127.0.0.1` 随机端口、真实 HTTP/1.1（用 `hyper` 直接实现——它是 reqwest 的传递依赖，不引入新重依赖；`axum` 只许留在 `gqy-gateway`，01 §3）；三种协议的路径与响应形状（openai-chat 的 `data: [DONE]`、anthropic 的 `event:`+`message_stop`、responses 的 `response.completed` 等）。
  - `MockScript`：每次响应可含文本、推理（含 `FieldPresence` 的 `Absent`/`Empty`/`Text`）、签名、不透明项、工具调用（含并行、分段参数、编号晚到）、`finish_reason`、用量（含缓存字段、四种方言形状）。
  - 故障注入：`Delay{first_byte}`、`StallAfter{events}`、`Status{code, body, retry_after}`、`Overflow{style}`（各供应商溢出文案）、`MalformedUsage`、`DropConnection`、`NoTerminalEvent`、`Oversized`、`tokens_per_sec`（限速输出，供 05 的慢模型场景）。
  - 请求记录：方法、路径、请求头（密钥打码）、body 原始字节、到达时间——供断言（含“重试发同一 body”与字节门禁的字节比对）。
  - 缓存模拟（可选开关）：按 `cache_block_tokens` 粒度模拟前缀命中，在用量里返回 `cache_read`。
  - 与暂停时钟配合（`start_paused`）：回环 IO 在真实时间下很快，空闲/首字节超时按虚拟时间推进；README 遗留 5 的实测在此落地。
- 不做：
  - 协议解码与业务断言（P02-04–06 的测试在自己的单里写）；
  - 真实供应商的录制回放（需要时用 `wiremock` 夹具，19 §4.1；本单不做）。
  - 字节门禁本身（P03-06 消费本 mock）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-provider/Cargo.toml` | `mock` feature：`hyper`（server）、`tokio`（net） |
| `crates/gqy-provider/src/mock/mod.rs`（新增） | `MockServer` 启停、端口与 URL |
| `crates/gqy-provider/src/mock/script.rs`（新增） | `MockScript`、`MockResponse`、故障枚举 |
| `crates/gqy-provider/src/mock/protocols.rs`（新增） | 三协议的 SSE 写出格式 |
| `crates/gqy-provider/src/mock/record.rs`（新增） | 请求取证 |
| `crates/gqy-provider/tests/mock_self.rs`（新增） | mock 自我测试 |

## 接口草案

草案，以实现为准。

```rust
// gqy-provider/src/mock/mod.rs

/// 真实 HTTP 上的假供应商。Drop 时关闭。
pub struct MockServer { /* addr、脚本队列、记录 */ }
impl MockServer {
    pub async fn start(protocol: Protocol, script: MockScript) -> Result<MockServer, io::Error>;
    pub fn base_url(&self) -> &Url;               // http://127.0.0.1:<port>
    pub fn records(&self) -> Vec<RequestRecord>;  // 打码后的请求取证
}

// gqy-provider/src/mock/script.rs
pub struct MockScript { pub responses: VecDeque<MockStep> }
pub enum MockStep {
    Reply(MockResponse),
    Fail(Fault),      // Delay / StallAfter / Status / Overflow / MalformedUsage / DropConnection / NoTerminalEvent / Oversized
}
pub struct MockResponse {
    pub text: Option<String>, pub reasoning: Option<FieldPresence>,
    pub thinking_signature: Option<String>, pub opaque: Option<serde_json::Value>,
    pub tool_calls: Vec<MockToolCall>, pub finish: FinishReason,
    pub usage: Option<RawUsage>, pub tokens_per_sec: Option<u32>,
}
```

## 实施步骤

1. 选 `hyper` 的低层 server 组合（不引入 axum；不引入 tower 全家桶），先跑通“一个固定响应 + SSE 写出”。
2. 三协议的写出格式（分块、终止事件、用法事件的位置差异；DeepSeek/OpenAI 的用量字段差异照 06 §14.2）。
3. 脚本队列与故障注入逐项实现；每项配一个“线级可观察”的自我测试。
4. 请求记录（含密钥打码；打码后仍能断言“两次请求的 body 逐字节相同、密钥头不同”——P02-08 将用它）。
5. 缓存模拟（前缀命中 → `cache_read`），先只做 mock 侧自测。
6. `cargo xtask check --fast`、`cargo test -p gqy-provider --features mock`；提交：`feat(provider): mock 供应商`。

## 测试与守护

- **脚本按序播放**：三次响应的脚本 → 客户端侧收到三段内容；脚本用尽后再请求 → 明确报错（不静默挂起）。
- **每个故障的线级效果**：`Delay` 在首字节超时前/后各一例；`StallAfter{2}` 触发流空闲；`Status{429, retry_after}` 带 `retry-after` 头与体；`Overflow{style}` 的体命中 06 §8.3 特征表；`MalformedUsage` 的用量形状可被 P02-07 判 malformed；`DropConnection` 产生 `Transport`；`NoTerminalEvent` 被适配器判 `StreamIncomplete`（P02-04 起）；`Oversized` 超限；`tokens_per_sec` 的到达间隔单调。
- **请求取证**：两个请求（第一次 429、第二次成功）→ 记录的 body 逐字节相同、密钥头打码后不同；记录含到达时间可用于排序。
- **缓存模拟**：同一前缀第二次请求 → 用量里出现 `cache_read`；关掉开关 → 无 `cache_read`。
- **暂停时钟**：`#[tokio::test(start_paused = true)]` 下跑“Delay 超过首字节超时”的用例，断言超时分类且测试不真等 120 s（这是遗留 5 的实测点）。
- 先红后绿对照（PR 贴输出）：去掉 `StallAfter` 的“停止发字节”行为（改为继续发）→ 流空闲用例必红。

## 验收流程

```sh
cargo test -p gqy-provider --features mock   # 全绿
cargo xtask check --fast                     # 全绿
# 手检（可选）：起一个 MockServer，用 curl 打三协议的路径，看到对应的 SSE 形状
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 06 §13 的故障清单逐项有自我测试；脚本与记录可用
- [ ] 不引入 `axum`（`cargo tree -p gqy-provider` 无 axum）；层序门禁绿
- [ ] 暂停时钟下的超时组合实测通过（或按 README 遗留 5 改非暂停 + 注入 Clock，并记录）
- [ ] 记录的 body 字节可用于 P02-08 的“重发同一 body”断言

## 风险与回退

- **hyper 版本与 reqwest 的匹配**：以施工时锁定的版本为准；mock 只服务测试，不进入生产路径。
- **暂停时钟与真实 IO 的组合**（README 遗留 5）：如果 `start_paused` 与回环 HTTP 的组合在某些平台上不稳定（例如超时在 IO 完成前误触发），把该用例改为真实时间 + 短断言窗口（不动性能断言原则，只断言分类结果），并在单里记录取舍。
- **回退**：feature 关闭时 mock 不参与编译；revert 即可。

