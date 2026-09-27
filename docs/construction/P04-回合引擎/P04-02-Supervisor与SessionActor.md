# P04-02 · Supervisor 与 SessionActor

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P04-01 |
| 设计依据 | designs/02-运行时与并发模型.md §3（所有权模型、Supervisor、SessionActor、唯一准入入口）、§6（上界）、§7（有界通道）；designs/03-回合引擎.md §2（v1 教训表）、§3.2、§5.1（队列与锁的边界）、§14（执行权唯一）、§16（私有构造的扫描规则）；designs/10-存储与数据演进.md §4.3（会话载入）、§7.2（sessions 行）；designs/12-人格配置与场所.md §7.2（会话创建时的快照） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-engine`（`supervisor.rs`、`actor.rs`、`state.rs`）、`crates/gqy-config`（`session_idle_unload` 等默认值）、`xtask/`（一条扫描规则） |

## 目标

02 §3 的所有权模型从图变成可运行骨架：`Supervisor` 单任务持有 `SessionId → SessionHandle` 表、只做加载/卸载/路由；`SessionActor` 每个已加载会话一个任务，**独占** `SessionState`，消息循环只做状态转移，耗时工作一律 spawn 成子任务并以消息回报。做完之后，「压缩（或任何慢操作）把主循环拖死」在结构上不可能发生，执行权唯一不依赖锁而依赖 actor 所有权。

## 范围

- 做：
  - `Supervisor`：`load`（从 store 重建会话：快照、账本游标、队列，10 §4.3）/ `unload`（空闲 `session_idle_unload` 默认 30 min **且**无活动回合，02 §3.1；配置进 `gqy-config` 单一来源）/ `route`（按 `SessionId` 取句柄）；自身不含业务逻辑，永不阻塞在业务上。
  - `SessionHandle { tx: mpsc::Sender<SessionCmd>（有界 64，02 §7）, state: watch::Receiver<SessionSummary> }` 与句柄表的并发访问（表只在 Supervisor 任务内）、重复加载去重。
  - `SessionActor`：`SessionState`（照 02 §3.2：会话快照基座、当前运行句柄、`ExclusiveOp`、队列、账本游标）；消息循环分发 `SessionCmd`；`RunTask`/`ExclusiveOp` 以 `JoinHandle` + 取消令牌持有，结束经 `RunFinished`/`OpFinished` 回报。
  - 本单实现的命令语义：`Snapshot`（生成 `SessionView` 骨架：活动运行、队列、待答问题；`as_of_seq` 由 P04-06 提供）、`RunFinished`/`OpFinished`（回报与状态收敛）、`Exclusive` 的忙碌语义（有运行或独占时返回 `BusyError`）。
  - 其余命令的收件口与形参先立好但语义留白：`Submit`（P04-03 填准入判定）、`Cancel`（P04-05 填树与传播）、`Answer`（P06/P07 填审批通道）、`Shutdown`（P04-07 填宽限流程）。
  - 构造私有：`SessionActor::new` 只在 `gqy-engine` 内部可见；xtask 新增扫描规则（03 §16 末条）并带违规 fixture。
  - span：`session{session}`（actor 生命周期）与 `run{run, turn, venue_kind, parent_run?}`（19 §3.1），字段只放 ID 与枚举。
- 不做：
  - 准入与代际规则（P04-03）、状态机内部转移（P04-04）、事件总线实现（P04-06）。
  - 压缩与工具（P08/P06）：本单只提供「spawn 子任务 + 回报」的通用机制与测试替身。
  - 停机的宽限与恢复（P04-07）。
  - 进程生命周期（daemon start/stop、信号安装）——P05-05。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-engine/src/supervisor.rs`（新增） | `Supervisor`、`SessionHandle`、句柄表、加载/卸载/路由 |
| `crates/gqy-engine/src/actor.rs`（新增） | `SessionCmd` 分发、子任务 spawn 与回报、私有构造 |
| `crates/gqy-engine/src/state.rs`（新增） | `SessionState`（字段照 02 §3.2；队列与代际字段由 P04-03 使用） |
| `crates/gqy-config/src/config.rs`（修改） | `engine.session_idle_unload`、`engine.session_queue_capacity` 等默认值（本单先落用到的） |
| `crates/gqy-engine/tests/{actor_isolation,lifecycle}.rs`（新增） | 见「测试与守护」 |
| `xtask/src/arch/scan.rs` 等（修改） | `SessionActor::new` 私有性规则 + fixture |

