# 13 · 网关与 API 协议

> 状态：【规划】。前置阅读：[00 设计理念](00-设计理念.md)、[01 总体架构](01-总体架构.md)、[02 运行时与并发模型](02-运行时与并发模型.md)（§7 事件总线与背压）。本文中所有接口、字段、路由都是尚未实现的设计草案。

## 1 目标

1. **一个协议，多种传输。** TUI、Web、`gqy ask`（共存期命令名 `gqy2`，见 D-06；下文统称 `gqy`）、桌面、连接器、mesh 对端都是同一套版本化 HTTP API 的客户端。UDS 上跑的是同一套 HTTP 协议，不再维护第二种帧格式。
2. **类型单一来源。** 所有请求/响应/事件的类型只在 `gqy-protocol` 定义一次，Rust 客户端直接复用，TypeScript 类型由它生成（铁律 6）。
3. **事件可续传、不死循环。** 每个事件有会话内单调 `seq`，断线按 `Last-Event-ID` 补拉，超出补拉窗口时显式要求整页重建，且重建有次数上限（02 §7）。
4. **鉴权由代码判定。** 身份来自传输层的真实凭据（UDS 本地令牌 + 对端 uid 附加检查、HTTP bearer / cookie、mesh 双向 TLS），永远不因为“来自本机回环”而放行（铁律 7）。
5. **兼容性可机器验证。** v1 协议内只允许加字段、加事件类型、加路由；旧夹具必须仍能反序列化，由测试钉住。

非目标：GraphQL / gRPC；多租户；公网直接暴露（公网访问只能经 mesh 或用户自备反向代理，且仍需 bearer）。

## 2 v1 教训

| v1 事实 | 后果 | v2 对策 |
| --- | --- | --- |
| 89 条 `/api` 路由没有版本号 | 前端与 daemon 必须同版本，改字段就是破坏性变更 | 前缀 `/api/v1/`；加法演进 + 夹具门禁（§10） |
| UDS IPC 是另一套协议（4 字节长度 + JSON，协议版本 3，帧上限 24 MiB） | 两条序列化路径，CLI 与 Web 能力不一致；“一次连接 = 一个回合” | UDS 上跑 HTTP/1.1，同一个 axum Router（§3） |
| 默认监听 `0.0.0.0:8300` | 局域网内任何人可以尝试访问 | 默认只监听 `127.0.0.1` + UDS（D-03，待定） |
| 前端 resync 死循环 | 订阅落后 → 重拉 → 仍落后 → 再重拉 | resync 有补拉上限与客户端重试预算（§6.4） |
| 连接器端点“不认本机回环” | 正确做法：沙盒里的成员会话也能连回环端口（Landlock 不管 socket） | 推广为全局规则：HTTP 一律要凭据（§8） |
| `gqy ask` JSONL 每行 `"v":1`，字段只加不改，退出码 0/1/2/3/124/130 | 这是 v1 做对的地方，外部脚本依赖它 | 原样继承契约（§9） |
| 前端配置默认值手抄 Rust（schema 3 文件 3.2k 行） | 两个真相源，改一处漏一处 | 配置 schema 由 `GET /api/v1/config/schema` 下发（§5.7、15） |
| 事件必须带 `run_id` | 旧 run 的迟到事件会污染新 run 的显示 | 信封固定带 `run`（可空），客户端按 run 过滤 |

## 3 传输与监听

| 传输 | 地址 | 默认 | 用途 | 身份来源 |
| --- | --- | --- | --- | --- |
| UDS | `<data>/run/gqy.sock`，目录 0700、socket 0600 | 开 | 本机 TUI、`gqy ask`、`gqy config`、桌面客户端模式 | 本地令牌（§8.1，必需）；对端 uid == daemon uid 为附加检查 |
| HTTP | `127.0.0.1:<port>`（端口默认 8310，待 P05 定） | 开 | Web 控制台、同机脚本 | bearer 令牌或浏览器会话 cookie |
| HTTP（局域网） | `0.0.0.0:<port>` 或指定地址 | 关（D-03 待定） | 局域网访问 | 同上；必须同时启用 TLS（17） |
| HTTPS mesh | `mesh.listen`，默认关 | 关 | 设备间 API、同步、委派 | 双向 TLS，证书按公钥钉扎（17） |
| WebSocket | `GET /api/v1/connector/ws`（HTTP 升级） | 随 HTTP | 连接器（16） | 连接器专用 bearer |
| in-proc | 桌面宿主 | — | Tauri 内嵌模式 | 宿主进程即 Owner（15） |

- **平台**（00 §5 平台范围）：UDS 是 Linux / macOS 上的本机传输。Windows 只要求能编译、跑单元测试：`gqy-client` 在 Windows 上编译为“回环 HTTP + 本地令牌”回退路径（令牌仍读 `run/local.token`），对端凭据检查在 `gqy-sys` 中报告为不可用能力；不为 Windows 做完整的传输与鉴权设计，缺失的能力失败关闭。
- 所有监听共享同一个 `axum::Router`，差异只在“身份提取层”（`tower` layer）：每种传输注入自己的 `AuthSource`，路由处理函数只看到解析后的 `Caller`。
- UDS 使用 `hyper` 的 HTTP/1.1 连接，`Host` 头固定写 `gqy.local`。`gqy-client` 对 UDS 与 TCP 暴露同一接口。
- 请求体上限：JSON 默认 1 MiB；附件上传单个 16 MiB（与 16 连接器一致）；超限返回 413 `PayloadTooLarge`。
- 每个请求都有 `request_id`（ULID），写入响应头 `X-Request-Id` 与 tracing span（19）。

## 4 版本化

