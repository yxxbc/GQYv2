# P05-10 · daemon 生命周期与优雅停机

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P05-02、P04-07 |
| 设计依据 | designs/13-网关与API协议.md §3（监听与停机顺序）、§5.1（`/daemon/stop`）、§12（停机行与 SSE 顺序）；designs/01-总体架构.md §4（进程形态、单实例锁、`run/daemon.json`）；designs/02-运行时与并发模型.md §9（停机五步）；designs/03-回合引擎.md §13（启动恢复）；designs/10-存储与数据演进.md §4.1（`run/` 布局）、§4.2（WAL 与降级）；designs/19-可观测性与测试.md §3.2（日志输出与轮转） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-daemon`（新，L6）、`apps/gqy`（子命令）、`crates/gqy-store`（排空与 checkpoint 调用）、`crates/gqy-sys`（信号与进程探测） |

## 目标

`gqy daemon run/start/stop/status/logs` 可用且语义明确：单实例锁（`flock`）+ `run/daemon.json`（pid、启动时间、协议版本、监听地址）+ 存活判断（锁 + UDS `ping`）；`gqy-daemon` 的**唯一组装函数**把 store/provider/engine/gateway 接起来；SIGINT/SIGTERM 触发完整停机编排；启动顺序固定（锁 → 清旧 socket → bind → `recover()` → 接受）；日志文件按 19 §3.2 接线。

## 范围

- 做：
  - `gqy-daemon::assemble_daemon(paths, cfg) -> DaemonHandle`（L6 库；不创建 runtime；`apps/gqy` 与桌面宿主共用）。
  - 子命令：
    - `gqy daemon run`：前台运行（服务管理器使用）。
    - `gqy daemon start`：后台拉起 `daemon run`（新会话/脱离终端），写 `daemon.json` 由 daemon 自己完成；start 只负责等待“可连”。
    - `gqy daemon stop`：经 UDS `POST /daemon/stop`（202）→ 等进程退出（有超时）→ 超时给出手工提示。
    - `gqy daemon status`：锁 + UDS `ping` + `daemon.json` 展示（版本、协议、端口、启动时间）。
    - `gqy daemon logs`：tail `logs/daemon.*.jsonl`（可 `--follow`）。
  - 停机编排（顺序固定）：停接受新连接 → 发 `daemon.stopping`（全局与会话流）→ 关 SSE（`gateway.drain_ms`，默认 5000 ms）→ `GracefulShutdown` 等连接 → P04-07 的 `shutdown(deadline)`（运行取消、宽限、强制终止子进程）→ store 写队列排空 + WAL checkpoint（P01-04 的接口）→ 删 `run/daemon.json`、释放锁、清理 socket、退出码 0；超宽限强制退出 → **非零**退出码。
  - 启动顺序：拿锁（失败 → 非零 + 写清锁与 pid）→ `prepare_socket_path`（P05-02）→ bind → `recover()`（P04-07；网关开始接受之前）→ accept；恢复摘要日志一行。
  - 日志：tracing 初始化（stderr 人读 + `logs/daemon.YYYY-MM-DD.jsonl` + `logs/requests.YYYY-MM-DD.jsonl`；非阻塞写线程与有界队列 8192，丢弃计数每分钟一行 warn）。
- 不做：
  - 桌面宿主与 Tauri（P16）、连接器进程监管（P14）、服务管理器安装文件（P16 的发布单）。
  - `gqy doctor`（P11-05）；Windows 的锁与信号细节只要求编译（00 §5）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-daemon/{Cargo.toml,src/lib.rs,src/assemble.rs}`（新增） | 唯一组装函数与生命周期 |
| `crates/gqy-daemon/src/{lock.rs,daemon_json.rs,shutdown.rs}`（新增） | 锁、`daemon.json`、停机编排 |
| `apps/gqy/src/cmd/daemon.rs`（新增） | 五个子命令 |
| `apps/gqy/src/logging.rs`（新增） | 19 §3.2 的日志初始化 |
| `crates/gqy-store/src/lib.rs`（修改） | 排空与 checkpoint 的公开调用（若 P01-04 未暴露） |
| `crates/gqy-daemon/tests/{single_instance,lifecycle,shutdown}.rs`（新增） | 见「测试与守护」 |

