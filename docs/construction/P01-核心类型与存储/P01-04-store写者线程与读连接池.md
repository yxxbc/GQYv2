# P01-04 · store 写者线程与读连接池

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P01-01 |
| 设计依据 | designs/10-存储与数据演进.md §4.1–§4.4、§5；designs/02-运行时与并发模型.md §2、§6、§7、§8；designs/19-可观测性与测试.md §3.1（`store.writer` span）、§3.6（状态指标） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-store` |

## 目标

`gqy-store` 的核心发动机：**单写者线程**（唯一写连接、两条车道、分组提交、每作业 SAVEPOINT、提交后才回复、写队列超时 `StoreBusy`、提交失败进入降级）与**读连接池**（只读连接、blocking 池执行、池耗尽超时）。做完之后，P01 之后的所有持久化都经 `Store::write / Store::read`；异步代码里不再出现同步 SQLite 调用（02 §2）。

## 范围

- 做：
  - `Store`：打开/创建 `gqy.db`（PRAGMA 全表照 10 §4.1；`application_id=0x47515932` 校验；`journal_mode=WAL` 首次设置并校验返回值）、`Store::write(Lane, label, f)`、`Store::read(label, f)`。
  - 写者：两条有界车道（交互 4096 / 后台 256）、每批优先交互且最多夹带 1 个后台作业、分组提交（首作业后最多等 `group_commit_ms` 或凑 256 个）、一个 `BEGIN IMMEDIATE` + 每作业 `SAVEPOINT`、COMMIT 后统一回复、入队等待超时（默认 5 s，上限 30 s）返回 `StoreBusy`、作业执行超 1 s 记 `warn`、COMMIT 失败 → `quick_check` → 失败则进入**只读降级**并对外可见。
  - 读池：默认 4 条只读连接（`query_only=ON`），在 `gqy_core::blocking::run` 里执行；池耗尽等待至多 5 s → `StoreBusy`；查询超 2 s 记 `warn`。
  - 统计与健康：`StoreStats`（队列深度、批次数、作业数、最近提交耗时）供 19 §3.6 的 `/daemon/status`；`Store::health()`（`watch` 通道，`Ok | Degraded`）——装配到事件总线在 P04-06/P05。
  - 参数全部显式传入（`StoreOptions`，**不提供 `Default`**；数值的唯一来源是 `gqy-config` 的 `[store]` 默认值，映射在装配处 P05-05 做——避免两处默认值）。
- 不做：
  - 迁移（P01-05；本单打开库时不建业务表）。
  - 任何 repo（P01-06 起）。
  - blob 写入（P01-07）、备份（P01-08）。
  - 降级的对外广播与 UI 呈现（P04-06/P05；本单只把状态与拒绝行为做出来）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-store/Cargo.toml` | 依赖 `rusqlite`（`bundled`）、`gqy-core`、`tokio`（`sync`、`rt` 供测试）、`tracing` |
| `crates/gqy-store/src/lib.rs` | 模块声明与文档 |
| `crates/gqy-store/src/store.rs`（新增） | `Store`、`StoreOptions`、`Lane`、写者线程、分组提交 |
| `crates/gqy-store/src/read.rs`（新增） | 读池 |
| `crates/gqy-store/src/pragmas.rs`（新增） | 连接参数（10 §4.1 表的唯一实现） |
| `crates/gqy-store/src/health.rs`（新增） | `StoreHealth`、`StoreStats`、`StoreError` |
| `crates/gqy-store/tests/store_*.rs`（新增） | 见“测试与守护” |

## 接口草案

草案，以实现为准。

```rust
// gqy-store/src/store.rs

/// 写作业的车道（10 §4.2）。交互车道优先；后台车道每批最多夹带 1 个。
pub enum Lane { Interactive, Background }

/// 运行参数。字段没有默认值：数值的唯一来源是 gqy-config 的 [store]（10 §12），
/// 装配处（P05-05）负责映射；测试直接给出显式数值（确定性优先）。
pub struct StoreOptions {
    pub synchronous: bool,            // true = FULL
    pub group_commit_ms: u64,
    pub read_pool_size: u32,
    pub enqueue_timeout: Duration,
    pub interactive_queue: usize,
    pub background_queue: usize,
}

pub struct Store { /* writer handle, read pool, stats, health */ }

impl Store {
    /// 打开或创建主库。`db_path` 由调用方给出（通常来自 GqyPaths::db_path()，
    /// 但 gqy-store 与 gqy-config 同层，不互相依赖——装配处传路径）。
    ///
    /// # Errors
    /// 目录不可写、`application_id` 不是 GQY2、PRAGMA 校验失败时返回错误。
    pub fn open(db_path: &Path, opts: StoreOptions) -> Result<Store, StoreError>;

    /// 唯一写入口。闭包在写者线程内、一个 SAVEPOINT 中执行；同步闭包
    /// （类型上就 await 不了），不得做网络/文件 IO（10 §4.2）。
    ///
    /// `Ok` 意味着已提交（分组提交成功之后才回复）。
    ///
    /// # Errors
    /// 入队超时（`StoreBusy`）、作业自身错误、提交失败/降级（`StoreDegraded`）等。
    pub async fn write<R: Send + 'static>(
        &self, lane: Lane, label: &'static str,
        f: impl FnOnce(&WriteTx<'_>) -> Result<R, StoreError> + Send + 'static,
    ) -> Result<R, StoreError>;

    /// 只读查询，在 blocking 池执行（02 §2）。
    pub async fn read<R: Send + 'static>(
        &self, label: &'static str,
        f: impl FnOnce(&ReadConn) -> Result<R, StoreError> + Send + 'static,
    ) -> Result<R, StoreError>;

    pub fn stats(&self) -> StoreStats;                 // 19 §3.6
    pub fn health(&self) -> watch::Receiver<StoreHealth>;
}
```

