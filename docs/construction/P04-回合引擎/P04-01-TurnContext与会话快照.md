# P04-01 · TurnContext 与会话快照

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P03-02（阶段整体先于本阶段；工具面字节来自 P03-04，`StableSection` 来自 P03-02/03） |
| 设计依据 | designs/02-运行时与并发模型.md §3、§4、§8；designs/03-回合引擎.md §4、§5.4；designs/12-人格配置与场所.md §5（PersonaSnapshot）、§6（Venue/VenueDecl）、§7.3（Surface 冻结）；designs/09-感知矩阵.md §3（PerceptionFrame）；designs/10-存储与数据演进.md §7.2（sessions 行与 WriteOnce 列）；designs/04-前缀缓存账本.md §8–§9（工具面字节与 StableSection）；designs/19-可观测性与测试.md §3.1（span 字段） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-engine`（新，L4）、`crates/gqy-core`（少量补字段）、`xtask/`（一条扫描规则） |

## 目标

`gqy-engine` 出现，并拿到全引擎共用的两样值：**TurnContext**（回合的显式上下文，02 §4 按值传给回合任务）与**会话快照**（回合开始时冻结、回合内字节不变的那一份输入）。做完之后，「回合内不变」是类型事实：配置、persona 代际、感知帧、工具面在回合开始就定影，回合任务只拿到 `Arc` 只读视图；业务上下文从此只有一个传递通道（参数），`task_local!` 与全局可变状态被扫描规则挡在门外。

## 范围

- 做：
  - `gqy-engine` crate 骨架（L4；依赖 gqy-core/store/config/provider/ledger；**不得创建 runtime、不得 `block_on`**，02 §2；`xtask arch` 按 01 §3 表核对）。
  - `TurnContext`：字段照 02 §4 草案逐项（session/run/parent_run/depth/venue/principal/persona/surface/model/workspace/perception/config/budget/cancel/events/clock）。
  - `SessionSnapshot`（回合冻结快照）与装配函数 `load(...)`：从 store 的会话行（`venue_json` 等 WriteOnce 列，10 §7.2）、`ArcSwap<Config>` 的当前值、persona 清单与工具面取值；**persona 与工具面按会话代际取值**（12 §5.5、§7.3）。
  - `SessionSurface`：会话内字节恒定的工具面承载（specs + P03-04 的规范化字节与 `surface_hash`）；本单只按值携带，不重算、不换面。
  - `SessionSummary`（`watch` 通道里的引擎内部状态摘要：活动运行、队列深度、待答问题数、最后活动时间；线上 DTO 归 P05-01）与冻结指纹字段（`persona_generation`、`surface_hash`、`config_revision`、`frozen_at`），供测试断言冻结。
  - xtask 扫描规则一条：`gqy-engine` 源码不得出现 `task_local!`（02 §4、19 §6.1 第 5 项）；带违规 fixture。
- 不做：
  - 可变的 actor 状态（`SessionState`、队列、当前运行）——P04-02/P04-03。
  - 准入与代际规则——P04-03；状态机与循环——P04-04。
  - 工具面的**编译**（P06-01）与换面时机（P11-04）；persona 热加载与代际切换机制（P11-03）。
  - 感知值的真实探测（P11-05）：本单只接受一个构造好的 `PerceptionFrame`（测试用合成值；`frozen_at` 用注入的 `Clock`）。
  - `ToolContext` 投影（P06-01，07 §4）；`TurnBudget` 的字段与检查点（P04-04 冻结，计量实现归 P04-05）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-engine/Cargo.toml`、`src/lib.rs`（新增） | 新 crate（L4）与模块文档 |
| `crates/gqy-engine/src/context.rs`（新增） | `TurnContext` 与构造辅助 |
| `crates/gqy-engine/src/snapshot.rs`（新增） | `SessionSnapshot`、`SessionSurface`、`load(...)`、`into_context(...)` |
| `crates/gqy-engine/src/summary.rs`（新增） | `SessionSummary` 与 `watch` 通道类型 |
| `crates/gqy-engine/tests/{freeze,snapshot}.rs`（新增） | 见「测试与守护」 |
| `xtask/src/arch/scan.rs` 等（修改） | `task_local!` 规则扩展到 `gqy-engine` + fixture |

## 接口草案

草案，以实现为准。

