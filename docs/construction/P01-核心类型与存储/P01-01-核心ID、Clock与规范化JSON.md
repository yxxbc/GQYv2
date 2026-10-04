# P01-01 · 核心 ID、Clock 与规范化 JSON

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P00-05 |
| 设计依据 | designs/10-存储与数据演进.md §4.4（ID、Principal、时间）；designs/04-前缀缓存账本.md §8.2（规范 JSON）；designs/06-模型供应商适配.md §6；designs/02-运行时与并发模型.md §4、§10；designs/13-网关与API协议.md §5（时间格式） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-core` |

## 目标

`gqy-core` 拿到最底层的三样东西：**类型化 ID**（带前缀 ULID 文本，如 `ses_…`、`run_…`）、**Clock**（可注入时钟 + RFC 3339 UTC 格式化）、**规范 JSON 写出器**（`canonical_json`，字节契约的地基）。做完之后，P01 其余各单与后续所有阶段都从这里取 ID、时间与规范字节；前缀写法、ULID 单调性与 Principal 编码从此只有一个实现。

## 范围

- 做：
  - `ids`：每种 ID 一个 newtype（`SessionId`、`RunId`、`TurnSeq`、`EntrySeq`、`CallId`、`JobId`、`WorkId`、`TimerId`、`QuestionId`、`BlobHash`、`Principal`；前缀与集合按 10 §4.4 与各文档使用处为准）；每种都有 `parse`（读）与 `as_str`（写），**读写一样严**：只接受自己写出去的规范写法；ULID 生成器（**单调**，见下）；Principal 编码（v1 公式 + 固定向量）。
  - `clock`：`Clock` trait（最小面：`now_ms()`），`SystemClock` 与测试替身 `TestClock`；`format_rfc3339_utc` / `parse_rfc3339_utc`（13 §58）**用 `jiff` 实现**。10 §4.4 的“Unix 毫秒 UTC”是唯一存储表示。
  - `canonical_json`：按 04 §8.2 的写出器：对象键按字节序排序、无多余空白、数字最短往返、字符串不做 Unicode 规范化、非 ASCII 原样输出不转义。
  - 测试：每类 ID 的读写 fixture 与坏例子；ULID 单调性（先红后绿）；Principal 固定向量；规范 JSON 字节 fixture。
- 不做：
  - 本地日期/时区换算的**调用点**（日志文件名、P11-05 感知）——统一用 `jiff`（已定，00 §5、tech-stack）；本单只提供 Clock 与 RFC 3339 原语。
  - `EventSeq`、`CallId` 与供应商 `provider_call_id` 的配对规则等后续概念（随使用它的单）。
  - `call_<session 序>_<n>` 内部格式之外的供应商兼容细节（06/07 的单）。
  - `gqy-provider` / `gqy-ledger` 的“禁 HashMap”扫描规则（随那两个 crate 的首张单加；本单只在文档里写明约束）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `Cargo.toml` | `[workspace.dependencies]` 加 `ulid`、`blake3`、`serde_json` |
| `crates/gqy-core/Cargo.toml` | 依赖 `ulid`、`blake3`、`serde_json` |
| `crates/gqy-core/src/ids.rs`（新增） | ID newtype、解析/格式化、ULID 生成器、Principal |
| `crates/gqy-core/src/clock.rs`（新增） | `Clock` trait、`SystemClock`、`TestClock`、RFC 3339 UTC 格式化/解析 |
| `crates/gqy-core/src/canonical_json.rs`（新增） | 规范 JSON 写出器 |
| `crates/gqy-core/src/lib.rs` | 模块声明与文档 |

## 接口草案

草案，以实现为准。

```rust
// gqy-core/src/ids.rs

