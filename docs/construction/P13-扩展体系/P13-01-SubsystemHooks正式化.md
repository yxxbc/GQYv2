# P13-01 · SubsystemHooks 正式化

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P11-06 |
| 设计依据 | designs/18-扩展体系.md §3（契约、字节契约、时限与失败、注册）、§10（钩子 panic / 超长块的边界）、§12（`[hooks]` 默认值）；designs/13-网关与API协议.md §6.2（`hook.skipped`）；designs/04-前缀缓存账本.md §4.2（尾部通道与块位置）、§5（化石化）、§12（字节前缀门禁）；designs/12-人格配置与场所.md §5.2（persona 启用集决定钩子集合）、§5.4（代际冻结）；designs/02-运行时与并发模型.md §2（钩子的阻塞 IO 走 `gqy_core::blocking::run`）、§6（超时上界）；designs/05-上下文与压缩.md §7.4（被逐出条目与折叠记录）；designs/11-记忆与知识库.md §5（Memory / KB 是首个实现，只访问自己的表） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-core`（trait 与值类型）、`crates/gqy-engine`（三个调用点与超时包装）、`crates/gqy-ledger`（`TailBlock` → `Channel::Context`）、`crates/gqy-config`（`[hooks]`）、`crates/gqy-daemon`（组装处注册）、`tests/`（字节契约联测） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

`SubsystemHooks` 从 P12-04 之前的最小接缝正式化为 `gqy-core` 的公开契约：引擎在三个唯一调用点调用钩子（`before_request` 每次请求规划前、`after_turn` 回合终态后、`on_evict` 压缩提交后）；钩子只能返回 `TailBlock`，由 `gqy-ledger` 以 `Channel::Context` 条目写入并在发送前化石化；注入顺序由 `(hook.id 字典序, 块返回顺序)` 决定，与钩子完成的先后无关；超时、失败、panic 都“什么都不注入”，不碰已有前缀；连续 3 回合失败的本会话禁用该钩子并通知 UI。做完之后：mock 供应商下第 N 轮请求是第 N−1 轮的字节前缀延伸；钩子在类型上不可能修改 system 段与工具面（另加 compile-fail 测试钉住）。

## 范围

- 做：
  - 类型与 trait 进 `gqy-core`：`SubsystemHooks`、`HookCx`、`RequestView`、`TailBlock`、`TurnDigest`、`EvictedSpan`、`HookError`（含 `Panicked`）；默认实现返回空结果（18 §3.1）。
  - 引擎三调用点：`before_request` 在每次请求规划前并发调用全部钩子（共享 1.5 s 时限，上限 5 s）；`after_turn` 在终态后由后台任务调用（30 s / 300 s），不阻塞下一回合准入；`on_evict` 在压缩提交后调用（10 s / 60 s），压缩已提交不回滚。
  - 字节契约：块经 `gqy-ledger` 写 `Channel::Context` 并化石化；`Trusted` → trusted tail（system 角色）、`Untrusted` → untrusted tail（user 角色）；`tag` 白名单登记（先登记 `associated-memories`、`kb-hits`、`self-review`）；未登记 tag 丢弃并记 `hook.invalid_block`；`once_per_turn`；上限单块 2 KiB / 单钩子 4 KiB / 全部 8 KiB，超出按块截断并记日志（18 §3.2）。
  - 顺序与确定性：注入顺序只由 id 与块序决定；两个钩子随机延迟完成 100 次，注入顺序恒定。
  - 失败与禁用：超时 / 出错 → `hook.skipped{id, reason}` 事件、不重试；panic 在独立任务内被捕获转 `HookError::Panicked`，绝不 unwind 进引擎；连续 3 回合失败 → 本会话禁用该钩子并通知 UI。
  - 注册：组装处（`gqy-daemon`）按 persona 启用集构造钩子；集合随 persona 代际冻结；`gqy-core` 不依赖 `gqy-store`，子系统存储句柄在组装处注入子系统自身，只访问自己的表（10）。
  - 配置：`[hooks]` 五键（`before_request_timeout_ms` = 1500、`after_turn_timeout_s` = 30、`on_evict_timeout_s` = 10、`max_block_bytes` = 2048、`max_total_bytes` = 8192）落进 `gqy-config` 唯一来源。
- 不做：
  - 记忆与知识库钩子的具体逻辑（P12-04 / P12-07 所有，本单只把接口换成正式版并复跑其测试，不留双路径）；
  - 扩展包提供钩子（Q-18-3 第一版不能）；
  - `after_turn` 内部的作业登记与重试（由钩子自己决定，接口在本单）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-core/src/hooks/{mod.rs,types.rs}`（新增） | trait 与值类型（正式版） |