```rust
// gqy-store/src/health.rs

pub enum StoreHealth { Ok, Degraded { reason: StoreDegradedReason } }

/// 降级原因要如实：提交失败、完整性检查未通过、写者线程异常。
pub enum StoreDegradedReason { CommitFailed, QuickCheckFailed, WriterPanic }

pub enum StoreError {
    Busy { waited: Duration },          // 入队/池等待超时
    Degraded { reason: StoreDegradedReason },
    Sqlite(rusqlite::Error),
    Job { label: &'static str, message: String },   // 作业闭包返回的错误
    /* … */
}
```

## 实施步骤

1. `pragmas.rs`：10 §4.1 表逐行实现 + 断言（含 `application_id` 校验、WAL 返回值）。
2. 写者骨架：先跑通“单作业写入 → 回复”的最短路径，再加车道与分组提交。
3. 分组提交：批次计数、`SAVEPOINT` 隔离、提交后回复（三条行为各配一个测试）。
4. 降级路径：COMMIT 失败 → `quick_check` → 降级 → 后续写入快速 `Degraded`。
5. 读池：连接创建、`blocking::run` 包装、等待超时、慢查询 warn。
6. `StoreStats` 与 `health` 通道；`store.writer` span 字段（batch_id、jobs、lane 统计）。
7. `cargo xtask check --fast`、`cargo test -p gqy-store`；提交：`feat(store): 单写者线程与读连接池`。

## 测试与守护

全部在 `TestHome`/临时目录里跑（不碰真实数据目录）。区分能力：先写测试、确认“去掉实现会红”，PR 里贴证据。

1. **PRAGMA**：打开后逐项断言 WAL、synchronous、busy_timeout、foreign_keys、`query_only`（读连接）、`application_id`、`cache_size`、`journal_size_limit`；**区分**：移除任一项设置 → 对应断言红。
2. **别家的库**：造一个 `application_id` 不同的 SQLite 文件 → `open` 拒绝，错误写明期望与实际。
3. **写读一致**：`write` 返回后，用**新的**只读连接（读池）能读到该行——证明“提交后才回复”。**区分**：把回复提前到 COMMIT 之前，此测试红。
4. **分组提交**：快速提交 256+ 个作业 → `stats.batches` 明显小于作业数；`group_commit_ms = 0` 时行为仍正确（每批至少一个作业）。
5. **SAVEPOINT 隔离**：同批两个作业，A 返回 `Err`，B 成功 → A 的写入回滚、B 生效，且 B 的返回值正确。
6. **入队超时**：容量 1 的写者被一个长作业占住（测试里 `sleep`）→ 第二个 `write` 在短超时后 `Busy { waited }`；恢复后照常可写。
7. **车道**：塞满后台车道 + 一个交互作业 → 交互作业不被饿死（每批最多夹带 1 个后台，有 `stats` 可观测）。
8. **COMMIT 失败 → 降级**（`#[cfg(unix)]`）：把数据目录权限撤掉使 COMMIT 失败 → 断言 `health() = Degraded`、后续 `write` 返回 `Degraded`、`read` 仍可用（只读降级）；把权限恢复后重启 `Store` 才能复原（重启语义在 P05-05）。Windows 上跳过并注明。
9. **读池**：4 条连接并发 4 个慢读 → 第 5 个等待超时 `Busy`；池恢复后可读。
10. **写者线程存活**：作业闭包 panic（测试代码允许 panic）→ 该作业得到错误、线程与后续作业正常（panic 不吞：错误里带 label）；**区分**：去掉 catch 边界 → 线程死亡、后续作业红。
11. **span/告警**：tracing 测试订阅器断言 `store.writer` span 与 batch_id 字段；慢作业 >1 s 的 warn。

## 验收流程

```sh
cargo test -p gqy-store          # 全绿；测试名对应上表 11 类
cargo xtask check --fast         # 全绿
# 先红后绿对照（PR 贴输出）：
#   1) 提前回复（COMMIT 前）→ 测试 3 红 → 恢复
#   2) 去掉 SAVEPOINT → 测试 5 红 → 恢复
#   3) 去掉入队超时 → 测试 6 红 → 恢复
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 10 §4.1 的 PRAGMA 表全部落地并有断言
- [ ] “提交后才回复”“SAVEPOINT 隔离”“降级不假装写成功”三条有独立测试
- [ ] `StoreOptions` 无默认值；数值来源唯一（注释写明映射在 P05-05）
- [ ] `StoreStats` 含 19 §3.6 需要的队列深度与批次计数
- [ ] 全部测试用临时目录；无真实数据目录写入

## 风险与回退

- **rusqlite 与线程**：写者线程持有唯一写连接，读连接在各自线程（blocking 池）里使用；连接不跨线程共享（`Connection` 非 `Sync` 的约束就是设计要的姿态）。
- **分组提交的等待实现**：写者线程是 OS 线程（std），等待用 `std::thread::sleep` 或以 channel 的 `recv_timeout` 凑批；不要为它造 runtime。具体机制以实现为准。
- **`write_queue_timeout_ms` 的 5 s / 30 s**：值是 config 默认与上下限（10 §12、02 §6）；本单只做“可传入 + 超时行为”，键的读取在 P05-05。
- **降级测试依赖文件权限**：Windows 上没有等价构造，跳过的用例要在 PR 与测试代码里写明原因（不静默放过）。
- **回退**：新增模块，revert 即可；库文件是新建的，不涉及既有数据。