- **大版本在路径里**：`/api/v1/`。v1 内部不做破坏性变更；真要破坏时新增 `/api/v2/` 并与 v1 并存至少一个发布周期（发布周期定义见 `docs/release-versioning.md`）。
- **小版本在握手里**：`gqy_protocol::PROTOCOL = ProtocolVersion { major: 1, minor: N }`。每次加字段/事件/路由，`minor += 1`。服务端在响应头 `X-GQY-Protocol: 1.N` 与 `GET /api/v1/info` 中公布；客户端在请求头发送自己的版本。
- **客户端要求**：客户端的 `major` 必须等于服务端；`minor` 可以高于或低于服务端。客户端发现服务端 `minor` 更低时，按能力位（`info.features`）降级，而不是按版本号猜。
- `run/daemon.json` 同时写 `protocol` 字段，CLI 在连接前即可判断大版本是否匹配，不匹配时退出码 2 并提示升级（错误说明期望与实际版本）。
- 其它独立版本号：连接器协议 `gqy-connector/1`（16）、mesh 协议 `gqy-mesh/1`（17）、`gqy ask` 行格式 `"v":1`（§9）。四者各自演进，互不绑定。

## 5 资源与路由

约定：
- 所有路径以 `/api/v1` 为前缀，下表省略前缀。
- 列表用游标分页：`?limit=`（默认 50，上限 500）与 `?cursor=`（不透明字符串），响应 `{ items, next_cursor }`。
- 时间一律 RFC 3339 UTC 字符串；ID 一律字符串（`SessionId`、`RunId` 等为 ULID；`TurnId` 为 `"<session>:<turn_seq>"` 的不透明编码，客户端不得解析）。
- 写操作支持 `Idempotency-Key` 头（24 小时内同键同体返回同一结果；同键异体 409 `Conflict`）。连接器与 `gqy ask` 的重试必须带它。
- 权限列：`O` = Owner，`M` = Member，`C` = 连接器令牌，`D` = mesh 对端设备（按委派授予，17）。

### 5.1 系统

| 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- |
| GET | `/health` | 无需鉴权 | `{ status: "ok" \| "stopping" }`。不暴露版本、路径、会话数 |
| GET | `/info` | O M | 版本、`protocol`、`features: string[]`、构建 ID、数据目录是否为默认值（不回显路径给 M） |
| GET | `/daemon/status` | O | 运行时摘要：已加载会话数、活动运行、写队列深度、各信号量占用 |
| POST | `/daemon/stop` | O（仅 UDS） | 触发 02 §9 停机流程，返回 202 |

### 5.2 会话

| 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- |
| GET | `/sessions` | O M | 列表，过滤 `?venue=&archived=&q=`；M 只看到自己 principal 的会话 |
| POST | `/sessions` | O M | 创建；体 `CreateSession { title?, persona?, model?, workspace?, capability? }`。persona/场所/工具面在此刻快照冻结（12） |
| GET | `/sessions/{id}` | O M | `SessionSummary` |
| PATCH | `/sessions/{id}` | O M | 改标题、归档、会话级模型覆盖；不能改 persona 快照（走 `/ops`） |
| DELETE | `/sessions/{id}` | O | 归档后删除；需要 `?confirm=<id>`。删除语义由 10 定义（软删 + 保留期） |
| GET | `/sessions/{id}/view` | O M | 会话视图快照：最近 N 条显示条目、进行中的运行、待答问题、队列、`as_of_seq`（§6.4） |
| GET | `/sessions/{id}/entries` | O M | 转录分页：`?before_seq=&after_seq=&limit=`，只返回 `Channel::Display` 投影（04），不返回 Context 通道原文；长工具输出按 07 截断 |
| GET | `/sessions/{id}/entries/{seq}` | O M | 单条目完整显示内容（未截断的工具输出、完整参数），供展开查看（14 §5.1） |
| POST | `/sessions/{id}/ops` | O M | 独占操作：`{ op: "compact" \| "undo_compact" \| "rewind" \| "fork" \| "persona_reload", … }`，对应 02 `SessionCmd::Exclusive`；忙时 409 `Busy`。换 persona id 不是操作，而是新建会话（12 §7.3） |

### 5.3 回合、运行、问题

| 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- |
| POST | `/sessions/{id}/turns` | O M C | 提交用户输入，返回 202 `Admission`（§5.3.1） |
| GET | `/sessions/{id}/turns` | O M | 回合列表（终态、用量摘要） |
| GET | `/runs/{run}` | O M | 运行详情：状态、终态原因、父运行、用量 |
| GET | `/runs/{run}/tree` | O M | 运行树：父子运行与子代理会话（03 §9.3） |
| POST | `/runs/{run}/cancel` | O M | 体 `{ reason?: string }`。已终态返回 200 + `already_terminal: true`，不报错 |
| GET | `/sessions/{id}/questions` | O M | 待答问题列表 |
| POST | `/questions/{qid}/answer` | O M | 体 `Answer`（§5.3.2）。已答/已关闭返回 409 `Conflict`，并带当前状态 |
| GET | `/sessions/{id}/grants` | O M | 本会话授权列表（08 §5.3） |
| DELETE | `/sessions/{id}/grants/{grant}` | O M | 撤销会话授权，立即生效（08 §5.3） |
| GET | `/audit?session=&since=` | O | 审计记录（08 §9.1） |

#### 5.3.1 提交与准入