## 接口草案

草案，以实现为准。

```rust
// gqy-engine/src/supervisor.rs
pub struct SessionHandle {
    pub tx: mpsc::Sender<SessionCmd>,       // 有界 64；满时按 02 §7 的策略显式选择
    pub state: watch::Receiver<SessionSummary>,
}
impl Supervisor {
    /// 引擎内唯一入口；`Supervisor::spawn` 之外拿不到 `SessionActor`（02 §3.3）。
    pub fn spawn(deps: SupervisorDeps) -> (JoinHandle<()>, SupervisorHandle);
    pub async fn submit(&self, req: TurnRequest) -> Result<Admission, SubmitError>; // 判定在 P04-03，本单只做加载 + 转发
    pub async fn snapshot(&self, session: &SessionId) -> Result<SessionView, SessionError>;
    pub async fn shutdown(&self, deadline: Instant) -> ShutdownOutcome;             // 语义在 P04-07
}

// gqy-engine/src/actor.rs
pub enum SessionCmd {                        // 照 02 §3.2
    Submit { req: TurnRequest, reply: oneshot::Sender<Admission> },
    Cancel { run: Option<RunId>, reason: CancelReason },
    Exclusive { op: ExclusiveOp, reply: oneshot::Sender<Result<OpTicket, BusyError>> },
    Answer { question: QuestionId, answer: Answer },
    RunFinished { run: RunId, outcome: RunOutcome },
    OpFinished { op: OpId, outcome: OpOutcome },
    Snapshot { reply: oneshot::Sender<SessionView> },
    Shutdown { deadline: Instant },
}
```

## 实施步骤

（L 单：每一步都能单独编译通过。）

1. `SessionState` + `Supervisor` 句柄表与 `watch` 通道；加载/卸载的最短路。
2. actor 循环：`Snapshot`/`RunFinished`/`OpFinished`/`Exclusive` 四类语义 + 子任务 spawn 与回报机制（用测试替身）。
3. 空闲卸载与重建；`session_idle_unload` 进配置默认值。
4. 私有构造扫描规则（先 fixture 红）与 span。
5. `cargo xtask check --fast`、`cargo test -p gqy-engine -p xtask`；提交：`feat(engine): Supervisor 与 SessionActor`。

## 测试与守护

- **不阻塞**（针对 v1 事故）：注册一个测试用的慢运行（等 10 s 才回报 `RunFinished`），期间连续发 `Snapshot` 与新的 `Submit`，断言在毫秒级返回；把业务 `await` 移回循环必红（02 §10 的隔离回归的引擎内版本）。
- **加载/卸载往返**：卸载后再次访问从 store 重建，快照指纹与账本游标与卸载前一致；有活动回合时**不**卸载（TestClock 前进 31 min 验证两分支）。
- **执行权唯一**：运行进行中提交 `Exclusive` → `BusyError`；`RunFinished` 之后再提交 → 成功。
- **回报顺序**：`RunFinished` 必在对应 `JoinHandle` 结束之后到达；同 `RunId` 重复回报报错，不覆盖状态。
- **私有构造**：fixture 在引擎外构造 actor → `cargo xtask arch` 报红。

## 验收流程

```sh
cargo test -p gqy-engine -p xtask   # 全绿
cargo xtask arch                    # 私有构造规则生效
cargo xtask check --fast
# 手检（可选）：起 3 个会话各跑一个假运行，观察 span 与卸载日志（空闲 30 min 用 TestClock 缩短）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 慢操作不拖住 actor 命令循环有区分能力测试（去掉 spawn 必红）
- [ ] 空闲卸载与重建有往返测试；有活动回合时不卸载
- [ ] `SessionActor::new` 私有性有扫描规则与违规 fixture
- [ ] 无无界通道（`mpsc::unbounded_channel` 在 `disallowed_methods` 名单内，02 §7）

## 风险与回退

- **actor 与 P04-03/04 的分界**：准入判定与状态机都不在本单；若发现需要调整 `SessionState` 字段，先改 02 §3 的设计再动代码。
- **`SessionView` 的完整性**：本单只提供骨架字段；`as_of_seq` 依赖 P04-06，缺失时字段留 `Option` 并在 P04-06 填实。
- **回退**：`supervisor.rs`/`actor.rs`/`state.rs` 是新文件，revert 即可；配置默认值一并回退。