## 接口草案

草案，以实现为准。

```rust
// gqy-daemon/src/assemble.rs
pub struct DaemonHandle { pub shutdown: ShutdownTrigger, pub status: Arc<Mutex<DaemonStatus>> }
pub async fn assemble_daemon(paths: GqyPaths, cfg: Arc<Config>) -> Result<DaemonHandle, DaemonError>;

// gqy-daemon/src/shutdown.rs
pub struct ShutdownTrigger { /* CancellationToken */ }
impl ShutdownTrigger {
    pub async fn stop(&self) -> ShutdownOutcome;   // 幂等：重复调用返回同一结果
}
```

## 实施步骤

（L 单：每一步都能单独编译通过。）

1. 锁与 `daemon.json`（先红：双实例被拒；stale 锁恢复）。
2. 组装函数把已有组件接起来（store → engine → gateway；mock 供应商）。
3. 启动顺序与 `recover()` 接线；`/daemon/stop` → 编排。
4. 子命令（run/start/stop/status/logs）与退出码。
5. 日志初始化与轮转（19 §3.2）。
6. `cargo xtask check --fast`、`cargo test -p gqy-daemon -p gqy-store`；提交：`feat(daemon): 生命周期、组装与优雅停机`。

## 测试与守护

- **单实例**：第二个 `start` → 非零退出，错误里写明锁文件与持有者 pid；锁可用时正常。
- **stale 恢复**：预置锁文件与旧 socket（模拟崩溃）→ 启动成功；`recover()` 的摘要日志一行。
- **停机**（02 §9、13 §14）：SIGTERM → 运行以 `Shutdown` 取消；SSE 先收 `daemon.stopping` 再关闭；`daemon.json` 删除、锁释放、退出码 0。
- **超宽限**：构造长任务 → 宽限到期强制退出，退出码非零，子进程按流程终止（fake terminator 断言）。
- **`/daemon/stop`**：202 → 进程退出；重复 stop 不报错。
- **日志**：两种文件都生成；`requests.*.jsonl` 只含 `gqy::usage` / `gqy::context_rewrite`；丢弃计数在队列打满时出现（有界队列 + 慢磁盘模拟）。
- **重启可连**：停机再启动，UDS 与 TCP 都可连；旧令牌失效（P05-04 断言复用）。
- 先红后绿对照（PR 贴输出）：去掉 `daemon.json` 删除、把宽限超时改成静默成功各一次。

## 验收流程

```sh
cargo test -p gqy-daemon -p gqy-store   # 全绿
cargo xtask check --fast
# 手检
GQY2_HOME=$(mktemp -d) cargo run -p gqy -- daemon start && cargo run -p gqy -- daemon status
cargo run -p gqy -- daemon logs | tail -5
GQY2_HOME=... cargo run -p gqy -- daemon stop && cargo run -p gqy -- daemon status   # 期望：未运行
echo $?   # stop 后 status 的退出码与文案按实现说明（未运行 → 非零 + 提示）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 单实例、stale 恢复、SIGTERM 停机、超宽限、`/daemon/stop` 各有区分能力测试
- [ ] 启动顺序与停机顺序与 13 §3 / 02 §9 一致（测试断言顺序或用日志时间戳断言）
- [ ] 日志两文件与轮转按 19 §3.2；丢弃计数可见
- [ ] 只有 `apps/gqy` 创建 runtime（02 §2 的静态规则不变）

## 风险与回退

- **后台化的平台差异**：`start` 用 `std::process::Command` + 新会话；Windows 只保证编译（00 §5）。
- **锁的语义**：`flock` 在本地文件系统成立；网络文件系统不在支持范围（写进模块文档）。
- **回退**：`gqy-daemon` 是新 crate；子命令与日志初始化 revert 即可。
