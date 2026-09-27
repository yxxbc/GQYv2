# P08-03 · fork 摘要

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P08-02 |
| 设计依据 | designs/05-上下文与压缩.md §6.1（路径选择与 `compact.fork` 的 `auto` 口径）、§6.2（fork 请求形态、防续聊三重防护、必需节标题）、§6.4（两套模板与新建/更新指令）、§6.5（输出帽与超时）、§8（必须用会话 `ModelBinding`）、§15（fork 三条测试）；designs/04-前缀缓存账本.md §11（`RequestPurpose::CompactFork`、`parent_prefix`）、§12.1（门禁对 CompactFork 的比较口径）；designs/06-模型供应商适配.md §4（`tool_choice_none`、`max_output_tokens`）、§9.1（缓存语义三类）、§14（用量归一化）；designs/19-可观测性与测试.md §3.3（`purpose=compact_fork` 独立统计） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-engine`（`compact/fork.rs`、模板资源、`Summarizer` 的 Fork 实现）、`crates/gqy-ledger`（`CompactFork` 请求记录与 `parent_prefix`）、`crates/gqy-provider`（只消费 `tool_choice`/用量） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

fork 摘要把主链前缀「白吃」回来：请求 = 本纪元 `StableSection`（同字节）+ 视图直到切点的全部 `Context` 条目 + 尾部一条 user 摘要指令；响应经三重防护与节标题校验后产出摘要正文。做完之后：fork 请求字节满足 04 §12.1 对 `CompactFork` 的比较；指令与摘要**不**写入主链；`purpose=compact_fork` 的请求记录与用量非零（v1 教训 3 关闭）。

## 范围

- 做：
  - 路径选择 `SummaryPath::{Fork, Isolated, Placeholder}`；`compact.fork = auto`：会话绑定的缓存语义为 `Contract`/`BestEffort` → Fork、`PerRequest` → Isolated；`on`/`off` 显式覆盖（违反语义的配置如 `PerRequest + on` 返回配置校验错误）。
  - fork 请求构造：`[S]` 与主链同字节 + `entries[..cut]` 逐条目相同 + 尾部追加 user 条目（常量指令 + 模板 §6.4）；`RequestPurpose::CompactFork`；请求记录 `parent_prefix` 指向被复用的主链请求；用量照常写 `llm_requests`/`usage_records` 与 `gqy::usage` 日志。
  - 防续聊三重防护：支持时 `tool_choice = none`；指令首句常量 `CRITICAL: Do not call any tools. Respond only with the summary.`（英文资源）；代码层——响应出现任何工具调用即判失败（含流式增量里出现工具调用块）。
  - 校验：全部必需节标题可解析（允许内容 `(none)`），缺失 → 失败并列出缺失标题；空摘要不判错（节齐全即可）。
  - 帽与超时：`max_tokens = clamp(0.8 × reserved_output, 1024, 8192)`；超时 `90 s + max_tokens / 40`（单次请求总时长口径；首字节与流空闲沿用 02 §6）。
  - 模板资源（英文）：Coding 十节 / Chat 九节（05 §6.4）；新建与更新两套指令（更新含 PRESERVE/ADD/UPDATE；旧摘要先剥离足迹三块再送入——剥离点在 P08-04 接线，本单提供 `strip_blocks`）。
  - 失败分类 `SummaryFailure::{Timeout, ToolCallReturned, MissingSections, Provider}` 交给 P08-04 决策；本单不决定回退。
- 不做：
  - 隔离路径与占位摘要（P08-04）、失败矩阵的决策（P08-04）、足迹三块与回灌（P08-06）。
  - 四道闸（P08-05）、溢出恢复（P08-07）；`tool_choice` 的供应商实现细节（P02 已有）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-engine/src/compact/fork.rs`（新增） | 请求构造、三重防护、校验、失败分类 |
| `crates/gqy-engine/resources/prompts/compact/*.md`（新增） | 模板与指令（英文；位置与门禁 13 对齐） |
| `crates/gqy-engine/src/compact/mod.rs`（修改） | `Summarizer` 的 Fork 实现接位与路径选择 |
| `crates/gqy-ledger/src/freeze.rs`（修改） | `CompactFork` 记录的 `parent_prefix` 与 `epoch` 口径 |
| `crates/gqy-engine/tests/compact_fork.rs`（新增） | 见「测试与守护」 |

