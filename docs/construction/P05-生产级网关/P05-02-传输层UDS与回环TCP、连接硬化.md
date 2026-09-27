# P05-02 · 传输层：UDS 与回环 TCP、连接硬化

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P05-01 |
| 设计依据 | designs/13-网关与API协议.md §3（传输与监听、监听实现基线、超限连接处置）、§3.1（连接与请求上界表）、§8.1（UDS 权限与对端凭据）、§12（socket 残留）；designs/01-总体架构.md §4（单实例锁与监听）；designs/02-运行时与并发模型.md §7（背压）；designs/19-可观测性与测试.md §3.1（span） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-gateway`（新，L5）、`crates/gqy-sys`（`peer` 模块）、`crates/gqy-config`（新键）、`Cargo.toml`（workspace 依赖） |

## 目标

两个监听以**生产形态**立起来：自建 accept 循环（`hyper-util`，官方说明 `axum::serve` 不带配置）、连接硬化（TCP / UDS / HTTP1 三组参数）、连接上限与“达上限暂停 accept”、`GracefulShutdown` 连接跟踪、`ConnectInfo` 与 UDS 对端凭据；UDS 目录/文件权限与旧 socket 清理按 §8.1/§12 执行。做完之后，网关可以带着真实网络栈被压测与攻击（慢头、连接洪峰），行为都是设计里写明的。

## 范围

- 做：
  - `gqy-gateway` crate 骨架（L5；依赖 axum、tower、tower-http、hyper、hyper-util、socket2；**不得**创建 runtime）。
  - `listen.rs`：UDS（`run/gqy.sock`）与回环 TCP（`127.0.0.1:8310`）的 bind、权限、accept 循环；每连接一个任务；`hyper_util::server::conn::auto::Builder` + `hyper_util::server::graceful::GracefulShutdown` 跟踪；`into_make_service_with_connect_info::<SocketAddr>()`。
  - 连接上限：`gateway.http.max_connections`（默认 512，上限 4096）；达上限**暂停 accept**（背压），计数并每分钟一行 `warn`。
  - `conn.rs`：HTTP/1 参数（`header_read_timeout` + `TokioTimer`、`max_headers`、`max_buf_size`、`keep_alive`、`half_close(false)`）、TCP 参数（`TCP_NODELAY`、keepalive、backlog=1024，经 socket2）。
  - `uds.rs`：目录 0700 / socket 0600 的设置与校验；`prepare_socket_path()`（拿锁后清理旧 socket，纯函数，由 P05-10 调用）；对端凭据取用接口。
  - `gqy-sys/src/peer.rs`：`peer_cred()`（Linux `SO_PEERCRED`；macOS `getpeereid`；Windows → `Unsupported`，调用方失败关闭）；unsafe 全部带 `// SAFETY:`。
  - 配置键（默认与上限照 13 §3.1/§13）：`gateway.http.listen/max_connections/header_read_timeout_ms/max_headers`、`gateway.uds.enabled` 等。
  - 最小 `GET /health`（固定 200 `{status:"ok"}`）仅用于自证连通与停机态；正式路由与中间件随 P05-03。
  - 优雅停机接口：`serve(handles, shutdown: CancellationToken)`：收到信号 → 停止 accept → `GracefulShutdown::shutdown()` → 返回；宽限与强制终止归 P05-10。
- 不做：
  - 鉴权（P05-04）、中间件栈与错误契约（P05-03）、业务路由（P05-06/07）、SSE（P05-08）。
  - TLS 加密层（P05-12；本单只在 `conn.rs` 留“连接包装器”接缝）。
  - 单实例锁与 `daemon.json`（P05-10；本单只提供 `prepare_socket_path` 供其调用）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-gateway/Cargo.toml`、`src/lib.rs`（新增） | crate 骨架与模块文档 |
| `crates/gqy-gateway/src/listen.rs`（新增） | accept 循环、连接上限、GracefulShutdown、ConnectInfo |
| `crates/gqy-gateway/src/conn.rs`（新增） | HTTP/1 与 TCP 参数；TLS 包装接缝 |
| `crates/gqy-gateway/src/uds.rs`（新增） | 权限、旧 socket 清理、对端凭据接口 |
| `crates/gqy-sys/src/peer.rs`（新增） | `PeerCred` 与平台实现（Windows 不支持） |
| `crates/gqy-config/src/config.rs`（修改） | 上述新键与上限校验 |
| `crates/gqy-gateway/tests/{slow_header,conn_cap,uds_perms,graceful}.rs`（新增） | 见「测试与守护」 |

## 接口草案

草案，以实现为准。

```rust
// gqy-gateway/src/listen.rs
pub struct GatewayHandles { pub router: axum::Router, pub cfg: Arc<ConfigSnapshot>, pub paths: GqyPaths }
pub async fn serve(handles: GatewayHandles, shutdown: CancellationToken) -> Result<(), GatewayError>;
/// 达上限时暂停 accept；返回前会等所有被跟踪连接结束或 shutdown 触发。
pub struct ConnGuard { /* Semaphore + 计数 */ }

