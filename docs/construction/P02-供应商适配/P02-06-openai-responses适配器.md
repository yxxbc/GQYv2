# P02-06 · openai-responses 适配器

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P02-03 |
| 设计依据 | designs/06-模型供应商适配.md §5（信封：`Opaque` 原字节回放、`reasoning` 项）、§6（编码）、§7（`response.completed` / `incomplete` / `failed`、`incomplete_details`）、§8、§16（模板）；designs/04 §9.3（`encoding_version`）；06 §20 Q-06-4（`store=false` 全量回放，按推荐） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-provider`（`encode/openai_responses.rs`、`decode/openai_responses.rs`、模板）、`crates/gqy-config`（responses 模板） |

## 目标

Responses API 适配器：`input` 数组全量回放（`store=false`，账本是唯一真相——Q-06-4 的推荐）、`Opaque` 推理项按原始 JSON 字节写回、跨协议降级规则（换模型后丢签名与不透明项）在这一单统一验证一次。做完之后，三种协议齐了；P02-07 的用量方言与 P03 的门禁有了全部输入。

## 范围

- 做：
  - `encode`：`instructions`（system 段）/ `input` 数组 / `tools`（函数形态）/ `stream` / 采样参数，顺序照 06 §6“非消息字段在前、消息数组最后”；**`store = false` 且不使用 `previous_response_id`**（Q-06-4）；`Opaque { raw }` 以原始 JSON 字节写入 `input`；跨协议降级回放规则（06 §5.3：保留 `Text`/`ToolCall`，丢弃 `Thinking`/`RedactedThinking`/`Opaque`/签名/`ReasoningContent`）。
  - `decode`：事件流（`response.output_item.added`、`response.output_text.delta`、`response.function_call_arguments.delta`、`response.reasoning_summary_text.delta` 一类——以官方现行事件名为准并记录在单里）；终止事件 `response.completed` / `response.incomplete` / `response.failed`；`incomplete_details.reason = max_output_tokens` → `Length`、`content_filter` → `ContentFilter`；`response.failed` 按 §8.1 分类；无终止 → `StreamIncomplete`。
  - 用量：`input_tokens` / `input_tokens_details.cached_tokens` / `output_tokens`（形状进 `RawUsage`，映射在 P02-07）。
  - 模板：`gqy-config` 的 `openai-responses` 内置模板（协议、`store=false` 相关能力位）。
- 不做：
  - `previous_response_id` 的服务端状态（Q-06-4 已定不做）；服务端工具与 `reasoning.encrypted_content` 的续传优化（保持原字节回放即可）。
  - 其他协议（P02-04/05）、重试（P02-08）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-provider/src/encode/openai_responses.rs`（新增） | 编码 |
| `crates/gqy-provider/src/decode/openai_responses.rs`（新增） | 流解码与信封组装 |
| `crates/gqy-config/src/templates.rs` | `openai-responses` 模板 |
| `crates/gqy-provider/tests/{encode_responses,decode_responses,degrade}.rs`（新增） | 见“测试与守护” |
| `crates/gqy-provider/tests/fixtures/{responses,request-shapes/openai-responses}/…`（新增） | 夹具 |

## 接口草案

草案，以实现为准。

```rust
// gqy-provider/src/encode/openai_responses.rs
pub fn encode(profile: &ProviderProfile, plan: &RequestPlan) -> Result<WireRequest, EncodeError>;

// 跨协议降级（06 §5.3）：三协议共用一个纯函数集，本单把矩阵测齐。
pub fn degrade_for_protocol(env: &AssistantEnvelope, protocol: Protocol) -> AssistantEnvelope;
```

## 实施步骤

1. 编码：照 06 §16 与官方现行字段名实现；`input` 数组的项形态（message / function_call / function_call_output / reasoning 项）逐条编码。
2. `Opaque` 往返：原生 reasoning 项 → `Opaque` → 回放字节一字不差。
3. 解码：事件名以官方文档为准（施工时记录捕获到的真实形态；样本为手写）；终止与错误分类。
4. 跨协议降级：与 P02-04/05 的信封一致地实现 06 §5.3，并在这单加一条“三协议 × 降级”的矩阵测试（共用函数）。
5. 模板与夹具；`cargo xtask check --fast`、`cargo test -p gqy-provider --features mock`；提交：`feat(provider): openai-responses 适配器`。

## 测试与守护

- **`store=false` 断言**：body 里没有 `store: true`（也不出现 `previous_response_id`）；键序 `input` 最后。
- **`Opaque` 字节保真**：往返字节相等；去掉原始字节回放改为重编码 → 红。
- **降级矩阵**：同一信封在三协议下回放：responses 保留 `Opaque`，chat 丢弃推理、anthropic 丢弃 `Thinking`；逐协议断言。
- **终止与错误**：`incomplete_details` 两种 reason；`response.failed` → 分类；无终止 → `StreamIncomplete`。
- **确定性**：1000 次相同。
- 先红后绿对照（PR 贴输出）：把 `input` 挪到中间、跳过降级丢弃各一次。

## 验收流程

```sh
cargo test -p gqy-provider --features mock   # 全绿
cargo xtask check --fast                     # 全绿
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] `store=false` 与全量回放有断言；`input` 最后有键序断言
- [ ] `Opaque` 字节保真与三协议降级矩阵通过
- [ ] 事件名以现行官方文档核对并记录在 PR（样本为手写，注明来源）

## 风险与回退

- **事件名漂移**：Responses API 演进较快；夹具用“现行文档 + 记录来源日期”的方式，真实流量（P05 起）抓到差异时再加变体（加夹具，不改既有字节）。
- **回退**：删除模块与模板条目；其余适配器不受影响。
