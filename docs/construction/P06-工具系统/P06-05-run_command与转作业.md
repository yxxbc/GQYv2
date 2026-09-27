# P06-05 · run_command 与转作业

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P06-02 |
| 设计依据 | designs/07-工具系统.md §10（run_command 契约与上限）、§8（8 MiB 捕获、截断与 blob）、§3.3（`ToolEffect::StartJob`）、§13（`[tools.run_command]` 默认值）；designs/03-回合引擎.md §10.1（长命令转作业、`JobRecord` 与固定文案）；designs/02-运行时与并发模型.md §6（前台 120 s 软超时、硬上限、SIGTERM → 2 s → SIGKILL）、§5（取消传播）；designs/08-权限审批与沙盒.md §6.3（`sh -c` 与环境白名单）、§5.3（审批键的分词口径） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-tools`（`builtin/run_command.rs`、`spawn.rs`、`capture.rs`、`shell.rs`、`jobs.rs`）、`crates/gqy-sys`（进程组/信号/CLOEXEC）、`crates/gqy-config`（超时默认值） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

`run_command` 成为可用的执行入口：`sh -c`（非 login shell，08 §6.3）、白名单环境、独立进程组、输出边读边截（8 MiB 捕获上限），软超时（默认 120 s）到期**不杀进程**，转后台作业并立即返回作业句柄。做完之后，长命令不再丢回合（02 §6 的 v1 backlog 关闭）；`yes` 类风暴吃不光内存。

## 范围

- 做：
  - `spawn.rs`：进程组启动；环境白名单照 08 §6.3（`PATH`、`HOME`、`LANG`、`LC_*`、`TERM`、`TZ`、`USER`、`SHELL`、`TMPDIR`、`COLUMNS`/`LINES` + `exec.env_passthrough`；`GQY*` 与密钥一律清除）；`TMPDIR` 指向会话私有临时目录；除 stdio（与 PTY）外 fd 一律 CLOEXEC；终止流程 SIGTERM → 2 s → SIGKILL。
  - `capture.rs`：8 MiB 读取上限，超出继续读并丢弃、只计数（防管道满阻塞）；全文落 blob，`Raw` 通道记引用，`out:<call>` 可回取。
  - 软超时：`timeout_seconds` 1–600（默认 120）；到期转 `ToolEffect::StartJob(JobSpec)` 并返回 03 §10.1 的固定文案（`Command still running after 120s; moved to background as job <id>. …`）；`background: true` 直接转作业。
  - `jobs.rs`：`JobSpec`（命令、cwd、环境、`wake: OnFinish`、硬上限）与 `JobPort` 接缝；真实作业调度归 P13-02（本单测试用替身）。`sandbox` 字段随 P07-05 接入真实类型。
  - `shell.rs`：保守分词与审批键（08 §5.3）——无引号/元字符时按空白分词取 `argv[0] + 第一个非选项子命令`；含 shell 元字符或分词失败时退回完整命令精确匹配。完整 POSIX 词法归 P07-03（原地升级，不留两份）。
  - `assess`：`Exec`、`Command { argv, cwd }` 目标、`approval_key`；`risk` 本单恒 `Normal`（危险识别归 P07-03）。
- 不做：
  - 作业持久化、唤醒、`job_*` 工具与调度（P13-02）；本单只给效果与接缝。
  - 沙盒挂载与判定（P07-05/P07-06、P07-01）、审批流程（P07-02）。
  - PTY（P06-06）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-tools/src/builtin/run_command.rs`（新增） | 工具本体与 `assess` |
| `crates/gqy-tools/src/{spawn.rs,capture.rs,shell.rs}`（新增） | 启动器、捕获器、分词与审批键 |
| `crates/gqy-tools/src/jobs.rs`（新增） | `JobSpec` 与 `JobPort` 接缝（真实实现归 P13-02） |
| `crates/gqy-tools/src/builtin/mod.rs`（修改） | `BUILTINS` 加一行 |
| `crates/gqy-sys/src/proc.rs`（新增/扩展） | 进程组、信号、CLOEXEC、私有 TMPDIR 辅助 |
| `crates/gqy-tools/tests/run_command.rs`（新增） | 见「测试与守护」 |

