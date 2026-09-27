# P02-05 · anthropic 适配器

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P02-03 |
| 设计依据 | designs/06-模型供应商适配.md §5（信封与回放）、§6（编码；`tool_result` 分组规则）、§7（流解析：`event:`、`ping`、`input_json_delta`）、§8（`overloaded_error`、529）、§11（`cache_control` 断点规划）、§16（模板）；designs/04-前缀缓存账本.md §4.4（system-late 映射） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-provider`（`encode/anthropic.rs`、`decode/anthropic.rs`、模板）、`crates/gqy-config`（Anthropic 模板） |

## 目标

Messages API 适配器：顶层 `system`、内容块数组、签名思考的原样回放、`cache_control` 三断点、`input_json_delta` 累积、`ping` 心跳空转（只复位空闲计时）。做完之后，三种协议里的两种就位，且 Anthropic 的缓存断点规则有确定性测试。

## 范围

- 做：
  - `encode`：`system` 顶层；消息按条目逐条编码、`ToolResult` 分组为一条 `user` 消息里的多个 `tool_result` 块（06 §6.4）；工具 schema 用 `input_schema`；`temperature`/`max_tokens`/`thinking`（按画像）等非消息字段固定顺序；`arguments_raw` 非法 JSON 时编码为 `{"_raw": "<原始字符串>"}`（06 §5.2）；**`cache_control` 断点规划**（06 §11：稳定断点 = system 尾块；尾部断点 = 最后内容块；读取断点 = 上次尾部距本次 >20 块时；总数 ≤3、前缀短于 `cache_min_tokens` 不放、TTL 5 min/1 h）——断点是编码层叠加物，不改 `RequestPlan`。
  - `decode`：`event:` 名字分派（`message_start` / `content_block_start|delta|stop` / `message_delta` / `message_stop` / `ping` / `error`）；`ping` 与注释只复位空闲计时、不上抛；文本 `text_delta`、思考 `thinking_delta`+`signature_delta`、`redacted_thinking`、`input_json_delta`（按内容块索引累积）；`message_delta` 的用量（`cache_read_input_tokens` / `cache_creation_input_tokens` / `input_tokens` / `output_tokens`）；`error` 事件与 529 → `Overloaded`；`stop_reason` 映射（`end_turn`/`stop_sequence`→`Stop`、`tool_use`→`ToolCalls`、`max_tokens`→`Length`、`refusal`→`ContentFilter`、`pause_turn` 等→`Other`）。
  - 信封：`Thinking { text, signature }`、`RedactedThinking { data }` 的原样回放（`signed_thinking = true` 时带签名）；跨协议降级规则只在 P02-06 的单里测一次（同一实现，本单覆盖 anthropic 侧的行为）。
  - 模板：`gqy-config` 的 `anthropic` 内置模板（含 `late_system_role = None`、`cache_control = Anthropic{ttl: 5m, max_breakpoints: 3}` 等）。
- 不做：
  - 重试/选钥（P02-08）、用量映射（P02-07）、其他协议（P02-04/06）。
  - 服务端工具（`server_tool_use` 等）与 `pause_turn` 的重发策略（`Other` 原样带出；用到再定）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-provider/src/encode/anthropic.rs`（新增） | 编码 + 断点规划 |
| `crates/gqy-provider/src/decode/anthropic.rs`（新增） | 流解码与信封组装 |
| `crates/gqy-config/src/templates.rs` | `anthropic` 模板数据（若 P02-04 尚未建立该文件，本单建立并补上） |
| `crates/gqy-provider/tests/{encode_anthropic,decode_anthropic,breakpoints}.rs`（新增） | 见“测试与守护” |
| `crates/gqy-provider/tests/fixtures/anthropic/*.sse`、`fixtures/request-shapes/anthropic/*.json`（新增） | 夹具 |

## 接口草案

草案，以实现为准。

```rust
// gqy-provider/src/encode/anthropic.rs
pub fn encode(profile: &ProviderProfile, plan: &RequestPlan) -> Result<WireRequest, EncodeError>;

/// 断点规划：纯函数，便于单测（06 §11）。
pub fn plan_breakpoints(plan: &RequestPlan, profile: &ProviderProfile) -> Vec<BreakpointAddr>;
```

## 实施步骤

1. 编码骨架：非消息字段顺序 + `system` + 逐条消息映射；先写 `tool_result` 分组与 `{"_raw":…}` 的夹具。
2. 断点规划单独成函数（输入 `RequestPlan` 与画像，输出断点地址列表），先测规则再接线编码。
3. 解码：按 `event:` 分派；`input_json_delta` 按内容块索引累积；`ping` 空转；`error`/529 分类；`message_stop` 为终止。
4. 信封：`Thinking`/`RedactedThinking`/签名回放夹具（原生响应 → 信封 → 回放字节）。
5. 模板数据；`cargo xtask check --fast`、`cargo test -p gqy-provider --features mock`；提交：`feat(provider): anthropic 适配器`。

## 测试与守护

- **断点**（06 §11 逐条）：只 system 一块 → 1 个断点；普通两请求 → 稳定 + 尾部；上次尾部距本次 >20 块 → 出现读取断点；前缀短于 `cache_min_tokens` → 0 断点；`ttl = 1h` 时带 `"ttl":"1h"`；总数不超过 3。
- **分组**：`ToolResult` 批在一条 `user` 消息里（与 04 §6.4 一致）；跨 assistant 信封不合并。
- **流夹具**：`ping`/注释存在时不触发空闲超时（与 P02-02 的计时联动用例）；`NoTerminalEvent` → `StreamIncomplete`；`overloaded_error` → `Overloaded`。
- **回放**：签名思考原字节；`RedactedThinking` 原字节；`max_tokens` → `Length`。
- **编码确定性**：1000 次相同；打乱构造顺序相同。
- 先红后绿对照（PR 贴输出）：去掉读取断点的条件、把 `ping` 当成内容增量各一次。

## 验收流程

```sh
cargo test -p gqy-provider --features mock   # 全绿
cargo xtask check --fast                     # 全绿
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 06 §11 的断点规则逐条有测试；断点不改 `RequestPlan`（字节门禁剥离后再比对的前提）
- [ ] `input_json_delta`/`ping`/`error` 三类事件处理有夹具
- [ ] 签名思考与不透明数据回放逐字节一致

## 风险与回退

- **`cache_control` 与字节门禁**：断点是叠加物，P03-06 的比对要先剥离 `cache_control`（04 §12.1）；本单保证“剥离后与无断点版本逐字节一致”。
- **API 版本头**：`anthropic-version` 等常量头写入画像/编码（06 §16 模板），施工时以官方现行版本为准并在单里记录。
- **回退**：删除模块与模板条目；其他适配器不受影响。