## 接口草案

草案，以实现为准。

```rust
// gqy-engine/src/compact/fork.rs
pub enum SummaryFailure { Timeout { elapsed_ms: u64 }, ToolCallReturned, MissingSections { missing: Vec<&'static str> },
    Provider(ProviderError) }
pub struct ForkSummary { pub text: String, pub tokens: u32 }

/// 请求字节 = 本纪元 StableSection + entries[..cut]（与主链相同）+ 尾部一条 user 指令。
pub async fn run_fork(ctx: &TurnContext, req: &SummaryRequest<'_>) -> Result<ForkSummary, SummaryFailure>;

// 模板装载（英文资源文件；门禁 13 同口径）
pub fn load_template(profile: ContextProfile, update: bool) -> &'static CompactTemplate;
pub fn strip_blocks(prev_summary: &str) -> &str;   // 送入更新模板前剥离足迹三块
```

## 实施步骤

（L 单：每一步都能单独编译通过。）

1. 模板资源与装载（先写「缺任一必需节 → 失败」的会红用例）。
2. 请求构造（用 mock 的请求取证断言「字节与主链前缀相同」）+ `CompactFork` 记录与 `parent_prefix`。
3. `tool_choice` 与三重防护；工具调用返回 → 失败。
4. 帽与超时；用量与请求记录非零（v1 教训 3 的回归测试）。
5. 缓存命中实测（mock 的缓存模拟）并把数字记录在 PR 描述。
6. `cargo xtask check --fast`、`cargo test -p gqy-engine -p gqy-ledger -p gqy-provider`；提交：`feat(context): fork 摘要`。

## 测试与守护

- **前缀复用**：fork 请求字节是主链上一请求的同 head + 消息项逐项前缀（04 §12.1 的 CompactFork 口径）；改一个字节此测试红。
- **不写主链**：指令与摘要在主链 `Context` 条目里不存在（按 seq 扫描断言）。
- **三重防护**：mock 返回工具调用 → `ToolCallReturned`；`tool_choice_none = false` 的画像 → 仍靠判据拦下（去掉判据测试红）。
- **节标题**：缺一节 → `MissingSections` 列出标题；全 `(none)` → 成功；剥离足迹三块后再送更新模板。
- **超时口径**：慢模型 mock（20 tok/s）+ 150k 上下文场景能完成（05 §15 的贴近实况验收）。
- **用量**：`purpose=compact_fork` 的行非零、`parent_prefix` 链接上、独立统计与 `main` 不混。
- 先红后绿对照（PR 贴输出）：去掉节标题校验、把 fork 记录写成 `main` 各一次。

## 验收流程

```sh
cargo test -p gqy-engine -p gqy-ledger -p gqy-provider   # 全绿
cargo xtask check --fast
# 手检（mock + 缓存模拟）：触发一次 fork 压缩，打印 fork 请求的 purpose / parent_prefix
#   与用量行（cache_read 占 input_total 的比例）；对照 05 §15 的「fork 用量落库非零」
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] fork 请求字节满足 04 §12.1 的 CompactFork 比较；指令与摘要不进主链
- [ ] 三重防护各有测试；节标题校验覆盖缺节与 `(none)`
- [ ] 用量与请求记录非零且 `parent_prefix` 正确（v1 教训 3 回归）
- [ ] 模板为英文资源文件且过门禁 13；`auto` 的选择输入（缓存语义）有对照用例

## 风险与回退

- **`tool_choice` 依赖画像**：默认 false 时防线收窄为指令 + 判据——如实测试，不假装三层都在。
- **超时公式**：`90 s + 帽/40` 是「单次请求总时长」的收窄值；若与 02 §6 的上限冲突，先改 02 再实现。
- **回退**：`fork.rs` 与资源文件是新增；`Summarizer` 接缝回到 P08-02 的「恒失败」替身，隔离/占位路径（P08-04）不受影响。