// gqy-gateway/src/uds.rs
/// 拿到 daemon.lock 之后调用：清理上次崩溃留下的 socket 文件。
pub fn prepare_socket_path(path: &Path) -> std::io::Result<()>;

// gqy-sys/src/peer.rs
pub struct PeerCred { pub uid: u32, pub pid: Option<u32> }
pub fn peer_cred(stream: &impl AsRawFd) -> Result<PeerCred, SysError>;   // Windows: Unsupported
```

## 实施步骤

（L 单：每一步都能单独编译通过。）

1. crate 骨架 + `gqy-sys::peer`（先写“Windows 返回 Unsupported”的编译期分支）。
2. UDS 监听：权限、清理、accept 循环、`/health`；手检 `curl --unix-socket`。
3. 回环 TCP 监听 + ConnectInfo + TCP/HTTP1 参数（慢头与连接上限的测试先行）。
4. `GracefulShutdown` 接线与停机路径（含在途请求）。
5. 配置键与上限校验；`cargo xtask check --fast`、`cargo test -p gqy-gateway -p gqy-sys -p gqy-config`；提交：`feat(gateway): UDS 与回环传输、连接硬化`。

## 测试与守护

- **慢头**：连上后只发一半请求头并停顿 → `header_read_timeout` 到点关闭连接，计数 +1（去掉 timer 配置会 panic，测试即红）。
- **连接上限**：把上限调成 2，打满后第 3 个连接不被接受（accept 暂停），日志限频生效；释放一个后恢复。
- **UDS 权限**：目录 0700、socket 0600；权限不对时启动报错而不是继续。
- **旧 socket 清理**：预置一个同名文件 → `prepare_socket_path()` 后可 bind；无锁时**不得**清理（由 P05-10 的调用顺序测试覆盖）。
- **优雅停机**：在途请求完成后连接关闭；停机开始后不再 accept。
- **对端凭据**：同 uid 通过；跨 uid 情形标 `#[ignore]`（需特权，Linux 用 `unshare` 跑）；Windows 编译通过且能力报告为不可用。
- 先红后绿对照（PR 贴输出）：去掉 `header_read_timeout`、把连接上限检查改恒真各一次。

## 验收流程

```sh
cargo test -p gqy-gateway -p gqy-sys -p gqy-config   # 全绿
cargo xtask check --fast
# 手检：起网关，然后
curl --unix-socket <tmp>/run/gqy.sock http://gqy.local/health
curl -v http://127.0.0.1:8310/health        # 期望 200；X-Request-Id 在 P05-03 之后可见
# 手检（慢头）：nc 127.0.0.1 8310 后只发一行请求头，观察连接被关闭与日志计数
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 慢头、连接上限、UDS 权限、优雅停机各有区分能力测试
- [ ] 13 §3.1 表中“传输与连接”相关行全部有实现与测试
- [ ] `gqy-sys::peer` 的 unsafe 均有 `// SAFETY:`；Windows 分支为“不支持 + 失败关闭”
- [ ] 不用 `axum::serve`（PR 描述里说明 grep 证据）

## 风险与回退

- **hyper-util API 演进**：`auto::Builder` 与 `GracefulShutdown` 的接口以固定版本为准，升级单独开单并跑回归。
- **macOS 对端凭据差异**：只做 uid 检查（`getpeereid`），不依赖 pid；文档写明。
- **回退**：`gqy-gateway` 是新 crate，revert 即可；`gqy-sys::peer` 与配置键一并回退。
