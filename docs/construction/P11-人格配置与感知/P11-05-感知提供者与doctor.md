# P11-05 · 感知提供者与 doctor

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P11-04 |
| 设计依据 | designs/09-感知矩阵.md 全文：§3（`PerceptionFrame`、`PerceptionProvider`、`Fact<T>`、`Visibility`）、§4.1（Host 与 `HostCap`）、§4.2（Workspace）、§4.3（Origin 与 `safe_prompt_field`）、§4.4（Context）、§5（位置、冻结与化石化规则）、§6（`gqy doctor`）、§7（边界与失败模式）、§8（测试与守护）、§9（配置项）；designs/12-人格配置与场所.md §8（`environment` 节的来源）、§10（system 确定性）；designs/01-总体架构.md §3（`gqy-sys` 边界）、§4（`gqy doctor` 子命令）；designs/08-权限审批与沙盒.md §6.4–§6.5（能力查询、`cfg` 扫描规则）、§8（`sandbox.unavailable`）；designs/02-运行时与并发模型.md §4（`TurnContext.perception`）、§6（探测上界）；designs/00-设计理念.md §5（`gqy doctor` 体检项：P11-05 落地）、§6（Q-09-1、Q-09-2、Q-09-3） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-perception`（新 crate：探测编排、渲染、doctor 数据）、`crates/gqy-sys`（探测后端）、`crates/gqy-config`（`[perception]` 默认值）、`crates/gqy-engine`（`TurnContext.perception` 冻结）、`apps/gqy`（`doctor` 子命令） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

“系统知道什么”与“模型看到什么”拆开：四个维度（Host / Workspace / Origin / Context）先成为类型化事实（`Fact<T>` + `Source` + `Visibility`），再经唯一渲染函数变成模型可见文本；同一回合内尾部字节恒定，注入过的化石化。`gqy doctor` 打印同一份事实（含 `--json` 与 `--model-view`），与模型看到的字节同源。做完之后：探测都有上界（单项 500 ms / 总 2 s），超时字段 `Unknown` 而回合照常；`Visibility::SystemOnly` 字段不会出现在渲染输出里；不可信字段经唯一的 `safe_prompt_field` 转义。

## 范围

- 做：
  - `gqy-perception` 新 crate：`PerceptionFrame`、`PerceptionProvider` trait、`Fact<T>` / `Known` / `ProbeSource` / `Visibility`；`safe_prompt_field`（去控制字符与双向覆盖字符、XML 转义、截断上限、空值 `(empty)`）。
  - Host：字段与来源照 09 §4.1（`os_family` / `arch` / `os_name`、`shell`、`package_managers`、沙盒、PTY、trash、增强命令、CPU / 内存；`kernel`、`path_guard` 仅系统）；`HostCap` 与 `CapState{Available|Degraded|Missing}`；刷新策略（启动全量、10 分钟后台、doctor 即时；刷新只影响新会话，关键能力变化 → 下一回合一次 `<host-update>`）。
  - Workspace：`root` / `read_roots` / `write_roots` / `mode` / `project_kind` / `vcs`（读 `.git/HEAD`，零 spawn）/ `cwd`；探测只读、单项 200 ms、不遍历目录树；`/cd` 与模式切换的差异块判定走账本内容哈希，不靠内存标志。
  - Origin：字段照 09 §4.3 由入口构造；不可信字段（昵称、群历史、附件文本）与可信字段分通道渲染。
  - Context：`now`（分钟精度 + 时区偏移）、`cwd`、`jobs_finished`、`occupancy`（仅系统）、`budget`、`hook_blocks`。
  - 渲染（09 §5.4）：唯一函数 `render`；固定属性顺序、XML 转义、时间格式 `YYYY-MM-DDTHH:MM±HH:MM`；字节预算（`<host>` ≤ 600、`<workspace>` ≤ 400、trusted 感知 ≤ 300、runtime ≤ 120）与确定性丢弃规则。
  - `gqy doctor`：文本与 `--json`（字段即 `HostFacts` / `WorkspaceFacts` 序列化）、`--model-view`（调同一渲染函数）、退出码 0 / 1 / 2；daemon 运行中并排显示两边快照并标差异；体检项接入可用部分：配置校验（P11-01）、沙盒可用性、DB 完整性（P01-08 的工具）、日志降级（19 §3.2）、网络与供应商建连延迟最小抽样（有上界、`--no-net` 可关）。
  - `[perception]` 默认值进 `gqy-config`（09 §9 全键；`runtime_tail = false` 时 doctor 给警告）；能力查询纪律：业务 crate 只问 `HostFacts::has` / `state`，不写 `cfg!(target_os = …)`（08 §6.5 的既有扫描规则接上核对）。
- 不做：
  - 网络与供应商延迟的深度报告（最小抽样即可；版本一致性体检在 P16 另接）；`--gc` / `--deep` 本身（P01-08 已有，doctor 只汇总调用）；system 段组装（P11-06；本单只提供 `environment` 节的数据与渲染函数）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-perception/src/{frame.rs,fact.rs,sanitize.rs}`（新增） | 数据模型、`Fact<T>`、`safe_prompt_field` |