```rust
// gqy-engine/src/context.rs
/// 回合的显式上下文；回合内每一层都从参数拿它（02 §4）。
pub struct TurnContext {
    pub session: SessionId,
    pub run: RunId,
    pub parent_run: Option<RunId>,        // 子代理；本阶段只作为数据存在
    pub depth: u8,
    pub venue: Arc<Venue>,                // 来源场所：信任级、能力、交互性（12 §6）
    pub principal: Principal,             // 随会话冻结
    pub persona: Arc<PersonaSnapshot>,    // 会话代际内冻结（12 §5）
    pub surface: Arc<SessionSurface>,     // 会话内字节恒定的工具面
    pub model: ModelBinding,              // 供应商 + 模型 + 能力画像（06 §10）
    pub workspace: Arc<WorkspaceScope>,   // 工作区根、读写边界、沙盒策略（08 §4）
    pub perception: Arc<PerceptionFrame>, // 本回合冻结的感知值（09 §3）
    pub config: Arc<ConfigSnapshot>,      // 回合开始时的配置快照（12 §4）
    pub budget: TurnBudget,               // 字段与检查点在 P04-04 冻结；计量实现归 P04-05
    pub cancel: CancellationToken,        // 运行级取消令牌（树的构建归 P04-05）
    pub events: EventSink,                // 有界事件出口（总线实现归 P04-06）
    pub clock: Arc<dyn Clock>,
}

// gqy-engine/src/snapshot.rs
/// 回合开始时定影的会话快照；`frozen_at` 之后的配置/persona 改动不影响本回合。
pub struct SessionSnapshot {
    pub session: SessionId,
    pub venue: Arc<Venue>,
    pub principal: Principal,
    pub persona: Arc<PersonaSnapshot>,
    pub surface: Arc<SessionSurface>,
    pub model: ModelBinding,
    pub workspace: Arc<WorkspaceScope>,
    pub perception: Arc<PerceptionFrame>,
    pub config: Arc<ConfigSnapshot>,
    pub frozen_at: Timestamp,
}

impl SessionSnapshot {
    /// 从 store、配置与清单装配；错误写清期望与实际（缺哪一行、哪个字段、哪个路径）。
    pub async fn load(deps: &SnapshotDeps<'_>, session: &SessionId) -> Result<Self, SnapshotError>;
    /// 拆出 TurnContext；budget/cancel/events 由调用方（actor）提供。
    pub fn into_context(self, run: RunId, budget: TurnBudget,
                        cancel: CancellationToken, events: EventSink) -> TurnContext;
}
```

## 实施步骤

1. crate 骨架 + `task_local!` 扫描规则（先 fixture 红、再实现；每一步可单独编译）。
2. `SessionSnapshot::load`：会话行字段、venue 解析（12 §6.1）、persona 代际与工具面取值；先写「回合内不变」测试再实现。
3. `into_context` 与字段投影；`SessionSummary` 与 `watch` 通道（本单只做初始化路径）。
4. `cargo xtask check --fast`、`cargo test -p gqy-engine -p xtask`；提交：`feat(engine): TurnContext 与会话快照`。

## 测试与守护

- **冻结语义**：装配快照后热更新 `ArcSwap<Config>`、改 persona 清单文件与工具面输入，断言 `ctx` 的 config/persona/surface 字节不变（把快照改成惰性读文件必红）。
- **venue 只读**：`sessions.venue_json` 的 WriteOnce 触发器拒绝 UPDATE（复用 P01-06 的触发器测试路径）。
- **surface 一致**：同一会话两次装配得到同一 `surface_hash`；`SessionSurface` 的字节与 `StableSection` 中的 tools 字节逐字节相等（与 P03-04 的形状夹具互相印证）。
- **缺字段报错**：会话行指向的 persona 文件缺失时报错写清路径与期望值；不静默回退默认人格。
- **扫描规则**：`gqy-engine` 里放一个 `task_local!` 的违规 fixture，规则去掉即 fixture 失效。
- 先红后绿对照（PR 贴输出）：把 `load` 改成每次读文件一次。

## 验收流程

```sh
cargo test -p gqy-engine -p xtask   # 全绿
cargo xtask arch                    # gqy-engine 只依赖更低层；task_local! 规则覆盖 gqy-engine
cargo xtask check --fast            # 全绿
# 手检（可选）：临时库建会话 → 装配两份快照 → 打印 surface_hash/config_revision；
#              热更新配置后再装配 → 第三份不变、第四份变化
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] `TurnContext` 字段与 02 §4 逐项对应（任何差异在 PR 描述里说明）
- [ ] 冻结语义有区分能力测试（去冻结必红）
- [ ] `session{session}` / `run{run, turn, venue_kind}` span 字段可用（19 §3.1）；span 只放 ID 与枚举
- [ ] `gqy-engine` 通过层序与 `task_local!` 扫描

## 风险与回退

- **与 P03 类型的接口**：若 `StableSection` / `NormalizedSurface` 需要补字段，先改 04 的设计再动代码（设计先行）。
- **与 P11-04 的分界**：本单只承载只读的工具面快照；「什么时候换面」归 P11-04（12 §7.3），本单不得实现换面。
- **回退**：新 crate，revert 即可；store 表已在 P01-06，未动。