| `crates/gqy-engine/src/hooks/{mod.rs,runner.rs}`（新增） | 三个调用点、超时与 panic 隔离、失败计数 |
| `crates/gqy-ledger/src/context.rs`（修改） | `TailBlock` 写入 `Channel::Context` 与 tag 白名单 |
| `crates/gqy-config/src/config/hooks.rs`（新增） | `[hooks]` 段 |
| `crates/gqy-daemon/src/assemble.rs`（修改） | 按启用集注册钩子集合 |
| `crates/gqy-memory/src/hooks.rs`（修改） | 适配正式 trait（行为不变） |
| `tests/hook_contract.rs`（新增） | 前缀门禁、顺序、超时、panic、禁用 |

## 接口草案

草案，以实现为准。

```rust
pub trait SubsystemHooks: Send + Sync + 'static { /* 18 §3.1 的原文签名 */ }

pub struct TailBlock { pub trust: BlockTrust, pub tag: &'static str, pub body: String, pub once_per_turn: bool }

/// 引擎侧唯一入口：并发执行 + 统一时限 + 按 (id, 块序) 排序；失败即空注入。
/// 返回的类型只允许是 TailBlock —— system 段与工具面在类型上不可达。
pub async fn collect_before_request(
    hooks: &[Arc<dyn SubsystemHooks>], cx: &HookCx, req: &RequestView, cfg: &HooksConfig,
) -> Vec<(&'static str, TailBlock)>;
```

## 实施步骤

1. 值类型与 trait 进 `gqy-core`；`gqy-memory` 适配（原接缝删除，不留双路径）。
2. 引擎三调用点 + 时限包装 + panic 隔离 + 失败计数（先写「超时后请求照常发出」用例）。
3. `gqy-ledger` 写入与 tag 白名单（先写「未登记 tag 被丢弃」用例）。
4. `[hooks]` 配置；`hook.skipped` 事件接线。
5. 前缀门禁、顺序夹具、compile-fail 测试；`cargo xtask check`；提交：`feat(core): SubsystemHooks 正式化`。

## 测试与守护

- **字节契约**：测试钩子每轮返回不同内容 → 第 N 轮请求仍是第 N−1 轮的字节前缀延伸；钩子改不到 system / 工具面（compile-fail 夹具 + 类型签名）。
- **顺序确定性**：两个钩子随机延迟完成 100 次 → 注入顺序恒为 id 字典序；改成按完成顺序，红。
- **超时**：永不返回的 `before_request` → 1.5 s 后请求照常发出且无该块；去掉超时包装，测试由外层 10 s 判红。
- **panic 隔离**：panic 钩子 → `HookError::Panicked`、按超时处理、引擎不终止；去掉隔离，进程级测试红。
- **禁用**：连续 3 回合失败 → 本会话禁用并通知；成功一次后计数清零有对应用例。
- **大小上限**：超长块被截断并记日志；总量超 8 KiB 时后续块被丢弃的顺序有断言。
- **once_per_turn**：工具循环多请求下，`once_per_turn` 块只在第一个请求注入。
- 先红后绿对照（PR 贴输出）：去掉超时包装、把注入顺序改成完成顺序各一次。

## 验收流程

```sh
cargo xtask check
cargo test -p gqy-core -p gqy-engine -p gqy-ledger
# 手检（mock 供应商 + 测试钩子）：连续 5 轮对话，用 doctor --model-view 对比第 N 轮请求前缀；
#   日志可见 hook.skipped / hook.invalid_block 字段齐全（id、reason、字节数）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 三个调用点全仓各只有一处（审查 + 架构检查）；无第二实现、无第二份超时值
- [ ] 原 P12 接缝路径已删除；记忆钩子相关测试全部照旧通过
- [ ] 字节契约、顺序、超时、panic、禁用、上限、`once_per_turn` 各有区分能力测试
- [ ] `[hooks]` 默认值只在 `gqy-config` 一处，与 18 §12 一致

## 风险与回退

- **P12-04 已按旧接缝写**：接口替换不改行为；若其测试形状不同，先对齐 18 §3.1 再改单方，禁止两边各留一套。
- **`hook.invalid_block` 事件名未登记**：复核清单第 4 条；并入 13 §6.2 前只写日志，不新增未登记事件。
- **回退**：整单 revert 到“无钩子”状态（记忆召回块的注入归 P12 自己决定），前缀契约不受影响。