```rust
// 草案，以实现为准（gqy-protocol）
pub struct SubmitTurn {
    pub input: Vec<InputPart>,            // text / image(blob ref) / file(blob ref)
    pub mode: SubmitMode,                 // Queue（默认）| Steer | Interrupt —— 与 03 §5.4 InputMode 一一对应
    pub model: Option<String>,            // 仅本回合覆盖，不落配置
    pub client_request_id: Option<String>,// 与 Idempotency-Key 二选一，供不能设头的客户端
}

pub enum Admission {                      // 03 §4 Admission 的线上投影
    Started  { turn: TurnId, run: RunId },
    Queued   { turn: TurnId, run: RunId, position: u16 },
    Steered  { into: RunId },
    Rejected { kind: ErrorKind, code: String, message: String }, // queue_full / principal_mismatch / venue_quota / shutting_down / duplicate …
}
```

- `Venue` 与 `Principal` 不由请求体给出，由网关根据 `Caller` 推导（12）。请求体里出现 `venue` 字段直接忽略——未知字段本来就忽略，这里强调它不承担任何含义。
- 返回 202 后，回合进展只通过事件流观察，响应体不等待回合结束。
- 每会话队列容量由引擎决定（`engine.session_queue_capacity`，默认 8，03 §5.1、§15），网关不另设上限；满了返回 `Rejected{code: queue_full}`，HTTP 仍为 202（准入是业务结果不是传输错误）。**待定 Q-13-3**：是否改为 429。

#### 5.3.2 问题与审批

审批与模型提问共用 `Question`（08 定义审批语义，这里只定义线上的形状）：

```rust
// 草案，以实现为准
pub struct QuestionView {
    pub id: QuestionId,
    pub run: RunId,
    pub kind: QuestionKind,               // Approval { call: CallId, tool: String, effect: EffectClass, preview: ApprovalPreview } | Ask
    pub prompts: Vec<QuestionPrompt>,     // header, question, options[], multi: bool, allow_free_text: bool
    pub asked_at: String,
}
pub enum Answer {
    Approval { decision: ApprovalDecision, note: Option<String> }, // AllowOnce | AllowForSession | Deny
    Ask { answers: Vec<PromptAnswer> },   // 每题一答；多选题为数组
    Dismiss,                              // 关闭问题：Ask 视为“无回答”，Approval 视为 Deny
}
```

- 任一有权限的客户端都可以回答；先到者生效，其它客户端收到 `question.answered` 后关闭自己的界面。
- 回答方身份写入账本（审计，08）。M 不能回答 O 会话的问题。

### 5.4 事件

| 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- |
| GET | `/sessions/{id}/events` | O M | SSE 流；`Last-Event-ID` 或 `?after_seq=` 续传 |
| GET | `/sessions/{id}/events?format=json&after_seq=&limit=` | O M | 同一数据的 JSON 分页拉取（resync 与调试用） |
| GET | `/events` | O | 全局 SSE：会话列表变化、作业、daemon 状态；独立的全局 seq 空间 |

### 5.5 命令

| 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- |
| GET | `/commands` | O M | 服务端命令表（`CommandSpec[]`），驱动 TUI/Web 的补全、帮助（14 §6） |
| POST | `/sessions/{id}/commands` | O M | `{ name, args: string }` 执行服务端命令；结果为 `CommandOutcome { notice?, opens?: ViewRef }` |

### 5.6 作业与用量

| 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- |
| GET | `/jobs` | O M | 作业列表（后台命令、定时器、子代理后台任务，03） |
| GET | `/jobs/{id}` | O M | 详情 |
| GET | `/jobs/{id}/output?after=&limit=` | O M | 输出分页（字节偏移游标） |
| POST | `/jobs/{id}/cancel` | O M | 取消 |
| GET | `/usage?from=&to=&group_by=day\|provider\|model\|session` | O | 用量汇总，四桶 `input_uncached / cache_read / cache_write / output` + 请求数（06、19） |

### 5.7 配置与人格

| 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- |
| GET | `/config` | O | 当前配置，secret 字段替换为 `{ "set": true }`，附 `revision` |
| GET | `/config/schema` | O M | 配置模式（字段类型、默认值、范围、枚举、分组、i18n 键、是否需重启、是否 secret），唯一来源为 `gqy-config`（12、15） |
| PATCH | `/config` | O | JSON Merge Patch，必须带 `If-Match: <revision>`；不一致 409 `Conflict`（防 v1 并发保存互相回滚） |
| PUT | `/config/secrets/{path}` | O | 只写，响应不回显 |
| GET | `/personas` | O M | 列表 + 当前 generation |
| GET/PUT | `/personas/{name}` | O（PUT） | 读 / 原子写 persona；写入产生新 generation，只影响下一回合（12） |

### 5.8 鉴权与令牌、blob

| 方法 | 路径 | 权限 | 说明 |
| --- | --- | --- | --- |
| POST | `/auth/login` | 无需鉴权（限速） | 用一次性配对码换浏览器会话（§8.3） |
| POST | `/auth/logout` | 任意已登录 | 吊销当前浏览器会话 |
| GET/POST | `/auth/tokens` | O | 列出 / 创建 bearer 令牌（`scope`、`label`、`expires_at?`），明文只在创建时返回一次 |
| DELETE | `/auth/tokens/{id}` | O | 吊销，立即生效（按请求查库，02 §8） |
| POST | `/blobs` | O M C | 上传附件，返回 `{ hash, size, mime }`（blake3 内容寻址，10） |
| GET | `/blobs/{hash}` | O M | 读取；只能读自己会话引用过的 blob（M），防止按哈希探测 |

连接器 WebSocket 与 mesh 路由分别在 16、17 定义，挂在同一 Router 下的 `/api/v1/connector/*`、`/api/v1/mesh/*`。

## 6 事件

### 6.1 信封

