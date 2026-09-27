# P02-04 · openai-chat 适配器

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P02-03 |
| 设计依据 | designs/06-模型供应商适配.md §5（信封与回放）、§6（确定性编码）、§7（流解析）、§8（分类与溢出）、§9（缓存语义两类投放）、§14（用量方言）、§16（深寻模板）；designs/04-前缀缓存账本.md §4.5、§9.3（`encoding_version`） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-provider`（`encode/openai_chat.rs`、`decode/openai_chat.rs`、模板）、`crates/gqy-config`（DeepSeek/OpenAI 模板数据） |

## 目标

第一个真实协议适配器：`RequestPlan` 按 06 §6 的布局**确定性**编成字节；SSE 流解成 `StreamEvent` 并组装 `AssistantEnvelope`；错误按 06 §8 分类；深寻（DeepSeek）与 OpenAI 两套用量方言的原始字段原样带出。做完之后，P03 的字节门禁有了第一条可比对的前缀链，P02-08 的重试有了可重发的同一份 body。

## 范围

- 做：
  - `encode`：非消息字段固定顺序（`model`、采样参数、`tools`、`tool_choice`、`system`（top-level，LateSystemRole 未支持时把尾巴消息包进 `<system-context>…</system-context>` 的 user 消息——与 04 §4.4 对齐）、会话缓存字段（`PromptCacheKey` 写体、`Header{name}` 写头，辅助请求不带）、`stream`、`stream_options.include_usage`（按画像）→ **`messages` 最后**；规范 JSON；`tool` 结果与 assistant 信封逐条回放（不合并相邻同角色条目）；`reasoning_content` 按 `send_reasoning_content` 决定；`arguments_raw` 非法 JSON 时按字符串原样回放。
  - `decode`：SSE 事件 → `StreamEvent`（`choices[].delta.content` → `TextDelta`；`reasoning_content`/`reasoning` → `ReasoningDelta`；工具调用按 `index` 累积、**`index` 缺失时按 `id` 归并**（部分网关），`id` 晚到/缺失时按 06 §5.2 生成 `call_<run 前 8 hex>_<request_seq>_<index>`；`usage` 事件（含 `[DONE]` 前后位置的差异）；`finish_reason` 映射照 06 §3.3 含旧 `function_call`）；无终止事件 → `Protocol(StreamIncomplete)`。
  - 信封：把增量组装为 `AssistantEnvelope`（保持供应商顺序；`ReasoningContent` 的 `FieldPresence` 区分）。
  - 用量：把原始 `usage`（DeepSeek 的 `prompt_cache_hit_tokens` / OpenAI 的 `prompt_tokens_details.cached_tokens` 等）打进 `RawUsage` 并标注 `UsageDialect`（映射在 P02-07）。
  - 模板：`gqy-config` 的 `deepseek`、`openai`（chat 系）内置模板数据（实测画像照 06 §16；`tool_schema_mode` 等新字段一并）。
  - `encoding_version` 常量（04 §9.3）与形状夹具目录约定 `tests/fixtures/request-shapes/openai-chat/*.json`。
- 不做：
  - 重试/选钥（P02-08）、用量映射与日志（P02-07）、anthropic 与 responses（P02-05/06）。
  - 保温与写入宽限（投放层；P02-08 起按配置接入——本单不实现）。
  - 网关兼容的逐项画像开关（未知即不发已经是默认；遇到具体网关再补模板字段）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-provider/src/encode/openai_chat.rs`（新增） | 编码 |
| `crates/gqy-provider/src/decode/openai_chat.rs`（新增） | 流解码与信封组装 |
| `crates/gqy-provider/src/lib.rs` | 导出 `OpenAiChat` 实现与 `encoding_version` |
| `crates/gqy-config/src/config.rs` 或 `src/templates.rs` | `deepseek`、`openai` 模板数据 |
| `crates/gqy-provider/tests/{encode_chat,decode_chat}.rs`（新增） | 见“测试与守护” |
| `crates/gqy-provider/tests/fixtures/request-shapes/openai-chat/*.json`、`fixtures/openai-chat/*.sse`（新增） | 形状与流夹具 |

## 接口草案

草案，以实现为准。

```rust
// gqy-provider/src/encode/openai_chat.rs
/// 单一路径：每种条目在 openai-chat 下只有这一个编码函数（04 I-7）。
pub fn encode(profile: &ProviderProfile, plan: &RequestPlan) -> Result<WireRequest, EncodeError>;

// gqy-provider/src/decode/openai_chat.rs
/// 流式解码器：SSE 事件进，统一事件出；结束时交出信封与用量。
pub struct ChatDecoder { /* 累积器：文本、推理、工具调用（按 index/id） */ }
impl ChatDecoder {
    pub fn new(profile: &ProviderProfile, request_seq: u32, run: &RunId) -> Self;
    pub fn on_event(&mut self, ev: &SseEvent) -> Result<Vec<StreamEvent>, ProviderError>;
    pub fn finish(self, finish: FinishReason) -> Result<ProviderResponse, ProviderError>;
}
```

## 实施步骤

1. 编码：先把 06 §16 的 DeepSeek 例子写成 `RequestPlan` → 目标 body 夹具（人工核对的期望字节），再实现编码器。
2. 布局与规范 JSON 断言：**消息数组是最后一个键**（解析 JSON 键顺序断言）；同一 plan 编码 1000 次字节相同、内部集合打乱构造顺序仍相同。
3. 解码：先写 DeepSeek / OpenAI 两份流夹具（含推理、缓存用量、分段工具调用、`[DONE]`、`index` 缺失的网关变体），再实现解码器；无终止事件 → `StreamIncomplete`。
4. 信封往返：原生响应 → 信封 → 回放编码，断言 `Absent`/`Empty` 区分、`arguments_raw` 原字节、非法 JSON 按字符串回放。
5. 模板数据（deepseek/openai）与 `encoding_version`；形状夹具入 `request-shapes/`（门禁在 P03-06 接）。
6. `cargo xtask check --fast`、`cargo test -p gqy-provider --features mock`；提交：`feat(provider): openai-chat 适配器`。

## 测试与守护

- **确定性**：1000 次编码字节相同；打乱构造顺序相同（去掉排序/规范化的实现 → 红）。
- **布局**：body 最后一个键是 `messages`；非消息字段顺序固定。
- **流夹具逐字节**：`.sse` → 事件序列与期望一致；**从每一个字节处切断喂入**结果一致（复用 P02-02 的暴力法）。
- **工具调用**：分段参数、`index` 缺失、`id` 晚到、并行多调用、参数被 `length` 截断（执行侧的拒绝规则在 03，本单只保证原字节回放）。
- **分类方言**：DeepSeek 溢出文案（`prompt has N tokens, but the configured context size…`）命中；`Too many tokens, please wait` 不误判；429 带 `retry-after`。
- **信封保真**：`FieldPresence` 三态、`reasoning_content` 回放条件（`ToolTurns`）。
- **与 mock 联调**：用 `MockServer` 打一遍“文本+推理+工具调用+用量”的脚本，断言 `ProviderResponse` 与记录。
- 先红后绿对照（PR 贴输出）：把 `messages` 从最后挪到中间、去掉 `index` 缺失归并各一次。

## 验收流程

```sh
cargo test -p gqy-provider --features mock   # 全绿
cargo xtask check --fast                     # 全绿
# 手检（可选）：对 MockServer 的 openai-chat 端点跑一次真实编码+发送，打印 wire_hash 与信封
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 06 §6 的六条规则逐条有测试；`messages` 最后有键序断言
- [ ] DeepSeek/OpenAI 流夹具逐字节比对通过（含分词切入）
- [ ] 信封往返夹具通过；非法参数按字符串回放
- [ ] 模板数据只存在于 `gqy-config` 一处；`encoding_version` 常量就位

## 风险与回退

- **网关变体**：`index` 缺失、旧 `function_call`、`usage` 位置差异都按“可观察地兼容”处理（不静默丢数据）；遇到新变体加夹具，不改既有字节。
- **DeepSeek 模板数值**：画像里的实测字段以 06 §16 为准，价格数字不入模板。
- **回退**：删除编码/解码模块与模板条目即可；mock 不受影响。