/// 带前缀的 ULID 文本 ID。每种 ID 一个类型：写混会被编译器拦下
/// （“回合编号”与“会话内序号”是两种东西，理由见 gqy 1-1 的同款决定）。
///
/// 读和写一样严：只接受自己写出去的规范写法（大写 Crockford Base32、固定前缀）。
/// 以后要放宽写法，加新写法（格式演进），不改旧的（参考 gqy 1-1 的风险一节）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionId(Ulid);      // "ses_…"
// 同构：RunId("run")、CallId("call")、JobId("job")、WorkId("work")、
// TimerId("timer")、QuestionId("q")、BlobHash 见下。
// TurnSeq / EntrySeq 是 u32 newtype：JSON 里是数字，代码里是两种类型。

/// 内容哈希：`blake3(raw)` 的 64 位小写 hex（10 §6.2）；`hex()` 之外的写法不外泄。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlobHash([u8; 32]);

/// ID 生成器：进程内共享一份（`Arc<IdGen>`）。
///
/// 用 ULID 的单调生成器：同一毫秒内递增，系统时间回拨不倒退。
/// 这不是锦上添花：gqy 施工单 3-9（补）实测过不单调的后果——
/// “列出会话”的测试单独连跑 200 次红 10 次（同一毫秒的两个会话，先后随机）。
pub struct IdGen { /* 共享的单调上下文 + 锁 */ }

impl IdGen {
    pub fn new(clock: Arc<dyn Clock>) -> Self;
    pub fn session(&self) -> SessionId;
    pub fn run(&self) -> RunId;
    /* …按需 */
}

/// Principal：沿用 v1 编码，保证导入后记忆归属不变（10 §4.4）。
/// blake3(len_le_u64(platform) ‖ platform ‖ len_le_u64(account_id) ‖ account_id
///        ‖ len_le_u64(user_id) ‖ user_id) 取前 24 位 hex，文本 `principal:<24hex>`。
/// 对照实现：v1 `src/platform_types.rs` 的 `PlatformPrincipal::stable_key`。
pub fn principal(platform: &str, account_id: &str, user_id: &str) -> Principal;
```

```rust
// gqy-core/src/clock.rs

/// 可注入时钟（02 §10）：超时与时间相关逻辑的测试都从这里换时间。
pub trait Clock: Send + Sync {
    /// Unix 毫秒 UTC（10 §4.4：存储里的唯一时间表示）。
    fn now_ms(&self) -> i64;
}

pub struct SystemClock;
pub struct TestClock { /* 可设定、可推进 */ }

/// 13 §58：`2026-09-28T04:00:00.123Z`。用 `jiff` 实现（唯一时间库，tech-stack、00 §5）；
/// 本地日期/时区换算同源。
pub fn format_rfc3339_utc(ms: i64) -> String;
pub fn parse_rfc3339_utc(s: &str) -> Result<i64, TimeError>;
```

```rust
// gqy-core/src/canonical_json.rs