```rust
// 草案，以实现为准（gqy-protocol）
pub struct EventEnvelope {
    pub v: u16,                 // 信封版本，当前 1
    pub seq: EventSeq,          // 会话流：会话内单调；全局流：全局单调
    pub session: Option<SessionId>, // 全局流上的非会话事件为 None
    pub run: Option<RunId>,     // 与某次运行相关时必填
    #[serde(rename = "type")]
    pub kind: String,           // 点分小写，如 "assistant.delta"
    pub ts: String,             // RFC 3339，服务端时钟
    pub data: serde_json::Value,// 按 kind 对应的强类型体序列化
}
```

- 线上是 JSON；Rust 侧另有强类型 `enum Event`，`EventEnvelope` 由它的唯一序列化函数生成（单一路径）。未知 `type` 在客户端解析为 `Event::Unknown { kind, data }` 并忽略，不报错。
- SSE 帧：`id: <seq>`、`event: <type>`、`data: <整个信封 JSON>`。`data` 里重复带 `seq`/`type`，便于非 SSE 通道（JSON 拉取、`gqy ask`）复用同一解析器。
- 心跳：15 秒无事件时发 SSE 注释行 `: ping`，不占 seq。

### 6.2 事件类型目录（v1 协议内的初始集合）

| 类型 | 持久化 | data 主要字段 | 说明 |
| --- | --- | --- | --- |
| `session.created` / `session.updated` / `session.archived` | 是 | `SessionSummary` | 全局流与会话流都发 |
| `session.forked` | 是 | `from_session, from_seq, new_session` | 17 的冲突分叉也用它 |
| `session.expired` | 是 | `session, reason` | 子代理审计会话或已删除会话被保留作业清除（03 §9.3、10 §9.3）；只在全局流 |
| `session.persona_generation` | 是 | `persona, generation` | persona 代际切换生效（压缩或 `/persona reload`，12 §5.5） |
| `session.mode_changed` | 是 | `mode` | 能力档位切换（08 §4.1） |
| `queue.enqueued` | 是 | `turn, run, position, from?: "steer"` | 03 §3.2、§5.4 |
| `queue.dequeued` | 是 | `turn, run` | 出队开始运行 |
| `queue.dropped` | 是 | `turn, run, reason: superseded\|ttl\|user\|restart` | 03 §5.2、§13 |
| `generation.superseded` | 是 | `run, generation` | 过期运行的对外投递被抑制（03 §5.2） |
| `steer.accepted` / `steer.applied` | 是 | `run, entry_seq?` | 插话进入收件箱 / 在安全点落账（03 §5.4） |
| `run.started` | 是 | `turn, run, parent_run?, model, venue_kind` | |
| `run.state` | 是 | `run, from, to` | 每次状态转移（03 §3.2） |
| `run.completed` | 是 | `usage, elapsed_ms, final_entry_seq, truncated` | 终态 |
| `run.failed` | 是 | `error: ApiError` | 终态 |
| `run.cancelled` | 是 | `reason: CancelReason` | 终态（02 §5） |
| `run.interrupted` | 是 | `recovered_at` | 启动恢复时补发（02 §9） |
| `run.awaiting` | 是 | `question` | 进入 `AwaitingApproval`/等待回答 |
| `request.started` | 否 | `req_seq, prefix_hash` | 回合内第 N 次模型请求 |
| `request.retrying` | 是 | `req_seq, attempt, kind, delay_ms` | 供应商层单层重试（06 §8.2） |
| `request.finished` | 是 | `req_seq, usage, finish_reason, ttfb_ms, elapsed_ms` | 用量四桶绝对数（19） |
| `provider.key_disabled` | 是 | `provider, key_id, reason` | 密钥被禁用（06 §8.2）；全局流 |
| `assistant.delta` | 合并（§6.3） | `text` | |
| `reasoning.delta` | 合并 | `text` | 仅在 persona/配置允许显示推理时发 |
| `assistant.message` | 是 | `entry_seq, text` | 一次请求的完整助手文本（显示通道） |
| `tool.preparing` | 否 | `call, name` | 流中出现工具调用，参数尚未完整 |
| `tool.started` | 是 | `call, name, args_preview, effect` | |
| `tool.progress` | 否 | `call, message` | |
| `tool.output` | 合并 | `call, stream: stdout\|stderr, text` | 输出截断规则见 07 §8 |
| `tool.artifact` | 是 | `call, blob, mime, name, alt?` | 图片、文件等产物 |
| `tool.finished` | 是 | `call, ok, summary, elapsed_ms, error?` | |
| `permission.decided` | 是 | `call, decision: ask\|deny, layer, rule_id` | 只发 ask 与 deny（08 §9.2） |
| `sandbox.unavailable` | 是 | `backend, reason` | 沙盒探测由可用变不可用（08 §8）；全局流 |
| `question.asked` | 是 | `QuestionView` | |
| `question.answered` | 是 | `question, by: CallerSummary, answer_summary` | |
| `question.closed` | 是 | `question, reason: dismissed\|run_ended\|expired\|cancelled` | `expired`：启动恢复时关闭（03 §13） |
| `question.unanswerable` | 是 | `question, venue_kind` | 非交互场所需要审批而被拒（08 §5.5），供 UI 提醒 |
| `context.compact_started` | 是 | `trigger: auto\|force\|overflow\|manual, stage, before_tokens` | 05 §12 |
| `context.compact_finished` | 是 | `path, folded_turns, kept_turns, summary_tokens, before_tokens, after_estimate, rehydrated_files, epoch` | |
| `context.compact_skipped` | 是 | `gate` | 四道闸之一拦下（05 §5.2） |
| `context.compact_failed` | 是 | `reason, fallback: placeholder\|none` | |
| `context.notice` | 是 | `kind: occupancy\|stuck\|thrashing, …` | 05 §4.2、§5.2 |
| `context.rewrite` | 是 | `reason, epoch, from_seq, to_seq` | 所有前缀改写（04 §9.1 的原因枚举，铁律 9，19 §3.4） |
| `usage.updated` | 否 | `session_totals` | 会话累计，供底栏显示 |
| `job.created` / `job.progress` / `job.finished` | 是（progress 否） | `JobSummary` | 全局流与所属会话流（03 §10.1） |
| `work.dead` | 是 | `work_id, kind, attempts, last_error` | 内部工作项重试耗尽（11 §7）；全局流 |
| `timer.fired` / `timer.missed` | 是 | `timer, late_by_ms?` | 03 §10.2、§13 |
| `subagent.started` / `subagent.finished` | 是 | `child_session, child_run, summary?` | 子运行详细事件在子会话流里（03 §9） |
| `hook.skipped` | 是 | `hook_id, reason: timeout\|error` | 子系统钩子本次未注入（18 §3.3） |
| `ext.mcp_status` | 是 | `server, state` | MCP 健康状态变化（18 §6.4）；全局流 |
| `room.message` | 是 | `room, member, text, entry` | 聊天室（16 §9） |
| `config.changed` / `config.invalid` | 是 | `revision, changed_paths, restart_required` / `errors` | 12 §3.4；全局流 |
| `persona.invalid` | 是 | `persona, errors` | 12 §5.4；全局流 |
| `store.degraded` / `store.integrity_failed` | 是（尽力） | `reason` | 10 §4.2、§9.2；全局流 |
| `notice` | 否 | `level: info\|warn\|error, code, message` | 非致命提示 |
| `stream.resync_required` | 否 | `from_seq, reason: lagged\|purged\|too_far` | §6.4 |
| `daemon.stopping` | 否 | `deadline_ms` | 全局与会话流都发，然后服务端关闭流 |

