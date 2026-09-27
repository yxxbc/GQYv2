# P13-06 · MCP stdio 客户端

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P13-01 |
| 设计依据 | designs/18-扩展体系.md §6.1（范围：只 stdio；initialize / tools/list / tools/call / list_changed）、§6.2（进程与 IO：独立进程组、16 MiB 单消息、stderr 环形缓冲、请求表与 EOF）、§6.3（时限表）、§6.4（健康状态机与重启退避）、§6.5（列举缓存与工具面稳定：`mcp_catalog`、离线 stub、`surface_drift`、描述清洗）、§6.6（每服务器信任与权限：`min_trust` / `effect` / `tool_effects` / `sandbox` / `timeout_call_s`）、§10（非 JSON 行、重名冲突）、§12（`[mcp]`）；designs/07-工具系统.md §4.2（`ToolSource("mcp:<server>")`）、§4.3（前缀命名与冲突）、§5.3（`surface_drift` 不改字节）、§6（stub）；designs/10-存储与数据演进.md §7.6（`mcp_catalog`，`ReplaceWhole`）、§4.4（受控更新）；designs/02-运行时与并发模型.md §6（超时表）、§10（隔离回归）；designs/08-权限审批与沙盒.md §5.3（`approval_key` = 真名）、§6.2（Landlock 与 `sandbox = workspace`）；designs/13-网关与API协议.md §6.2（`ext.mcp_status`） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-ext`（`mcp/`）、`crates/gqy-store`（`mcp_catalog` repo）、`crates/gqy-config`（`[mcp]`）、`crates/gqy-daemon`（来源注册与启动等待）、`apps/gqy`（`ext mcp` 子命令）、`tests/`（假 server 联测） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

MCP stdio 客户端成为 `gqy-ext` 的工具来源：每个 server 一个子进程（独立进程组），一个专用读任务按行读 stdout（单消息 ≤ 16 MiB，超出判协议错误并重启），stderr 进 256 KiB 环形缓冲供 doctor 与日志；请求表 `id → oneshot` 每个自带超时；读任务结束（EOF / 崩溃）时所有待决请求以 `error[upstream]: mcp server <id> exited` 完成——v1“`read_line` 无超时永久占住唯一执行线程”在结构上不存在；健康状态机 `Starting → Ready → Degraded → Restarting → Failed`，退避 1 s → 60 s、10 分钟内 5 次后进 `Failed`（不再自动重启，配置变化或 `gqy ext mcp restart <id>` 才恢复）；工具面**只读持久化列举缓存**（`mcp_catalog`，键 = `blake3(server 配置规范化 JSON)`，不含 secret 值）：server 离线时会话工具面照样包含它的 stub，调用时返回 `unavailable (state=…)`；`list_changed` 或新列举导致 `catalog_hash` 变化只更新目录并对现有会话记 `surface_drift`，不改字节；第三方工具描述按不可信文本清洗（去控制字符、≤ 1,024 字符、schema 深度 ≤ 16 且 ≤ 16 KiB，超限排除并 doctor 报告）。

## 范围

- 做：
  - 协议子集：`initialize`（10 s / 上限 60 s）、`tools/list`（15 s + 5 s 余量）、`tools/call`（60 s / 上限 300 s）、`notifications/tools/list_changed`；取消发 `notifications/cancelled` 且 1 s 后不再等待；resources / prompts 收到即忽略。
  - 进程与 IO：子进程独立进程组；逐行读 stdout；非 JSON 行计数丢弃，> 100 行/分钟判 `Degraded`（许多 server 在 stdout 打日志）；`sandbox = workspace` 时进程在 08 的 Landlock 策略下启动，`none` 只允许 Owner 配置且 doctor 警告。
  - 健康与重启：状态机与`ext.mcp_status` 事件（字段照 13 §6.2）；失败列举 60 s 内不重试，之后按退避；daemon 启动最多等 3 s（`startup_wait_ms`），随后后台回填目录。
  - 列举缓存：`mcp_catalog` 读写（`ReplaceWhole`，派生数据）；工具面编译只读该目录；首次从未成功列举的 server 不进已有会话；离线 / `Restarting` 时 stub 调用错误含 server 与 state。
  - 命名与冲突：`mcp_<server>_<tool>`（非法字符替换 `_`、截 64 字符、截断冲突追加 blake3 前 6 hex，07 §4.3）；与内置重名不覆盖、记日志与 doctor。
  - 描述清洗与排除；排除项进 doctor（原因可见）。
  - 配置：`[mcp]` 五键与 `[[mcp.servers]]`（`command` / `args` / `env`（secret 引用，值不进日志与模型可见文本）/ `min_trust` / `effect`（默认 `exec`）/ `tool_effects` / `sandbox`（`none | workspace`）/ `timeout_call_s`）。
  - `apps/gqy ext mcp {list,status,restart,logs}` 最小 CLI（`logs` 显示 stderr 环形缓冲尾部）。
- 不做：
  - HTTP（streamable HTTP）传输（Q-18-5）；
  - resources / prompts 与 server 提供钩子；
  - Web / TUI 的 MCP 管理界面（CLI 最小集够用，界面另开单）；
  - 沙盒内 MCP 进程的网络限制（`network` 无法强制，如实写）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-ext/src/mcp/{mod.rs,client.rs,health.rs,catalog.rs,untrusted.rs}`（新增） | 客户端、状态机、缓存、清洗 |