/// 规范 JSON（04 §8.2、06 §6）：
/// - 对象键按字节序排序；
/// - 无多余空白；
/// - 数字最短往返（整数不带小数点）；
/// - 字符串不做 Unicode 规范化；
/// - 非 ASCII 原样输出、不转义。
///
/// 输入经过 serde_json 的 `Value` 中转（默认 BTreeMap 有序），
/// 所以即使上游不小心混入了无序容器，输出仍然稳定；但**线上类型禁止
/// `HashMap`**（06 §6.2）是构造侧的规定，扫描规则随 gqy-provider 的首张单加。
pub fn to_canonical_json<T: serde::Serialize>(v: &T) -> Result<String, CanonicalJsonError>;
```

## 实施步骤

1. 依赖入库：`ulid`、`blake3`、`serde_json` 进 `[workspace.dependencies]`；`cargo xtask arch` 确认层序仍绿。
2. `ids.rs`：先写 fixtures 与坏例子（读写得一字不差、坏写法拦下），再实现 newtype 与解析。
3. ULID 生成器：接上单调上下文；**先写“同一时钟毫秒内连造 1000 个，严格递增”的测试并证明它现在会红**（随机尾数版本），再换单调实现转绿——照 gqy 3-9 的做法，先红后绿。
4. Principal：按 v1 公式实现；固定向量写死进测试（施工时从 v1 源码 `~/Projects/gqy-agent/src/platform_types.rs` 的 `stable_key` 取或按公式生成，向量与推导过程写进 PR）。
5. `clock.rs`：`Clock`、`SystemClock`、`TestClock`、RFC 3339 往返。
6. `canonical_json.rs` + 字节 fixtures。
7. `cargo xtask check --fast`、`cargo test -p gqy-core`；提交：`feat(core): 核心 ID、Clock 与规范 JSON`。

## 测试与守护

- **ID 读写**：每类 ID 照样本写出去、读回来一字不差；坏例子逐条断红并检查报错信息（写清期望与实际）：错前缀、错长度、非法字符（Crockford 的 `I/L/O/U`）、小写（首版只收规范大写）、空串。
- **ULID 单调**：固定 `TestClock` 在同一毫秒，连造 1000 个 → 严格递增且前 48 位是该毫秒；时钟回拨 1 秒再生成 → 仍然严格递增（不倒退）。**区分能力**：把生成器换回“随机尾数”版本，此测试必红（记录在 PR；参考 gqy 3-9 的“修前 200 次红 10 次”现象）。
- **Principal 固定向量**：至少 3 组向量（含空串、多字节、大小写混合），改坏长度前缀或截断位置必红；同一向量在 P16 导入的兼容测试里复用。
- **规范 JSON**：键序（`"Z" < "a"` 字节序）、紧凑（无空白）、数字（`1` 与 `1.0` 的输出、最短往返）、非 ASCII 原样（中文/emoji 不被 `\u` 转义）、不做 Unicode 规范化（组合字符与预组合字符输出不同字节）。**区分能力**：去掉排序/改回转义，对应 fixture 必红。
- **RFC 3339**：往返一致（毫秒保留）；坏输入（缺 `Z`、错位数）报错。
- 测试输出遵循 19 §4.1：失败消息含期望值、实际值与相关 ID。

## 验收流程

```sh
cargo test -p gqy-core            # 全绿；测试名能看出每条守护（monotonic_、principal_fixed_vectors、canonical_…）
cargo xtask check --fast          # 全绿
# 先红后绿对照（PR 里贴输出）：
#   1) 去掉 ULID 生成器的单调上下文 → same-ms 测试红 → 恢复
#   2) 改坏 Principal 的长度前缀 → 固定向量红 → 恢复
#   3) 规范 JSON 改为默认转义 → 非 ASCII fixture 红 → 恢复
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 每类 ID 有读写 fixture 与坏例子；ULID 单调有先红后绿证据
- [ ] Principal 有 ≥3 组固定向量，注明与 v1 `stable_key` 的对照
- [ ] 规范 JSON 的每一条规则（排序/紧凑/数字/非 ASCII/不规范化）都有字节级 fixture
- [ ] `Clock`/`TestClock` 可在测试中确定性换时间（P02 起超时测试复用它）
- [ ] `ulid`/`blake3`/`serde_json`/`jiff` 进 `[workspace.dependencies]`；时间换算不经第二套实现

## 风险与回退

- **`ulid` crate 的单调 API**：以施工时的版本为准（`Generator`/`Context` 一类）；若它不满足“时钟回拨不倒退”，就包一层自己的上下文（先例：gqy 3-9 用的库自带该能力）。
- **大小写与校验严格度**：首版只收规范大写；如果后续真实数据里出现小写（例如导入、供应商回显），**加新写法**而不是放宽旧写法（格式演进规则）。
- **`BlobHash` 与 `ContentHash`**：10 §6.2 已定 blake3；若后续某处确需 sha256（例如形状夹具用 sha256），那是**另一个类型**，不要合并。
- **回退**：纯新增类型，revert 即可；不涉及数据。

