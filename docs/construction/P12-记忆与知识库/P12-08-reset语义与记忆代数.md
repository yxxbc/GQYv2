# P12-08 · reset 语义与记忆代数

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P12-05 |
| 设计依据 | designs/11-记忆与知识库.md §9.1（四个命令与 `/wipe` 的降级）、§9.2（代数守护：同事务 +1、写作业携带、同步与异步路径、物理删除范围、化石块归属）、§12（边界）、§15（Q-11-2、Q-11-3）；designs/10-存储与数据演进.md §9.1（`gqy data wipe` 前自动备份）、§9.3（会话清除）、§4.4（删除白名单触发器）；designs/13-网关与API协议.md §5.2（会话删除与 `/ops`）、§6.2（事件登记）；designs/08-权限审批与沙盒.md §5.6（谁能回答确认）、§9.1（审计）；designs/02-运行时与并发模型.md §3.2（独占操作）；designs/00-设计理念.md 铁律 9（用户数据不静默丢失） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-memory`（`reset.rs`）、`crates/gqy-engine`（命令处理与独占操作）、`crates/gqy-gateway`（命令路由与确认参数）、`apps/gqy`（`data wipe` 子命令）、`tests/`（代数端到端） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

reset 收敛为四个命令 + 一条命令行：`/clear` 只清客户端屏幕；`/new` 归档当前会合并新开（不删任何记忆）；`/forget-session` 删除本会话产生的记忆（需确认）；`/forget-all` 删除当前 persona 的全部记忆（需输入 persona 名确认）；v1 的 `/wipe` 不再是聊天命令，改为 `gqy data wipe --persona <id>`（执行前自动备份）。代数在同一写事务 +1；所有异步与同步写入在写作业内比对代数，不符即整批放弃并记 `memory.write_rejected{reason:"stale_generation"}`。做完之后：清完不复活；删除是物理删除且可追溯（备份 + 审计）；会话删除不关记忆的事（语义写清）。

## 范围

- 做：
  - 命令表（P09-04 的机制）与服务端命令：四个命令的注册、参数与确认流程；`/clear` 是纯客户端命令；`/new` 走引擎的归档 + 新会话（同一场所绑定改指新会话）；两个 `/forget-*` 走 `SessionCmd::Exclusive`（忙时 409 `Busy`，入口提示）。
  - `gqy data wipe --persona <id>`：先自动备份（10 §9.1）→ 删除该 persona 的全部记忆 → 输出摘要（删除行数、备份路径）；需显式 `--yes` 或交互确认。
  - 代数：`memory_scopes.generation` 与删除在同一写事务 +1，记 `last_reset_at` / `last_reset_kind`；所有写作业（整理写回、复盘、召回登记、日记、`remember`）在作业内先比对（已有接缝在本单端到端验收）。
  - 拒绝路径：不符 → `StaleGeneration`，整批放弃、作业标记成功（异步）/ 英文说明（同步）；记 `memory.write_rejected`（名字按 README 复核第 2 条登记进 13 §6.2）。
  - 删除范围：`memories`、`memory_tags`、`memory_subjects`、`memory_sources`、`memory_recalls`、`memory_postings`（含 `evicted:<session>`）中的对应行；`kb` 文档与索引**不在**范围（参考资料而非记忆；删除 kb 走文件系统，本单不发明 kb 的 reset 命令）。
  - 触发器白名单接上（P12-01 留的口）：删除语句只允许出现在 reset 与保留清理两条登记路径里。
  - 会话删除语义（如实）：`DELETE /sessions/{id}`（13 §5.2）走 10 §9.3 的 30 天清除，**不删记忆**；已化石化进账本的召回块属于会话数据，随会话清除，不随 `/forget-*` 删除（11 §9.2）。
  - doctor 与审计：显示各 persona 的代数与最近 reset；`/forget-*` 与 `data wipe` 记审计（08 §9.1）与备份路径。
- 不做：
  - kb 的 reset；TUI 的 `/wipe` 确认界面（遗留）；v1 导入（P16）；群聊上下文的清理（16 范围，如需以 `gqy data wipe` 扩展另行开单）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-memory/src/reset.rs`（新增） | 四个命令的实现与代数事务 |