| `crates/gqy-store/src/repo/mcp.rs`（新增） | `mcp_catalog` |
| `crates/gqy-config/src/config/mcp.rs`（新增） | `[mcp]` 与 `[[mcp.servers]]` |
| `crates/gqy-daemon/src/assemble.rs`（修改） | 来源注册与启动等待 / 回填 |
| `apps/gqy/src/ext/mcp.rs`（新增） | `list` / `status` / `restart` / `logs` |
| `tests/mcp_client.rs`（新增） | 假 server、隔离、目录稳定、清洗 |

## 接口草案

草案，以实现为准。

```rust
pub enum McpState { Starting, Ready, Degraded, Restarting, Failed }

pub struct McpServerRuntime { /* 子进程、请求表、健康状态、stderr 环形缓冲 */ }
impl McpServerRuntime {
    pub async fn call_tool(&self, tool: &str, args: serde_json::Value, deadline: Instant)
        -> Result<CallResult, McpError>;
    pub fn state(&self) -> McpState;
}

/// 清洗后的条目才允许进入工具面；超限返回 None 并记录原因（doctor 可见）。
pub fn sanitize_tool(tool: RawMcpTool) -> Option<SanitizedTool>;
```

## 实施步骤

1. 客户端骨架：spawn、`initialize`、请求表、每请求超时（先写「假 server 永不回复 → 超时且会话 B 不受影响」用例）。
2. 读任务、EOF 完成待决请求、非 JSON 行计数与 `Degraded`。
3. 健康状态机、重启退避、`Failed` 与 CLI `restart`。
4. `mcp_catalog` 与工具面（离线 stub、`surface_drift`、首列举只影响之后的会话）。
5. 描述清洗与命名；`ext.mcp_status` 事件；CLI `logs`。
6. `cargo xtask check`；提交：`feat(ext): MCP stdio 客户端`。

## 测试与守护

- **隔离**：假 server 读 stdin 后永不回复，会话 A 调用它时会话 B 完成 10 个回合（02 §10 的 MCP 版本）；调用按测试配置的 1 s 超时返回。
- **读任务结束**：kill 子进程 → 全部待决请求以 `mcp server <id> exited` 完成；去掉，红。
- **目录稳定**：server 离线时创建会话 → 工具面含持久化目录 stub；server 返回不同目录后现有会话 `surface_hash` 不变；改成即时重编，红。
- **缓存键**：只改 secret 值 → `config_hash` 不变（secret 被写进键，红）。
- **清洗**：控制字符、超长、深层 schema 三例分别被清洗 / 截断 / 排除；排除项 doctor 可见。
- **重启上界**：10 分钟内 5 次后进 `Failed` 且不再自动重启；退避序列 1 → 60 s 有断言。
- 先红后绿对照（PR 贴输出）：去掉每请求超时（测试外层 10 s 判红）、去掉 secret 替换各一次。

## 验收流程

```sh
cargo xtask check
cargo test -p gqy-ext
# 手检：配一个真 MCP server（或测试假 server）→ 新会话工具面出现 mcp_*；
#   kill server → doctor 与 ext.mcp_status 显示状态；重启后调用返回 unavailable（state=…）
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] v1“列举拖垮启动”回归：不可达 server 不阻塞 daemon 就绪（启动等待 ≤ 3 s，测试钉住）
- [ ] 离线 / `Failed` 时既有会话字节不变；stub 调用错误文案含 server 与 state
- [ ] 清洗、缓存键、隔离、退避各有区分能力测试
- [ ] `[[mcp.servers]]` 默认值只在 `gqy-config`；`sandbox = none` 的 doctor 警告可见

## 风险与回退

- **Q-18-6（默认沙盒）**：拍板前 `none` 为默认 + doctor 警告；不得擅自改成强制 `workspace`（许多 server 需要读家目录配置）。
- **server 行为漂移**：以实测为准；非 JSON 行容忍与 `Degraded` 阈值照 18 §10 写进配置，不在代码里散落常数。
- **回退**：停注册 `mcp:*` 来源即可；目录行保留（派生数据，可重建）。
