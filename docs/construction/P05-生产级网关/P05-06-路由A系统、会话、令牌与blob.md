# P05-06 · 路由 A：系统、会话、令牌与 blob

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P05-04 |
| 设计依据 | designs/13-网关与API协议.md §5.1（系统）、§5.2（会话）、§5.8（鉴权令牌与 blob）、§5 约定（游标分页、时间与 ID 格式）、§7（错误体）、§12（边界表）；designs/10-存储与数据演进.md §6（blob）、§7.2（sessions）、§7.5（attachments）；designs/04-前缀缓存账本.md §9.1（视图与 `Display` 投影）；designs/02-运行时与并发模型.md §3.2（`Snapshot` 命令）；designs/03-回合引擎.md §5.1（队列，`/view` 字段） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-gateway`（`routes/{system,sessions,tokens,blobs,audit}.rs`）、`crates/gqy-store`（blob 引用查询）、`crates/gqy-engine`（`SessionSummary`/`SessionView` 映射） |

## 目标

第一组业务路由可用：`/health`、`/info`、`/daemon/status`、`/daemon/stop`（仅 UDS）、会话全套（列表/创建/详情/改/删/`view`/`entries`/`ops`）、`/auth/tokens*`、`/blobs*`、`/audit` 骨架。权限（O/M）、分页、`?confirm`、blob 引用检查都按 13 执行；**未就位的操作不返回假数据**。

## 范围

- 做：
  - `/info`：版本、`protocol`、`features`、构建 ID；M 不回显数据目录路径（§5.1）。
  - `/daemon/status`：已加载会话数、活动运行、store 写队列深度、信号量占用、限速指标、日志降级标记（19 §3.6 字段；不可得的字段用 `null`，不编造）。
  - `/daemon/stop`：**仅 UDS**；调 P05-10 的 `ShutdownTrigger`，返回 202；在 HTTP 监听上不注册该路由（404）。
  - `/sessions`：列表（`?venue=&archived=&q=`，游标分页；M 只见自己 principal）与创建（`CreateSession`；persona/场所/工具面在此刻快照冻结——完整 persona 归 P11，本单用最小 persona 配置并在响应里如实反映）。
  - `/sessions/{id}`（`SessionSummary`）、`PATCH`（标题、归档、会话级模型覆盖）、`DELETE`（需 `?confirm=<id>`；软删 + 保留期归 10）。
  - `/sessions/{id}/view`：走 actor `Snapshot`（P04-02），含 `as_of_seq`（P04-06）。
  - `/sessions/{id}/entries`（`?before_seq=&after_seq=&limit=`；只 `Display` 投影）与 `/entries/{seq}`（完整显示内容；工具输出截断规则在 P06 前透传全量，代码留截断钩子）。
  - `/sessions/{id}/ops`：`ExclusiveOp` 提交；忙时 409 `Busy`；**未就位的 op 返回 400 `unsupported_op`**（错误体写清期望与实际），不返回假结果。
  - `/blobs`（上传：尺寸上限 `gateway.blob_upload_max_bytes`；内容寻址；返回 `{hash,size,mime}`）与 `/blobs/{hash}`（读取；M 只能读自己被会话引用过的 blob）。
  - `/audit`：O；`?session=&since=`；**不含** `auth_events`（§8.6）。
- 不做：
  - 回合、运行、问题、授权（P05-07）；事件流（P05-08）。
  - `/config*`、`/personas*`（随 P11/P12 生长）；`/commands`（P09/P14）；`/jobs*` 与 `/usage`（P13/P01 部分就位后另开）。README 的归属口径：这些路由**不注册**，404 而不是假数据。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-gateway/src/routes/system.rs`（新增） | `/info`、`/daemon/status`、`/daemon/stop` |
| `crates/gqy-gateway/src/routes/sessions.rs`（新增） | 会话 CRUD、`view`、`entries`、`ops` |
| `crates/gqy-gateway/src/routes/{tokens,blobs,audit}.rs`（新增） | 令牌、blob、审计 |
| `crates/gqy-gateway/tests/{sessions,entries,blobs,ops}.rs`（新增） | 见「测试与守护」 |

## 接口草案

草案，以实现为准。

```rust
// gqy-gateway/src/routes/sessions.rs
pub fn router(deps: Arc<GatewayDeps>) -> axum::Router;   // 挂到 /api/v1 下
/// 游标：不透明字符串；实现是 base64(seq/last_id)，客户端不得解析。
pub struct Cursor(String);
```

## 实施步骤

1. `/info`、`/daemon/status`（字段先按可得性填，缺口用 `null`）。
2. 会话 CRUD 与权限（O/M 过滤）；`SessionSummary` 映射。
3. `view`/`entries`/`ops`（含 `unsupported_op` 与 409 `Busy`）。
4. `/blobs`（上传/读取 + M 的引用检查）、`/audit`、`/daemon/stop`。
5. `cargo xtask check --fast`、`cargo test -p gqy-gateway -p gqy-store -p gqy-engine`；提交：`feat(gateway): 系统与会话路由`。

## 测试与守护

- **权限**：M 的列表只含其 principal 的会话；M 访问他人会话 → 403；O 全量。
- **分页**：`limit` 默认 50/上限 500；`next_cursor` 可续且不重复不漏（构造 120 条断言三段）。
- **删除**：缺 `?confirm=<id>` → 400；带对 → 202/204 并进入软删状态（10 的语义）。
- **ops**：忙时 409 `Busy`；未就位 op → 400 `unsupported_op`（文案含期望与实际）。
- **blob**：上传超限 413；上传后按哈希读回逐字节一致；M 读未被自己会话引用的 blob → 404（不泄露存在性）。
- **`/daemon/stop`**：UDS 上 202；HTTP 上 404。
- **`/audit`**：含 `decision` 行（P06/P07 前用测试数据）；不含 `auth_events`。
- 先红后绿对照（PR 贴输出）：把 M 的过滤去掉、把 blob 引用检查改成恒真各一次。

## 验收流程

```sh
cargo test -p gqy-gateway -p gqy-store -p gqy-engine   # 全绿
cargo xtask check --fast
# 手检（需 daemon 起来，见 P05-10）
S=$(curl -s --unix-socket run/gqy.sock -H "Authorization: Bearer $(cat run/local.token)" -X POST http://gqy.local/api/v1/sessions -d '{"title":"t"}' | jq -r .id)
curl -s -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8310/api/v1/sessions/$S/view" | jq '{as_of_seq, queue}'
curl -s -H "Authorization: Bearer $TOKEN" "http://127.0.0.1:8310/api/v1/sessions/$S/entries?limit=5" | jq '.items | length'
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 每条路由的权限、分页、错误码都有测试；未就位 op 走 400 `unsupported_op`
- [ ] M 的过滤与 blob 引用检查有区分能力测试
- [ ] `/daemon/stop` 只在 UDS 注册

## 风险与回退

- **`SessionSummary` / `SessionView` 的映射**：引擎侧类型来自 P04-01/P04-02；映射差异以协议 DTO 为准（13 §5.2）。
- **persona 最小配置**：P11 前创建会话只能用最小 persona；响应与文档都如实说明，不假装已支持。
- **回退**：`routes/*` 是新文件，revert 即可。