| `crates/gqy-memory/src/repo/deletes.rs`（新增） | 登记删除路径与触发器核对 |
| `crates/gqy-engine/src/commands.rs`（修改） | 命令分发、确认流程与独占操作 |
| `crates/gqy-gateway/src/routes/commands.rs`（修改） | 服务端命令与确认参数 |
| `apps/gqy/src/cmd/data.rs`（新增） | `data wipe --persona` |
| `tests/memory_reset.rs`（新增） | 代数端到端、删除范围、会话删除语义 |

## 接口草案

草案，以实现为准。

```rust
pub enum ResetScope { Session { session: SessionId }, Persona { persona: PersonaId } }
pub struct ResetReport { pub deleted_rows: u64, pub new_generation: u64 }

/// 单事务：代数 +1 与删除；失败整体回滚。
pub async fn reset(store: &Store, scope: ResetScope, at: Timestamp) -> Result<ResetReport, MemoryError>;

/// 写作业通用守卫（各写入方在事务第一步调用）。
pub fn check_generation(tx: &mut WriteTx, persona: &PersonaId, expected: u64)
    -> Result<(), MemoryError>;   // 不等 → StaleGeneration
```

## 实施步骤

1. 代数字段与删除事务（先写「删除后代数 +1」用例）。
2. 四个命令与确认流程；`/clear` 的客户端实现接 P09-04 的命令表。
3. `data wipe` 的备份 + 删除 + 摘要。
4. 写作业守卫端到端（整理 / 复盘 / 日记 / `remember` 四条路径各一条用例）。
5. 会话删除语义的夹具断言；doctor / 审计接线。
6. `cargo xtask check`、`cargo test -p gqy-memory`；提交：`feat(memory): reset 收敛与记忆代数`。

## 测试与守护

- **代数端到端**：reset 后，先前的整理作业写回、复盘投递、召回登记、`remember` 全部被拒并记 `stale_generation`；去掉任一路径的守卫，对应测试红。
- **物理删除**：`/forget-session` 后九表相关行为 0、倒排清掉、`kb` 不受影响；删除只出现在登记路径（触发器 / 白名单测试）。
- **确认流程**：缺确认（或错误 persona 名）不执行；错误信息列出期望的确认值。
- **`/new` 不删记忆**：新会话后旧记忆仍可召回；旧会话保留可查阅。
- **会话删除语义**：删除会话不影响记忆；已投递的召回块随会话清除（夹具）。
- **`data wipe` 备份**：执行前存在新备份文件；摘要含路径。
- 先红后绿对照（PR 贴输出）：去掉 `check_generation`、去掉确认流程各一次。

## 验收流程

```sh
cargo test -p gqy-memory
# 手检（mock 供应商）：/forget-session 前二次确认 → 记忆清空、代数 +1、审计与日志可见；
#   之后让模型 remember 一条 → 成功（新代数）；对同一 persona 跑 gqy data wipe --persona <id> → 先生成备份
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 四个命令 + `data wipe` 是全部 reset 入口；仓库中不存在 `/wipe` 会话命令（grep 佐证）
- [ ] 代数、物理删除、确认、`/new` 语义、会话删除语义、备份各有区分能力测试
- [ ] 删除路径的白名单（reset / 保留清理）在数据库层可验证
- [ ] Q-11-2 / Q-11-3 推荐已留痕；事件名登记进 13 §6.2（README 复核第 2 条）

## 风险与回退

- **TUI 的命令确认样式**：本单先落最小确认（文本 + 参数），浮层样式随遗留项另开单。
- **备份前提**：`data wipe` 的自动备份依赖 10 §9.1 的实现（P01-08）；若缺，先接最小备份路径并回写 10。
- **回退**：命令与 `data wipe` 可整体下线（数据模型与代数字段保留）；已删除数据只存在于备份中，回退不恢复它们（这是用户显式操作，铁律 9 的语义如此）。