- “持久化 = 否”的事件也分配 `seq`，但不写入事件表；补拉时它们自然缺席。客户端必须容忍 seq 空洞——**seq 保证单调，不保证连续**。
- **本表是事件类型的唯一目录**：其他文档提到的事件类型都必须出现在这里，名称以本表为准；新增事件类型是加法变更（`minor += 1`），必须同时更新本表、TS 生成物和夹具。

### 6.3 增量事件的合并落库

逐 token 写库代价太高，逐条丢弃又违背 02 §7 “先落库再广播”。折中：

1. `assistant.delta` / `reasoning.delta` / `tool.output` 在事件总线处按 `(run, 类型, call)` 合并，窗口 50 ms 或 4 KiB，先到者触发；合并后的一条事件分配一个 seq、落库、广播。实时性损失 ≤ 50 ms。
2. 运行进入终态后，保留期（`store.retention.stream_event_days`，默认 7 天，10 §9.3）过后，由保留作业删除该运行的 `assistant.delta`、`reasoning.delta`、`tool.output` 行，只保留 `assistant.message`、`tool.finished` 等完整事件。完整内容始终可从账本重建（04），所以这是**可重建的数据**，并且登记在 10 §9.3 的保留清单中，不违反铁律 9。事件表结构与触发器归 10。
3. 被删区间内的 `after_seq` 补拉返回 `stream.resync_required { reason: "purged" }`。

### 6.4 续传与 resync 规则

```text
客户端连接 SSE，带 Last-Event-ID = L（首次连接不带，服务端从“当前”开始，只推新事件）
  ├─ L ≥ 当前最大 seq            → 直接实时推送
  ├─ 事件表中存在 (L, now] 且条数 ≤ replay_max（默认 5000）→ 先补发再实时
  └─ 否则（被清理 / 太远）       → 发一条 stream.resync_required{from_seq=L} 并关闭流
订阅者缓冲满（每订阅者 1024，02 §7） → 发 stream.resync_required{reason=lagged} 并关闭流
客户端收到 resync_required：
  1. GET /sessions/{id}/view → 得到 as_of_seq 与完整视图，整页替换本地状态
  2. 以 Last-Event-ID = as_of_seq 重连
  3. 重试预算：60 秒内最多 3 次 resync；超出则停止自动重连，界面显示“事件流跟不上”并提供手动重连
```

- `view` 与事件流的一致性：`view` 在 actor 的 `Snapshot` 命令中生成（02 §3.2），`as_of_seq` 是生成时刻已广播的最大 seq；之后的事件一定 `> as_of_seq`，不会重复也不会遗漏。
- 客户端按 `seq` 去重：`seq ≤ 本地最大 seq` 的事件丢弃。
- 服务端对同一订阅连续两次 `lagged`（间隔 < 10 秒）时，把该订阅降级为“只推持久化事件”（不推 delta），直到重连。这是对慢客户端的降级，属于 02 §7 允许的“丢弃可重建的数据”。

## 7 错误体

```rust
// 草案，以实现为准。ErrorKind 定义在 gqy-core（01 §3），这里是它在 API 上的投影。
pub struct ApiError {
    pub kind: ErrorKind,          // 机器可读分类
    pub code: String,             // 更细的稳定代码，如 "session_not_found"、"queue_full"
    pub message: String,          // 人读；说它真正知道的：期望 / 实际
    pub expected: Option<serde_json::Value>,
    pub actual: Option<serde_json::Value>,
    pub retryable: bool,
    pub retry_after_ms: Option<u64>,
    pub request_id: String,
}
// HTTP 响应体：{ "error": ApiError }
```

