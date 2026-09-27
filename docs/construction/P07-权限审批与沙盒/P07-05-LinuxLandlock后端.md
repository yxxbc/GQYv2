# P07-05 · Linux Landlock 后端

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P07-04 |
| 设计依据 | designs/08-权限审批与沙盒.md §6.2（ABI 探测与降级、访问位、规则集全表、硬规则、施加时机与资源限制、不覆盖清单）、§4.1（无后端规则、`Full` 不套沙盒、`unsandboxed:` 前缀）、§8（Landlock 变为不可用、写根被删除）、§9.2（`sandbox.unavailable`）、§10（`[exec]`）、§11（Landlock 实测清单）、§13（Q-08-3 网络不限制）；designs/09-感知矩阵.md §4.1（`HostCap::SandboxFs`、探测刷新、`<host-update>`）；designs/01-总体架构.md §3（`gqy-sys` 是唯一可以 unsafe 的 crate）；designs/12-人格配置与场所.md §3（`Config::validate` 集中校验）；designs/13-网关与API协议.md §8.1（沙盒内 UDS 请求 401） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-sys`（`sandbox/{mod.rs,landlock.rs,probe.rs}`、`proc.rs`）、`crates/gqy-core`（`SandboxAvailability`/`SandboxView`/`SandboxPolicy` 值类型）、`crates/gqy-tools`（`spawn.rs` 接线）、`crates/gqy-engine`（私有 tmp 生命周期、刷新接线）、`crates/gqy-config`（`[exec]` 校验） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

Landlock 真的套在子进程上：`pre_exec` 中建规则集 → `PR_SET_NO_NEW_PRIVS` → `landlock_restrict_self`，失败走 `_exit(126)` + errno 管道。做完之后（内核支持时）：沙盒内 `touch $HOME/x` 失败、`cat <data>/run/local.token` 失败、`ls /proc/self/fd` 只有 0/1/2；写根被删除失败关闭；ABI 不足时降级且 `SandboxView` 如实列出未受控访问类型与 "network: not restricted"。

## 范围

- 做：
  - ABI 探测（`landlock_create_ruleset(NULL, 0, VERSION)` ≤ 0 → `Unavailable{reason}`）与访问位按 ABI 取（1 基础、2 `REFER`、3 `TRUNCATE`、5 `IOCTL_DEV`）；v2 要求 ABI ≥ 1；`CompatLevel::HardRequirement` 口径：缺位降 handled 集合、不报错，但必须列出未受控类型。
  - 规则集照 §6.2 表：写根读写执行建删；会话私有 tmp `<data>/run/tmp/<session>/`（经 `TMPDIR` 传入、会话卸载时清理）；`/dev/null`、`/dev/zero`、`/dev/urandom`、`/dev/tty`（PTY 时）；系统只读目录（存在者保留）；`extra_read_roots` / `extra_write_roots`（审批界面显示）。
  - 不存在根先剔除；**写根不存在时整体失败**（§8 失败关闭）；不在规则集：数据目录全部（`run/`、`config/`、`gqy.db`）、`$HOME` 其余、`/tmp`。
  - 硬规则：`extra_read_roots`/`extra_write_roots` 覆盖 `run/`、`config/` → 配置校验失败并指出字段（`Config::validate`）。
  - 施加时机：路径在 fork 前转 fd/CString；`pre_exec` 中不分配内存；失败经管道回报 errno，父进程报 `error[sandbox_unavailable]: landlock_restrict_self failed: EPERM` 形态。
  - 资源限制：`RLIMIT_CORE = 0`、`RLIMIT_NOFILE = 4096`；`RLIMIT_AS` 默认不设；stdio（与 PTY）以外 fd 一律 `CLOEXEC`，在此路径钉测试。
  - `SandboxView` 产出（审批卡片与 doctor 读取接口共用）；Host 刷新接线：启动全量探测、每 10 分钟复测；从可用变不可用时发 `sandbox.unavailable`（逐会话去重）。
  - `SpawnSpec` / `JobSpec.sandbox` 接真实 `SandboxPolicy`（P06-05 留的字段原地补全）。
- 不做：
  - macOS 与无后端平台的路径（P07-06）、判定函数（P07-01）、命令策略（P07-03）。
  - `<host-update>` 尾部注入与 system 渲染（P11）；`gqy doctor` CLI 本体（P11-05，本单只留读取接口）。
  - 网络限制（Q-08-3：第一版不限制并明文登记）；Seatbelt（D-02 待拍板）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-sys/src/sandbox/{mod.rs,landlock.rs,probe.rs}`（新增） | 探测、规则集构建、`pre_exec` 施加、errno 管道 |