| `crates/gqy-perception/src/{host.rs,workspace.rs,origin.rs,context.rs}`（新增） | 四维提供者与刷新任务 |
| `crates/gqy-perception/src/render.rs`（新增） | 唯一渲染函数与字节预算 |
| `crates/gqy-sys/src/probe/*`（扩展） | 探测原语（uname、os-release、sysctl、PATH 查找、能力试开） |
| `crates/gqy-config/src/config/perception.rs`（新增） | `[perception]` 默认值 |
| `crates/gqy-engine/src/turn.rs`（修改） | 回合开始冻结 `PerceptionFrame` 并挂进 `TurnContext` |
| `apps/gqy/src/cmd/doctor.rs`（新增） | `gqy doctor` 子命令 |
| `tests/perception_render.rs`（新增） | 确定性、哨兵、转义用例 |

## 接口草案

草案，以实现为准。

```rust
pub struct PerceptionFrame { pub host: Arc<HostFacts>, pub workspace: Arc<WorkspaceFacts>,
    pub origin: OriginFacts, pub context: ContextFacts, pub frozen_at: Timestamp }
pub struct Fact<T> { pub value: Known<T>, pub source: ProbeSource,
    pub visibility: Visibility, pub probed_at: Timestamp }
pub enum CapState { Available { detail: String }, Degraded { reason: String, effect: String },
    Missing { reason: String, effect: String } }

pub trait PerceptionProvider: Send + Sync {
    fn dimension(&self) -> Dimension;
    /// 内部逐项超时；超时字段为 `Unknown{reason}`，不阻塞回合。
    fn probe<'a>(&'a self, req: &'a ProbeRequest) -> BoxFuture<'a, ProbeResult>;
}

/// 唯一渲染函数；doctor 的 `--model-view` 调它（09 §5.4）。
pub fn render(frame: &PerceptionFrame, part: RenderPart) -> String;
```

## 实施步骤

1. 数据模型与 `safe_prompt_field`（先写昵称伪造标签 / 双向覆盖字符用例）。
2. Host 与 Workspace 探测（`gqy-sys` 原语）+ 能力状态表；Windows CI 的 `Missing{reason}` 用例。
3. Origin 与 Context；`now` 分钟精度与回合内复用。
4. `render` 与字节预算、确定性丢弃；可见性哨兵测试。
5. 回合冻结接线；`<host-update>` / `<workspace-update>` / `<mode-update>` 的“只出现一次”判定接账本（09 §5.3）。
6. `gqy doctor` 文本 / `--json` / `--model-view` 与退出码；`[perception]` 默认值落地；`cargo xtask check`、`cargo test -p gqy-perception`；提交：`feat(perception): 四维感知与 gqy doctor`。

## 测试与守护

- **确定性**：固定 `HostFacts` / `WorkspaceFacts` / Origin / Clock → 渲染字节恒定；形状夹具覆盖 TUI owner、QQ 群 external、timer internal、子代理四种场所（去掉某节渲染的规范化，红）；Windows 单元测试产出完整能力表（缺项为 `Missing{reason}` 而非 panic / `Unknown`），渲染快照与三平台共用夹具输入。
- **回合内不变**：mock 工具循环 5 个请求，尾部感知字节 5 次相同且 `now` 等于第一次；改成每请求取时间，红。
- **可见性哨兵与转义**：给全部 `SystemOnly` 字段填哨兵，断言渲染输出不含哨兵（把某字段改成 `ModelVisible`，红）；标签伪造、双向覆盖、超长昵称逐条断言转义结果。
- **探测有上界**：注入永不返回的探测 → 回合在 500 ms 超时后开始，字段为 `Unknown`；去掉超时，红。
- **不重复注入**：daemon 重启后同一 `<host-update>` 不再出现（依赖账本判定，不靠内存）。
- 先红后绿对照（PR 贴输出）：去掉单项超时、把 `now` 改为每请求刷新各一次。

## 验收流程

```sh
cargo test -p gqy-perception
GQY2_HOME=$(mktemp -d) cargo run -p apps/gqy -- doctor              # 文本体检
GQY2_HOME=$(mktemp -d) cargo run -p apps/gqy -- doctor --json | head # 机器可读
GQY2_HOME=$(mktemp -d) cargo run -p apps/gqy -- doctor --model-view  # 模型可见字节
# 手检（需用户环境）：Linux 显示 landlock abi；macOS 显示 sandbox missing 的后果文案；断网时仍在上界内结束
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] `gqy doctor` 文本 / `--json` / `--model-view` 同源（`--model-view` 调同一渲染函数）
- [ ] 探测上界、可见性哨兵、转义、回合内不变、重启不重复各有区分能力测试
- [ ] `cfg!(target_os)` 只出现在 `gqy-sys`（既有扫描规则核对通过）
- [ ] `[perception]` 默认值与 09 §9 一致，超范围值被 `validate()` 拒收

## 风险与回退

- **不同环境的探测值不同**（容器、CI、服务进程 PATH 不同）：doctor 并排显示 daemon 与 CLI 两侧结论（09 §6），测试只断言夹具输入下的输出。
- **macOS 沙盒状态（D-02）**：`SandboxFs = Missing` 的后果文案与 08 §4.1 一致，不发明第二种说法。
- **回退**：`gqy-perception` 可整体 revert；引擎退回最小 `<runtime>` 块（时间 + cwd），doctor 退回 P01 形态。