| `ErrorKind`（API 投影） | HTTP | retryable | 典型 `code` |
| --- | --- | --- | --- |
| `InvalidInput` | 400 | 否 | `invalid_json`、`field_out_of_range` |
| `Unauthenticated` | 401 | 否 | `missing_token`、`token_revoked`、`peer_uid_mismatch` |
| `Forbidden` | 403 | 否 | `trust_insufficient`、`not_session_owner` |
| `NotFound` | 404 | 否 | `session_not_found`、`run_not_found` |
| `Conflict` | 409 | 否 | `revision_mismatch`、`question_already_answered`、`idempotency_key_reuse` |
| `Busy` | 409 | 是 | `exclusive_op_running` |
| `PayloadTooLarge` | 413 | 否 | `body_too_large`、`attachment_too_large` |
| `ProtocolMismatch` | 426 | 否 | `major_version_mismatch` |
| `RateLimited` | 429 | 是 | `rate_limited`（带 `Retry-After`） |
| `Internal` | 500 | 否 | 只给 `request_id`，细节进日志 |
| `Upstream` | 502 | 视情况 | 供应商错误（06 的分类放在 `actual` 里） |
| `StoreBusy` | 503 | 是 | 写队列超时（02 §6） |
| `Stopping` | 503 | 是 | daemon 停机中 |
| `Timeout` | 504 | 是 | 服务端内部等待超时 |

- 不允许出现“兜底 500 + 字符串”。每个库 crate 的 `Error` 实现 `kind()`，网关只做 `kind → HTTP` 的一张表映射（单一路径）；新增 `ErrorKind` 变体时编译器强制补表（穷尽 match）。
- `run.failed` 事件的 `error` 字段是同一个 `ApiError` 结构。

## 8 鉴权

### 8.1 UDS：本地令牌 + 对端凭据（00 §5 已定）

- **本地令牌是必需凭据**：每次 daemon 启动生成新的随机令牌，写入 `run/local.token`（0600，`run/` 目录 0700）；UDS 上的每个请求都必须带 `Authorization: Bearer <local token>`，缺失或不符 401 `missing_token`。daemon 重启后旧令牌立即失效，客户端重新读取文件。
- **对端凭据只是附加检查**：接受连接时读取对端凭据（Linux `SO_PEERCRED`，macOS `getpeereid`/`LOCAL_PEERPID`，由 `gqy-sys` 封装），uid 必须等于 daemon 的 uid，否则 401 `peer_uid_mismatch`。单凭 uid 不够：沙盒内的命令与 daemon 同 uid，可以 `connect()` 到 socket。
- **沙盒必须拒绝读取 `run/` 与 `config/`**（08 §6.2 硬规则，任何后端都适用），由 08 的策略单测钉住：去掉这条规则后“沙盒内读取 local.token”的测试必须变红。
- **边界诚实**：无沙盒执行的命令——`Full` 档位，或无沙盒后端平台上 Owner 批准的 `Workspace` Exec（08 §4.1）——等价于 Owner 本人，能读到令牌，不在本防护范围内；这一点在 08、审批卡片与用户文档里明写（铁律 8）。原 Q-13-1 由此关闭。

### 8.2 HTTP：bearer 令牌

- 令牌格式 `gqy_<scope>_<32 字节随机 base32>`，库里只存 blake3 哈希、scope、label、创建/最后使用/过期时间、吊销标记。比较用常数时间。
- scope：`client`（等价 Owner 的完整 API）、`member:<principal>`（成员账号，12 定义）、`connector:<platform>`（只能访问 `/api/v1/connector/ws` 与 `/blobs` 上传，16）、`readonly`（只读 GET）。scope 检查在路由层声明式完成（每条路由标注允许的 scope 集合，表驱动，编译期生成路由权限表，可 `GET /api/v1/commands` 同法导出审计）。
- **不信任回环**：来自 `127.0.0.1` 的 HTTP 请求同样必须带凭据；唯一例外是 `GET /health`。理由见 §2（沙盒不管网络，08）。
- 吊销立即生效：每个请求按令牌哈希查库（读连接池，带 5 秒 TTL 的进程内缓存，吊销时主动失效，02 §8）。SSE 长连接在令牌吊销后 5 秒内被服务端关闭。

### 8.3 浏览器会话

浏览器的 `EventSource` 不能设置请求头，所以 Web 控制台用 cookie：

1. `gqy web`（或 TUI 命令 `/web`）通过 UDS 请求一个一次性配对码（8 位、5 分钟有效、单次使用），并打印 `http://127.0.0.1:<port>/#pair=<code>`。
2. 页面用配对码 `POST /auth/login`，服务端下发 `gqy_session` cookie：`HttpOnly; SameSite=Strict; Path=/api; Secure`（非 TLS 回环除外）；有效期 30 天，滑动续期。
3. 所有非 GET 请求还必须满足：`Origin` 与服务地址同源，且带 `X-GQY-CSRF` 头（值来自 `GET /info` 的响应，绑定会话）。

**待定 Q-13-2**：是否改用“fetch 流式读取 SSE + 内存中的 bearer”以彻底去掉 cookie。推荐保留 cookie 方案：令牌不暴露给页面 JS，XSS 时也无法外带长期凭据。

### 8.4 身份到场所

`Caller { auth: AuthSource, principal: Principal, trust: Trust }` 在身份提取层产生。网关据此构造 `Venue`：UDS 且客户端声明为 TUI → `VenueKind::Tui`；浏览器会话 → `Web`；`gqy ask` → `Cli`；桌面 → `Desktop`；连接器 → `Connector{platform}`；mesh → `Mesh{device}`。客户端声明的“我是谁”（`X-GQY-Client: tui/0.1.0`）只影响 `VenueKind` 的显示类别，**不影响信任级**——信任级只由 `AuthSource` 决定（12）。

