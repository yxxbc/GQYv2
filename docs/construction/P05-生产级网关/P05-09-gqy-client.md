# P05-09 · gqy-client

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P05-08 |
| 设计依据 | designs/13-网关与API协议.md §3（UDS 与 TCP 同一接口、Windows 回退）、§4（协议版本匹配与 426）、§8.1（读 `run/local.token`）、§10（DTO 复用）、§12（协议不匹配、断开）；designs/01-总体架构.md §3（L5，客户端只依赖 `gqy-protocol`）、§4（`run/daemon.json`）；designs/02-运行时与并发模型.md §6（超时上界） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-client`（新，L5）、`docs/designs/01-总体架构.md`（L5 `gqy-client` 行补依赖列） |

## 目标

把“连 daemon”收口成一个库：UDS 与回环 HTTP 走**同一接口**（同一套 HTTP 协议与 `gqy-protocol` 类型）、自动读 `run/local.token`、四类超时、类型化错误（`ApiError` 原样上抛）、SSE 解析（复用协议类型）、重连退避；Windows 上退化为“回环 HTTP + 本地令牌”（UDS 不可用时失败关闭，不降级成无凭据）。

## 范围

- 做：
  - crate 骨架（L5；依赖 `gqy-protocol` 与 HTTP 客户端；`01 §3` 的 L5 `gqy-client` 行补依赖列）。
  - 连接发现：`run/gqy.sock`（UDS，Linux/macOS）与 `run/daemon.json`（端口、协议大版本）；大版本不匹配 → 明确错误（`ProtocolMismatch`，对应 `gqy ask` 退出码 2）。
  - 统一接口：`Client::get/post/patch/delete` + `Client::events()`（SSE 流）；请求/响应类型一律 `gqy-protocol` 的 DTO。
  - 凭据：读 `run/local.token`；请求带 `Authorization: Bearer`；daemon 重启导致 401 `missing_token` → **重读文件并重试一次**；仍失败则如实报错。
  - 超时（可配置、有上界）：建连 5 s、非流式请求总时长（调用方给，默认 30 s）、SSE 首字节 30 s、SSE 空闲容忍 2×心跳（30 s）。
  - 错误：`ClientError::Api(ApiError)`（解析错误体，含 `request_id`）、`ClientError::Transport`、`ClientError::ProtocolMismatch`；`Display` 写清期望与实际。
  - SSE：解析 `id/event/data` 三行帧与注释行（心跳）→ `Event`（`gqy-protocol::parse_envelope`）；向上层暴露 `last_event_id`。
  - 重连退避：指数 + 抖动、封顶 30 s（供 P05-11/P05-13/桌面复用）；`resync_required` 的辅助流程（拉 `view` → 以 `as_of_seq` 重连）作为可选帮助函数。
- 不做：
  - CLI/TUI 的语义层（P05-11/P05-13）；`gqy ask` 的退出码（P05-11）。
  - mesh 与连接器（P14/P17）；桌面内嵌模式（P16）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-client/Cargo.toml`、`src/lib.rs`（新增） | crate 骨架与统一接口 |
| `crates/gqy-client/src/{conn.rs,http.rs,sse.rs,retry.rs,discover.rs}`（新增） | 连接、请求、SSE、退避、发现 |
| `crates/gqy-client/tests/{contract,replay_token,timeouts,sse_parse}.rs`（新增） | 见「测试与守护」 |
| `docs/designs/01-总体架构.md`（修改） | L5 `gqy-client` 行的允许依赖列 |

## 接口草案

草案，以实现为准。

```rust
// gqy-client/src/lib.rs
pub struct Client { /* 连接 + 凭据 + 超时 */ }
impl Client {
    pub async fn connect(paths: &GqyPaths, opts: ClientOptions) -> Result<Self, ClientError>;
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ClientError>;
    pub async fn post<B: Serialize, T: DeserializeOwned>(&self, path: &str, body: &B) -> Result<T, ClientError>;
    pub fn events(&self, session: &SessionId, after: Option<EventSeq>) -> impl Stream<Item = Result<Event, ClientError>>;
}
```

## 实施步骤

1. 发现 + 连接 + 令牌读取（先红：401 重读重试）。
2. 请求接口与错误映射；协议大版本检查。
3. SSE 解析（帧拼接、注释行、`last_event_id`）。
4. 退避与 resync 帮助函数。
5. `cargo xtask check --fast`、`cargo test -p gqy-client -p gqy-gateway`；提交：`feat(client): gqy-client 传输与协议客户端`。

## 测试与守护

- **契约**：测试内起真实网关（UDS + 回环）→ `get/post/patch/delete` 与 DTO 类型往返；响应错误体解析为 `ApiError`。
- **令牌重读**：模拟 daemon 重启（换令牌）→ 第一次 401，客户端重读后成功（断言只重试一次）。
- **协议不匹配**：伪造 `daemon.json` 的 major=2 → `ProtocolMismatch`（含期望/实际）。
- **超时**：短值下建连/首字节超时各自报 `Transport`；SSE 空闲 30 秒（TestClock）不误杀（心跳容忍）。
- **SSE 解析**：分帧拼接（半个帧滚进来）、`: ping` 忽略、`data` 多行合并、未知 `kind` → `Event::Unknown`。
- **退避**：序列与抖动上界（TestClock）；封顶 30 s。
- Windows：编译通过，UDS 路径不可用时明确报错（`cfg` 排除 UDS 测试）。
- 先红后绿对照（PR 贴输出）：去掉重读重试、把心跳行当数据行各一次。

## 验收流程

```sh
cargo test -p gqy-client -p gqy-gateway   # 全绿
cargo xtask check --fast
# 手检：examples/ 下的小程序连本地 daemon 打印 /info 与 10 条事件（需 P05-10 起 daemon）
cargo run -p gqy-client --example info
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] UDS 与 TCP 走同一接口（同一测试同时覆盖两条传输）
- [ ] 401 重读重试、协议不匹配、四类超时、SSE 解析各有测试
- [ ] `01 §3` 的 L5 行已补依赖列（改动清单内）

## 风险与回退

- **HTTP 客户端选型**：与 `gqy-provider` 同族的客户端实现（rustls）；引入新依赖需同时更新 `01 §3`。
- **Windows 口径**：UDS 不可用 → 回环 + 令牌，能力缺失失败关闭；不为它做完整传输设计（00 §5）。
- **回退**：新 crate，revert 即可；`01 §3` 的依赖列同步回退。