## 接口草案

草案，以实现为准。

```rust
// gqy-tools/src/spawn.rs
pub struct SpawnSpec { pub argv: Vec<String>, pub cwd: PathBuf, pub env: Vec<(String, String)> }
pub async fn spawn_group(spec: &SpawnSpec, ctx: &ToolContext) -> Result<Child, SpawnError>;
pub async fn terminate_group(child: &mut Child, grace: Duration) -> TerminateOutcome;

// gqy-tools/src/jobs.rs（效果由引擎执行；P13-02 提供真实实现）
pub struct JobSpec { pub command: String, pub cwd: PathBuf, pub wake: WakePolicy,
    pub hard_deadline: Timestamp /*, pub sandbox: SandboxPolicy —— P07-05 接真实类型 */ }
pub trait JobPort: Send + Sync { fn start(&self, spec: JobSpec, child: Child) -> JobHandle; }

// gqy-tools/src/shell.rs（保守版；P07-03 升级为完整 POSIX 词法）
pub fn approval_key(command: &str) -> ApprovalKey;   // 08 §5.3
```

## 实施步骤

（L 单：每一步都能单独编译通过。）

1. `spawn.rs` + `gqy-sys` 原语（先写「终止后进程组全退出」的会红断言）。
2. `capture.rs`（8 MiB 边读边截 + blob + 计数）。
3. `run_command` 前台路径（超时、环境白名单、输出口径）。
4. 转作业：`JobSpec` + `JobPort` 接缝 + 固定文案 + `background: true`。
5. `shell.rs` 审批键与 `assess` 的 targets / effect。
6. `yes` 压力测试与 RSS 断言；`cargo xtask check --fast`、`cargo test -p gqy-tools -p gqy-sys`；提交：`feat(tools): run_command 与转作业`。

## 测试与守护

- **转作业**：后台 `sleep 300`，软超时（TestClock 推）→ 结果含固定文案与作业 id，进程仍在（pid 探测）；`background: true` 立即转；去掉「不杀进程」的路径测试红。
- **硬上限与终止**：硬上限到期 → SIGTERM → 2 s → SIGKILL 后进程组全退出（pid 探测）。
- **取消**：运行取消 → 1 s 内进程组退出（02 §5 口径）。
- **捕获**：`yes` 跑 5 s → daemon RSS 增长 < 32 MiB、捕获停在 8 MiB、计数继续（把上限改成无界，此测试必红）。
- **环境**：子进程打印环境 → 不含任何 `GQY*` 与注入的假密钥；`TMPDIR` 为会话私有目录。
- **参数与审批键**：`timeout_seconds` 边界 1 / 600 / 601；`cargo test -p x` → 键 `cargo test`；含 `|`、`>` 的命令 → 完整字符串精确匹配（08 §5.3）。
- 先红后绿对照（PR 贴输出）：把软超时改回「杀进程」、去掉环境白名单各一次。

## 验收流程

```sh
cargo test -p gqy-tools -p gqy-sys   # 全绿
cargo xtask check --fast
# 手检（临时数据目录 + 测试注入）：跑 `sleep 130` → 120 s 后收到作业句柄文案；
#   跑 `yes` 5 s → 观察 daemon RSS（对照 19 §3.7 的预算）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 软超时转作业不杀进程；硬上限与取消按 02 §6 终止；三条路径各有区分能力测试
- [ ] 8 MiB 捕获、`out:` 回取、`yes` 压力断言生效
- [ ] 环境白名单生效，假密钥不出现（脱敏的四路守护在 P07-07 收口）
- [ ] `JobSpec` 字段与 03 §10.1 的 `JobRecord` 对照记录在 PR 描述里

## 风险与回退

- **与 P13-02 的接缝**：`JobPort` 是过渡物；P13-02 落真实调度时字段只增，`sandbox` 随 P07-05 接入。
- **shell 词法的两阶段**：P07-03 升级 `shell.rs` 时同步升级本单的审批键用例（两处不分叉）。
- **回退**：五个新文件 + `BUILTINS` 删行即可；`gqy-sys` 原语随单 revert。