## 9 `gqy ask` 与 JSONL 契约

`gqy ask` 是 `gqy-client` 的薄封装，经 UDS 调用 §5 的 API。它的输出是对外稳定契约，独立于事件信封演进。

### 9.1 输出格式

- `--output-format text`（默认）：人读，终端渲染。
- `--output-format json`：只打一行终态（`done` 或 `error`）。
- `--output-format stream-json`：逐事件一行，最后一行必为 `done` 或 `error`。

每行一个 JSON 对象，固定带 `"v": 1`。**字段只加不改、类型只加不删**；消费方必须忽略未知字段与未知 `type`。

```text
{"v":1,"type":"started","session_id":"…","run_id":"…","turn_id":"…"}
{"v":1,"type":"text","delta":"…"}
{"v":1,"type":"reasoning","delta":"…"}
{"v":1,"type":"tool","phase":"start|progress|output|end","call_id":"…","name":"…", …}
{"v":1,"type":"image","call_id":"…","blob":"…","mime":"image/png","alt":"…"}
{"v":1,"type":"question","question_id":"…","kind":"approval|ask","prompts":[…]}
{"v":1,"type":"notice","level":"info|warn|error","message":"…"}
{"v":1,"type":"usage","usage":{"input_uncached":0,"cache_read":0,"cache_write":0,"output":0},"model":"…"}
{"v":1,"type":"done","session_id":"…","run_id":"…","text":"完整正文","usage":{…},"elapsed_ms":0}
{"v":1,"type":"error","kind":"usage|session_not_found|turn_failed|cancelled|timeout|disconnected|protocol","message":"…"}
```

- 映射规则：`AskLine` 是 `gqy-protocol` 中的独立类型，由唯一函数 `ask_line_from_event(&Event) -> Option<AskLine>` 从事件派生。v1 的 `usage` 结构改为四桶（v1 曾在 usage 里放供应商原始字段），这是 v2 相对 v1 契约的**唯一**有意变化，写入 `CHANGELOG`。
- json 模式下错误也以一行 `error` 打到 stdout（宿主只解析一个流），stderr 不复述。

### 9.2 退出码

| 码 | 含义 | 对应 `error.kind` |
| --- | --- | --- |
| 0 | 成功 | — |
| 1 | 回合失败（模型 / 工具 / daemon 错误、断连） | `turn_failed`、`disconnected` |
| 2 | 用法或参数错误、协议大版本不匹配 | `usage`、`protocol` |
| 3 | 会话不存在 | `session_not_found` |
| 124 | `--timeout` 到点，已发送取消 | `timeout` |
| 130 | 被取消（SIGINT / 服务端取消） | `cancelled` |

### 9.3 非交互场景的问题

`gqy ask` 场所默认声明 `interactive: false`（12 §6.2）。运行中出现问题时：`Ask` 类问题自动 `Dismiss`；`Approval` 类问题自动 `Deny`，并打一行 `notice` 说明被拒的工具。只有显式 `--interactive`（等同 `--approve=prompt`，需要 TTY，08 §5.5）才逐次询问。失败关闭：不存在“非交互就自动批准”的默认值。需要免审批时，由用户在配置里为 Cli 场所设置能力档位（08），而不是在 `ask` 上加旁路开关。

**待定 Q-13-4**：是否在 v2 继承 v1 的长驻 `gqy stdio` 协议。推荐：不继承独立协议；需要长驻的宿主直接用 `gqy-client` 或 UDS 上的 HTTP + SSE，减少一条路径。

## 10 类型单一来源与兼容规则

- 所有 DTO、事件体、`AskLine`、连接器帧、`ApiError` 定义在 `gqy-protocol`。`gqy-gateway`、`gqy-client`、TUI、连接器都依赖它；**不得**在其它 crate 重复定义线上的形状。
- TS 生成：`gqy-protocol` 的可选 feature `ts` 启用 `ts-rs` 派生；`cargo xtask gen-ts` 输出到 `apps/web-console/src/core/generated/`，文件头标注“生成文件，勿手改”，生成物入库。CI 运行 `cargo xtask gen-ts --check`，与入库内容不一致即红（15）。
- 兼容规则（在 `gqy-protocol` 内由测试与 xtask 规则强制）：
  1. 禁止 `#[serde(deny_unknown_fields)]`（xtask grep）。
  2. 新增字段必须 `Option<T>` 或 `#[serde(default)]`。
  3. 客户端解析的枚举必须有 `#[serde(other)] Unknown` 兜底变体（服务端可能比客户端新）。
  4. 字段不得改名、改类型、删除；枚举值不得删除。
  5. **夹具门禁**：`crates/gqy-protocol/tests/fixtures/v1/` 下每种类型至少一份 JSON 夹具，夹具只增不改（xtask 检查已入库夹具的 blake3 清单）。测试逐份反序列化并再序列化，断言关键字段不丢。区分能力：给任一 DTO 加 `deny_unknown_fields` 或删字段，夹具测试变红。
- 序列化稳定性：所有 map 字段用 `BTreeMap`，同一值序列化字节确定（与 04 的规范化 JSON 同一实现 `gqy_core::canonical_json`）。

## 11 限速