| `crates/gqy-sys/src/proc.rs`（修改） | 资源限制与 CLOEXEC 原语 |
| `crates/gqy-core/src/sandbox.rs`（新增） | `SandboxAvailability`/`SandboxView`/`SandboxPolicy` 值类型（归属先按复核清单定） |
| `crates/gqy-tools/src/spawn.rs`（修改） | `apply_sandbox` 接线、私有 tmp、写根检查 |
| `crates/gqy-engine/src/{session.rs,host.rs}`（修改） | 私有 tmp 生命周期、探测刷新与 `sandbox.unavailable` |
| `crates/gqy-config/src/validate.rs`（修改） | extra roots 覆盖 `run/`、`config/` 的校验 |
| `crates/gqy-sys/tests/landlock.rs`、`crates/gqy-tools/tests/sandbox.rs`（新增） | 见「测试与守护」 |

## 接口草案

草案，以实现为准。

```rust
// gqy-sys/src/sandbox（唯一 unsafe 之处都在本 crate；08 §6.2）
pub fn probe() -> CapState;                                  // → HostCap::SandboxFs
pub struct SandboxPolicy { pub write_root: PathBuf, pub read_roots: Vec<PathBuf>,
    pub exec_roots: Vec<PathBuf>, pub private_tmp: PathBuf, pub pty: bool /* … */ }
pub fn prepare(p: &SandboxPolicy) -> Result<PreparedSandbox, SandboxError>;  // fork 前：fd、CString
pub fn apply_prepared(p: &PreparedSandbox) -> Result<(), SandboxError>;      // pre_exec 内：不分配

// gqy-tools/src/spawn.rs（接线）
pub fn apply_sandbox(cmd: &mut Command, p: &PreparedSandbox) -> Result<(), SpawnError>;

// gqy-core/src/sandbox.rs（若 P07-01 已落位则复用，不建第二处）
pub enum SandboxAvailability { Available { backend: SandboxBackend, abi: u8 }, Unavailable { reason: String } }
pub struct SandboxView { /* backend、abi、写根、未受控访问类型、"network: not restricted" */ }
```

## 实施步骤

（L 单：每一步都能单独编译通过。）

1. 探测与 `CapState`；注入假 ABI 的测试路径先行。
2. 规则集构建与 `prepare`：08 §11 的实测用例先写成会红清单。
3. `pre_exec` 施加、errno 管道与错误文案；资源限制与 CLOEXEC。
4. 私有 tmp 与生命周期；`SpawnSpec` 接线；写根不存在的失败关闭。
5. 配置校验与 Host 刷新接线（`sandbox.unavailable` 逐会话去重）。
6. `cargo xtask check --fast`、`cargo test -p gqy-sys -p gqy-tools -p gqy-engine -p gqy-config`；提交：`feat(perm): Linux Landlock 后端`。

## 测试与守护

- **实测七条**（08 §11）：`touch $HOME/x` 失败、`touch <workspace>/x` 成功、`cat <data>/run/daemon.json` 失败、`cat <data>/run/local.token` 失败、`cat <data>/config/gqy.toml` 失败、`nc -U gqy.sock` 失败或无令牌被 401、`ls /proc/self/fd` 只有 0/1/2；内核不支持时跳过并打印原因，CI 另有一台保证支持的 runner。去掉 `run/`、`config/` 拒绝规则，后三条红。
- **失败关闭**：写根被删 / 施加失败 → 未 spawn（spawn 计数器）且文案含 errno 与期望值。
- **ABI 降级**：注入 ABI 1 → `SandboxView` 出现「cross-directory rename/link not restricted」；去掉降级登记测试红。
- **配置**：extra roots 覆盖 `run/`、`config/` → 校验失败并指出字段。
- **fd 与资源限制**：子进程 fd 快照与 `ulimit -c`/`-n` 值断言。
- 先红后绿对照（PR 贴输出）：删 `PR_SET_NO_NEW_PRIVS`、去掉私有 tmp 各一次。

## 验收流程

```sh
cargo test -p gqy-sys -p gqy-tools -p gqy-engine -p gqy-config   # 全绿（不支持的内核打印跳过原因）
cargo xtask check --fast
# 手检（Linux，内核带 Landlock）：临时数据目录起 daemon，mock 触发 run_command：
#   touch $HOME/x → 失败；touch <workspace>/x → 成功；cat <data>/run/local.token → 失败
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 规则集与 §6.2 表逐行一致；`run/`、`config/` 硬规则有区分能力测试
- [ ] `pre_exec` 不分配内存；失败 `_exit(126)` + errno 管道；CLOEXEC 与资源限制有测试
- [ ] 写根不存在失败关闭；ABI 降级在 `SandboxView` 如实列出未受控类型
- [ ] 探测刷新与 `sandbox.unavailable` 就位；值类型归属只在一处定义（PR 留痕）

## 风险与回退

- **内核差异**：ABI 1 上按降级如实登记，不宣称更强；测试按「跳过并打印原因 + 专用 runner」两档组织（08 §11）。
- **fork 安全**：`pre_exec` 里不做分配是硬约束，评审先看这一段。
- **回退**：`sandbox/` 与接线 revert，探测回退为 `Unavailable{reason}`——判定按 §4.1 无后端规则（Owner 无沙盒 + 审批、Member 拒绝），不引入新的裸奔路径。
