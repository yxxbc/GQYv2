# P02-02 · HTTP 客户端、四类超时与 SSE 解析

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P02-01 |
| 设计依据 | designs/06-模型供应商适配.md §7.1（SSE 规则与上限）、§7.2（四类超时细则）；designs/02-运行时与并发模型.md §5（取消 1 s 内停读）、§6（超时表）；designs/16 无涉 |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-provider`、`.gitattributes` |

## 目标

请求真的能发出去、流真的能读进来，而且**每一次等待都有上界、都能取消**：reqwest 客户端（rustls、不跟随重定向、连接超时）、四类超时（建连 / 首字节 / 流空闲 / 总时长，空闲计时在任意字节上复位、含注释行与心跳）、以及一个按 WHATWG 规则自写的 SSE 增量解析器（跨块 UTF-8 边界安全、三平台一致、字节可喂）。做完之后，P02-03 的 mock 与 P02-04 起的适配器都跑在这一层之上。

## 范围

- 做：
  - `http.rs`：`ProviderHttp`（`reqwest::Client` 封装；rustls；`redirect::Policy::none`；连接超时来自 02 §6 默认 10 s、可按画像收窄/在 02 §6 上限内放宽）。
  - `sse.rs`：`SseParser`（字节进、事件出）：`data:` 多行拼接、`event:`、`:` 注释、空行分派、`id:`/`retry:` 忽略；`\n`/`\r\n`/`\r` 三种换行且 `\r\n` 跨 chunk 不产生空事件；一行的字节收全才做 UTF-8 解码（残字节缓存，汉字被切开不坏）；上限：单行 1 MiB、单事件 4 MiB、单响应累计 64 MiB → `Protocol(Oversized)`；流结束时把未分派的尾事件交出来，遗留半行报 `StreamIncomplete`。
  - `timeout.rs`：`TimeoutStream` 包装任意 `Stream<Item = io::Result<Bytes>>`，实现首字节 / 流空闲 / 总时长三类超时（建连在 reqwest 层）；空闲计时在**任意字节**上复位（注释行、心跳也算）；取消令牌触发后 1 s 内停读（02 §5）；分类为 `FirstByteTimeout` / `StreamStalled` / `TotalTimeout`。
  - `.gitattributes`：加 `*.sse -text`（夹具要保字节，不能被 LF 归一化；参考 miyu 3-4（下）的教训）。
  - 夹具目录约定：`crates/gqy-provider/tests/fixtures/sse/<name>.sse`（输入字节）+ `<name>.events.txt`（一行一条的期望事件），逐字节比对。
- 不做：
  - 协议语义解码（`[DONE]`、`message_stop` 等终止判定归各适配器，P02-04–06）；mock（P02-03）；重试（P02-08）。
  - 代理、证书固定、HTTP/2 强制等未列入设计的项。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `Cargo.toml` | `[workspace.dependencies]` 加 `reqwest`（rustls，默认不开 features 之外的项） |
| `crates/gqy-provider/Cargo.toml` | 依赖 `reqwest`、`bytes`、`futures`、`tokio`、`tokio-util` |
| `crates/gqy-provider/src/http.rs`（新增） | `ProviderHttp`、连接与请求头组装 |
| `crates/gqy-provider/src/sse.rs`（新增） | `SseParser`、`SseEvent` |
| `crates/gqy-provider/src/timeout.rs`（新增） | `TimeoutStream`、四类超时的分类 |
| `crates/gqy-provider/tests/{sse,timeout}.rs`（新增） | 见“测试与守护” |
| `.gitattributes` | `*.sse -text` |

## 接口草案

草案，以实现为准。

```rust
// gqy-provider/src/sse.rs

/// 一条已分派的 SSE 事件（字节层面的原样字段，不做协议解释）。
pub struct SseEvent { pub event: Option<String>, pub data: String, pub id: Option<String> }