| 对象 | 默认 | 超限 |
| --- | --- | --- |
| `/auth/login` 每来源 IP | 5 次 / 分钟，连续 10 次失败后锁 15 分钟 | 429 |
| 每令牌写请求 | 30 次 / 秒，突发 60 | 429 + `Retry-After` |
| 每令牌 SSE 并发订阅 | 32 | 429 |
| 每会话提交回合 | 由准入与队列控制（02 §3.1、本文 §5.3.1） | `Admission::Rejected` |
| 连接器按 principal | 见 16 §6（游客 2 次 / 600 秒） | 连接器侧回执 |
| 全局 SSE 连接 | 256 | 503 `Busy` |

- 限速器是令牌桶，进程内、按键有界（LRU 上限 10k 键），不落库；重启后清零是可接受的。
- 所有数值在 `gqy-config` 一处定义（02 §6 的同一张表），可放宽到上限，不能关闭。

## 12 边界与失败模式

| 情况 | 行为 |
| --- | --- |
| daemon 停机中 | 新请求 503 `Stopping`；SSE 收到 `daemon.stopping` 后关闭 |
| store 写队列满 | 写路由 503 `StoreBusy`，`retryable: true`；事件落库失败时该事件不广播，运行以 `StoreBusy` 失败（不静默丢写） |
| 客户端大版本不匹配 | 426，`expected`/`actual` 写明版本 |
| SSE 客户端不读 | 缓冲满 → `resync_required{lagged}` → 关闭 |
| 同一问题被两个客户端同时回答 | actor 串行处理，后者 409 |
| 令牌在 SSE 期间被吊销 | 5 秒内关闭连接 |
| 请求体含未知字段 | 忽略（兼容规则 1） |
| UDS socket 文件残留（上次崩溃） | 启动时先拿 `daemon.lock`，拿到才删除旧 socket 重建（01 §4） |

## 13 配置项

| 键 | 默认 | 说明 |
| --- | --- | --- |
| `gateway.http.listen` | `127.0.0.1:8310` | D-03 待定；端口 P05 定 |
| `gateway.http.enabled` | `true` | 关闭后只剩 UDS |
| `gateway.uds.enabled` | `true` | |
| `gateway.body_limit_bytes` | 1 MiB | 上限 8 MiB |
| `gateway.events.replay_max` | 5000 | 上限 50000 |
| `gateway.events.coalesce_ms` | 50 | 范围 10–250 |
| `gateway.browser_session_ttl` | `30d` | |
| `gateway.rate.*` | 见 §11 | |

## 14 测试与守护

- **路由权限表测试**：遍历编译期生成的路由表，断言每条路由都声明了 scope；新增路由不声明则编译失败或测试红。
- **不信任回环测试**：从 `127.0.0.1` 无令牌访问任一非 `/health` 路由，全部 401。去掉鉴权层后此测试变红。
- **UDS 对端测试**：临时目录起 daemon；同 uid 无本地令牌 → 401；带令牌 → 200。（跨 uid 情形需特权，标记 `#[ignore]` 并在 CI Linux 用 `unshare` 跑，P05 定。）
- **续传测试**：mock 供应商产生 1000 个事件；客户端在 seq=300 断开，带 `Last-Event-ID: 300` 重连，断言收到的 seq 严格递增、无重复、持久化事件无遗漏。
- **resync 测试**：订阅者故意不读，触发 `lagged`；客户端按 §6.4 恢复后视图与服务端 `view` 一致；制造连续失败，断言 60 秒内至多 3 次 resync（防 v1 死循环）。
- **合并落库测试**：模拟 200 个 1 字节 delta 在 50 ms 内到达，断言落库事件数 ≤ 5 且拼接文本一致。
- **协议夹具门禁**：§10 第 5 条。
- **TS 生成门禁**：`gen-ts --check`。
- **`gqy ask` 契约测试**：对 mock 回合跑 `stream-json`，逐行断言 `v == 1`、最后一行为 `done`；对超时、取消、会话不存在分别断言退出码 124/130/3。去掉 `ask_line_from_event` 中任一分支，对应断言变红。
- **错误映射穷尽**：`ErrorKind → HTTP` 使用无通配 match，clippy `wildcard_enum_match_arm` 在该模块 deny。

## 15 与其他文档的关系

- 事件总线、seq、背压来自 02 §7；本文定义线上形状与续传规则。
- `Venue`、`Principal`、信任级推导见 12；审批语义见 08；回合/运行/排队状态见 03。
- 事件表与保留作业归 10；用量四桶定义见 06；日志与门禁见 19。
- 连接器端点与帧见 16；mesh 路由与 TLS 见 17；TUI 与 Web 如何消费见 14、15。

## 16 待定问题

| 编号 | 问题 | 推荐 | 状态 |
| --- | --- | --- | --- |
| D-03 | 默认监听地址 | `127.0.0.1` + UDS；局域网需显式开启并配合配对与 TLS | 待用户确认（00） |
| Q-13-1 | macOS 无沙盒后端时，子进程是否可能读到 `run/local.token` | 由 00 §5 的 UDS 鉴权与无沙盒 Exec 两项决策关闭：沙盒存在时必须拒绝读 `run/`；无沙盒执行等价 Owner，明文写出（§8.1） | 已定（2026-09-28） |
| Q-13-2 | 浏览器凭据用 cookie 还是内存 bearer | cookie（HttpOnly + SameSite=Strict + Origin + CSRF 头） | 待用户确认 |
| Q-13-3 | 队列满时准入返回 202+Rejected 还是 429 | 保持 202 + `Admission::Rejected`，准入是业务结果 | 待用户确认 |
| Q-13-4 | 是否继承 v1 `gqy stdio` 长驻协议 | 不继承；长驻宿主用 UDS 上的 HTTP + SSE | 待用户确认 |
| Q-13-5 | 默认 HTTP 端口 | 8310（避开 v1 的 8300，共存期不冲突） | 待用户确认 |