pub struct SseParser { /* 残字节缓冲、当前事件、行/事件/累计计数 */ }
impl SseParser {
    pub fn new(limits: SseLimits) -> Self;              // 默认 1 MiB / 4 MiB / 64 MiB
    /// 喂入一段字节，尽可能多地吐出完整事件；不完整部分留在内部。
    pub fn feed(&mut self, bytes: &[u8]) -> Result<Vec<SseEvent>, ProviderError>;
    /// 流结束：交出差一个空行的尾事件；半行报 `StreamIncomplete`。
    pub fn finish(self) -> Result<Vec<SseEvent>, ProviderError>;
}
```

```rust
// gqy-provider/src/timeout.rs

/// 在任意字节流上叠加三类超时（建连在 reqwest 层）。
/// - 首字节：从流开始到第一个字节；
/// - 空闲：任意字节后重置（注释行、心跳都算）；
/// - 总时长：从发起到结束。
pub struct TimeoutStream<S> { /* … */ }
impl<S: Stream<Item = io::Result<Bytes>>> TimeoutStream<S> {
    pub fn new(inner: S, timeouts: TimeoutOverrides, total: Duration, cancel: CancellationToken) -> Self;
}
```

## 实施步骤

1. `sse.rs` 先写夹具与解析测试（三种换行、跨块 UTF-8、多行 data、注释、上限、半行结尾），再实现解析器（参考 miyu 3-4（下）：一行收全才解 UTF-8）。
2. `timeout.rs`：先写“暂停时钟 + 合成字节流”的测试（四类超时各一例、心跳复位一例、取消一例），再实现。
3. `http.rs`：客户端与头部组装（密钥头注入点留在 P02-08 的 client；本层只发 `WireRequest` 的既有头）。
4. 夹具样本：手写 DeepSeek / OpenAI / Anthropic 形状的 `.sse` 样本（含 CRLF 变体、被切开的汉字、注释心跳、`[DONE]` 前后），并把 `.gitattributes` 的 `-text` 一起提交。
5. `cargo xtask check --fast`、`cargo test -p gqy-provider`；提交：`feat(provider): HTTP 客户端、四类超时与 SSE 解析`。

## 测试与守护

- **解析器**：`\n`、`\r\n`、`\r` 三种换行；`\r\n` 恰好在两个 chunk 之间（不产生空事件）；`data:` 多行拼接；`:` 注释被忽略；**从每一个字节处切开喂**，结果与一次喂完完全一致（miyu 同款暴力测试）；汉字被切开不 panic、不替换；超限（行/事件/累计）各一例 → `Oversized`。
- **夹具比对**：每份 `.sse` 的解出结果与 `.events.txt` 逐字节一致；去掉“跨块 UTF-8 缓存”实现后，切字节用例必红。
- **超时**（`#[tokio::test(start_paused = true)]` + 合成流）：首字节超时；流空闲超时（有注释心跳时不触发）；总时长超时；取消令牌触发 1 s 内流被丢弃；分类分别是 `FirstByteTimeout` / `StreamStalled` / `TotalTimeout`。
- **HTTP 层**：不跟随重定向（一个 302 的本地桩 → 报错而不是跟跳）；连接被拒 → `Transport`（本地封闭端口）。
- 先红后绿对照（PR 贴输出）：去掉 UTF-8 残字节缓存、去掉空闲计时复位各一次。

## 验收流程

```sh
cargo test -p gqy-provider     # 全绿；含“逐字节喂”的暴力用例
cargo xtask check --fast       # 全绿
git check-attr text crates/gqy-provider/tests/fixtures/sse/*.sse   # 全部为 unset（-text 生效）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] SSE 解析符合 06 §7.1（含上限与 `StreamIncomplete` 语义）；暴力喂入测试通过
- [ ] 四类超时全部有暂停时钟下的确定性用例；取消在 1 s 内停读
- [ ] `.gitattributes` 的 `*.sse -text` 生效（`git check-attr` 验证）
- [ ] `reqwest` 只在 `gqy-provider`；层序与重依赖白名单绿

## 风险与回退

- **上界与实现的对应**：流空闲/首字节的具体数值来自画像与 02 §6；本单只实现机制与默认值，画像覆盖在 P02-04 起接入。
- **WHATWG 的边角**：`retry:`/`id:` 只忽略不实现重连（我们不做 EventSource 自动重连）；若某供应商依赖 `id:` 语义（如续传），在对应适配器单里再补。
- **回退**：新增模块，revert 即可。

