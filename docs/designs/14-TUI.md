# 14 · TUI

> 状态：【规划】。前置阅读：[00 设计理念](00-设计理念.md)（铁律 6）、[01 总体架构](01-总体架构.md)（§3 入口约束）、[13 网关与 API 协议](13-网关与API协议.md)（事件目录、续传、命令表）。本文描述尚未实现的终端客户端设计。

## 1 目标

1. **纯客户端。** `gqy-tui`（`apps/tui`）只依赖 `gqy-client` 与 `gqy-protocol`，不持有任何业务事实。它总是经 UDS 连接 daemon 的网关（13 §3，带本地令牌，13 §8.1），不在进程内组装引擎；单机使用也一样（01 §1；技术白皮书 §3 的“app 独立使用时不走网关”与此不同，以 01 为准，17 §2 同此理解）。会话、回合、审批、命令语义都在 daemon；TUI 只负责显示与输入。
2. **单一渲染路径。** 每一帧先由布局模型一次性算出全部几何，绘制、光标定位、鼠标命中、预留行数全部读同一份几何。不存在绕过帧缓冲的“单行覆写”。
3. **长会话不卡。** 转录虚拟化：10 万行会话的滚动与流式输出，每帧只处理可见区域。
4. **审批与提问有界面。** v1 TUI 没有 Question 的消费者；v2 把审批/提问作为一等界面。
5. **命令只有一张表。** 补全、帮助、分发、`GET /api/v1/commands` 同源。
6. **可测试。** 用 ratatui `TestBackend` 做快照测试，用属性测试证明布局与命中互为逆运算。

## 2 v1 教训

- **两条渲染路径是 10 条验收问题的共同根因**【v1 实测，2026-09-24 验收清单】。代表性的几条：
  - #1 底栏左右闪跳：整帧按窄框左边距画底栏，而转轮 tick 与回合收尾走**单行覆写**，写死 `MoveTo(0)` + 终端全宽；两种画法交替出现在同一行。
  - #10 输入框换行后点击光标乱跳、底栏变两行：预留行数按“两格装饰 + 终端全宽”算，实际绘制按“四格装饰 + 窄框宽”；少留的行压在底栏上。点击也没有按换行后的布局反查落点。
  - #2 `/goal` 输出消失：短回执走 2.2 秒后过期的通知条，而不是转录。显示去向由内容长度隐式决定。
  - #9 文件类工具点击无反应：空内容不登记可展开块，点击静默返回 false。
  - #8 黑猫吉祥物任何尺寸都画不出：素材 48×23，容器最多 43 列，放不下就 `None`，没有降级。
- **TUI 重写过一次**，近 300 提交中 17 个 TUI 修复；最大文件 `render/stream/timeline.rs` 2402 行。
- **终端图片**：猜了四轮才查到 kitty 图形协议原文——设置页边距后，只有完全落在页内的图片随内容滚动，越界的被裁剪并留在原地；另一次事故是图片画出了预留行数之外，把后续文字压在图下【v1 实测】。结论：先读规范，图片必须占用布局模型分配的格子。
- **没有 Question 消费者**（v1 backlog）：模型提问只能在 Web 回答。

## 3 结构

```text
apps/tui/src/
  main.rs            # 仅参数解析与启动；由 apps/gqy 以库方式调用
  app.rs             # App：状态机与主循环（事件 → update → 需要重画则 draw）
  state/             # 纯数据：SessionView、Transcript、Editor、Overlays、Focus
  update/            # 纯函数：(State, Msg) -> (State, Vec<Effect>)
  layout/            # LayoutModel::compute（唯一几何来源）与 HitMap
  view/              # 只读 LayoutModel + State，写 ratatui Frame
  transcript/        # 条目模型、换行缓存、虚拟化索引、流式 markdown
  editor/            # 输入编辑器（字素级）
  commands/          # 客户端命令表 + 与服务端命令表合并
  image/             # 终端图片能力探测与放置
  effects/           # Effect 执行：调用 gqy-client、定时器
```

- **Elm 式单向数据流**：终端输入、网关事件、定时器都转换成 `Msg`；`update` 是纯函数，返回新状态与副作用列表；`effects` 执行副作用（HTTP 调用、订阅）并把结果再转成 `Msg`。`update` 不做 IO，因此可在单元测试里直接驱动。
- **与网关的连接**：`gqy-client` 提供 `trait ClientApi`（真实 UDS 实现 + 测试替身 `ScriptedClient`）。TUI 订阅当前会话的 SSE 与全局 `/events`，按 13 §6.4 处理续传与 resync。
- **单文件体积**：每个子目录的文件按 00 §3 限制；`view/` 下每个区域一个文件。

## 4 单一渲染路径

### 4.1 布局模型

```rust
// 草案，以实现为准
pub struct LayoutModel {
    pub area: Rect,                    // 终端全尺寸
    pub mode: LayoutMode,              // Lobby（空会话，大厅窄框）| Conversation | Inline
    pub header: Option<Rect>,
    pub transcript: Rect,
    pub transcript_window: VisibleWindow, // 可见条目范围与首行偏移（§5.3）
    pub input_box: Rect,               // 含装饰的外框
    pub input_text: Rect,              // 文本区（去掉装饰后的真实可写区域）
    pub input_rows: u16,               // = editor 按 input_text.width 换行后的行数（夹到上限）
    pub footer: Rect,                  // 恰好 1 行，宽度与 input_box 对齐
    pub overlays: Vec<OverlayRect>,    // 浮层（审批、命令候选、用量、会话切换）按 z 序
    pub images: Vec<ImagePlacement>,   // 图片占用的格子（§9）
    pub cursor: Option<Position>,      // 终端光标位置（由 editor 布局得出）
    pub hit: HitMap,                   // 区域 → 命中目标（条目、展开块、按钮、编辑器字素）
}

impl LayoutModel {
    pub fn compute(area: Rect, state: &UiState, caches: &mut LayoutCaches) -> LayoutModel;
}
```

规则（由测试与代码审查共同守护）：

1. **每帧恰好一次 `compute`。** `draw` 的入参是 `&LayoutModel`，view 层没有任何函数自行计算宽度、边距或行数。装饰宽度等常量只在 `layout/metrics.rs` 定义一次。
2. **不绕过帧缓冲。** 除图片转义序列外，禁止直接向 stdout 写 `crossterm` 光标移动或文本。转轮、计时器等动态元素只修改状态，然后触发整帧重画；ratatui 的差分缓冲保证只输出变化的格子，所以“整帧重画”并不昂贵。守护：`cargo xtask arch` 规则扫描 `apps/tui/src` 中除 `image/` 与 `app.rs` 终端初始化以外对 `crossterm::cursor::MoveTo`、`execute!`、`queue!`、`print!`、`println!` 的使用。
3. **命中测试读同一几何。** 鼠标事件经 `layout.hit.resolve(pos)` 得到目标；输入框点击用 editor 的同一换行结果反查字素索引。
4. **显示去向显式。** 每种输出在类型上声明去向：`Destination::Transcript | Toast | Overlay`。命令回执、goal 回执默认进转录（`Transcript`），通知条只用于“可丢失的”状态提示。去向不由内容长度推断（修 v1 #2）。
5. **放不下就降级，不静默消失。** 所有带尺寸需求的元素（吉祥物、图片、侧栏）实现 `fn fit(&self, avail: Size) -> Fit`，返回 `Full | Reduced(variant) | Placeholder`；`Placeholder` 至少画一行文字说明（修 v1 #8）。

### 4.2 布局模式与尺寸

| 模式 | 触发 | 输入框宽度 | 最小终端 |
| --- | --- | --- | --- |
| `Lobby` | 会话无条目 | `min(84, area.width - 4)` 居中 | 40×12 |
| `Conversation` | 会话有条目 | 全宽减 2 列边距 | 40×12 |
| `Inline` | `--inline` 或终端不支持备用屏 | 全宽 | 20×6 |

- 模式切换只由状态决定，同一状态必然得到同一布局（布局是 `(area, state)` 的纯函数），不会在两帧之间来回切（修 v1 #1）。
- 终端小于最小尺寸时只画一行 “terminal too small: need ≥40×12, got W×H”。

## 5 转录模型

### 5.1 条目

```rust
// 草案，以实现为准
pub struct TranscriptItem {
    pub key: ItemKey,              // Entry(EntrySeq) | Pending(RunId, local_id) | Local(u64)
    pub kind: ItemKind,            // User | Assistant | Reasoning | Tool | Notice | CommandEcho | SubagentLink | Image
    pub content: ItemContent,      // 已解析的块（markdown 块、代码块、工具摘要）
    pub expandable: Option<ExpandSpec>, // 可展开的详细内容来源（本地或按需拉取）
    pub state: ItemState,          // Streaming | Final | Failed | Cancelled
    pub wrap: WrapCache,           // 按宽度缓存的换行结果
}
```

- 条目来源只有两个：`/sessions/{id}/entries` 的历史页，与事件流。二者都经同一个 `apply_event` / `apply_entry` 归约函数进入状态（单一路径）。
- 工具条目**总是**可展开：展开内容由参数与输出拼出；确实为空时，展开区显示 “no output” 而不是点击无反应（修 v1 #9）。展开时若内容未在本地（被截断的长输出），按需请求 `GET /sessions/{id}/entries/{seq}`（13 §5.2）。
- 子代理：`subagent.started` 在父转录中产生 `SubagentLink` 条目；进入后打开子会话只读视图（§8.3）。

### 5.2 换行缓存

- `WrapCache { width: u16, lines: Arc<[Line]>, height: u32 }`：宽度变化时失效。计算用 `unicode-width` 与 `unicode-segmentation`，按字素簇切分，禁止按字节切片（v1 有中文 `split_at` panic 的前科）。
- 宽度变化（resize）时不立即重算全部条目，而是标记失效，只重算可见窗口附近的条目；高度索引对未重算条目使用估计值，滚动到那里时再精确化（§5.3）。

### 5.3 虚拟化

- 高度索引：`HeightIndex`（Fenwick 树，按条目下标存高度），支持 O(log n) 的“第 k 行属于哪个条目”与前缀和更新。
- 滚动锚点：`ScrollAnchor { item: ItemKey, line_offset: u32 }`，而不是绝对行号。resize、历史页插入（向上加载更多）、条目展开都不改变锚点条目在视口中的位置。
- **跟随模式**：锚点在底部时（`follow = true`），新内容到达自动滚到底；用户向上滚动即退出跟随，底栏显示 “↓ N new”，按 End 或滚到底恢复。
- 可见窗口 `VisibleWindow { first: usize, first_line: u32, last: usize }` 由 `LayoutModel::compute` 计算；`view` 只遍历这个范围。
- 历史懒加载：首次打开会话只拉最近 200 条；向上滚到距顶 50 行以内时预取上一页。

### 5.4 流式渲染

- `assistant.delta` 追加到 `Pending` 条目的文本缓冲。markdown 按块增量解析：已闭合的块（空行分隔的段落、闭合的代码围栏、列表结束）只解析一次并冻结；每帧只重新解析最后一个未闭合块。
- 帧调度：有新数据时标记 `dirty`，由 16 ms 的帧节拍合并重画（≤ 60 fps）；空闲时不重画。转轮只在有活动运行时以 100 ms 节拍刷新。
- `assistant.message` 到达时，用完整文本替换 `Pending` 条目并转为 `Entry(seq)`；以服务端文本为准，丢弃本地拼接结果（避免 delta 丢失导致的不一致）。
- 代码高亮：在工作线程外的同步路径中只做轻量高亮（关键字与字符串），长代码块（> 500 行）先显示无高亮版本。高亮库选型见 Q-14-2。

## 6 输入编辑器与命令表

### 6.1 编辑器

- 数据：`Editor { buffer: String, caret: usize /* 字节偏移，总在字素边界 */, selection: Option<Range>, history: History, pastes: Vec<PasteBlock> }`。
- 布局：`EditorLayout::compute(buffer, width) -> { rows: Vec<RowSpan>, caret_pos: Position }`。**绘制、预留行数（`LayoutModel.input_rows`）、点击反查三者都用这一个函数**（修 v1 #10）。
- 输入高度：1 行起，最多 `min(10, 终端高度 / 3)` 行，超出时编辑器内部滚动。
- 键位（默认，可在 12 的配置中覆盖）：Enter 发送；Shift+Enter / Alt+Enter 换行（终端不区分时用 Ctrl+J）；↑↓ 在首/末行时翻历史；Ctrl+C 有运行时取消运行、无运行时清空输入、连按两次退出；Esc 关闭浮层；Tab 补全；Ctrl+R 搜索历史。
- 粘贴：启用 bracketed paste；超过 2000 字符或 50 行的粘贴折叠为 `[pasted N lines]` 占位块，发送时展开为原文。
- 输入历史保存在 daemon（按会话，属于显示数据），不写客户端本地文件，保证 Web 与 TUI 看到同一份。**待定 Q-14-4**。

### 6.2 命令表

```rust
// 草案，以实现为准（gqy-protocol）
pub struct CommandSpec {
    pub name: String,              // "compact"
    pub aliases: Vec<String>,
    pub args: ArgSpec,             // None | Free { hint } | Choice { values_from: ValueSource } | …
    pub summary_key: String,       // i18n 键，界面文本走 i18n（00 §3）
    pub scope: CommandScope,       // Server | Client
    pub availability: Availability,// Always | NeedsSession | IdleOnly | OwnerOnly
}
```

- 服务端命令表由 engine 定义（03/12），通过 `GET /api/v1/commands` 下发；客户端命令（`/quit`、`/theme`、`/web`、`/clear-screen`）在 `apps/tui/src/commands/client.rs` 的一张表中。二者合并成一张 `CommandTable`，名称冲突时启动报错（测试守护）。
- 补全、`/help` 输出、分发都遍历 `CommandTable`，不存在第二份命令名单。客户端命令的处理函数用 `enum ClientCommand` 穷尽匹配；新增一行却没写处理分支会编译失败。
- 服务端命令经 `POST /sessions/{id}/commands` 执行，结果按 `CommandOutcome` 渲染：`notice` 进转录，`opens` 打开对应浮层（例如 `/usage` 打开用量浮层，修 v1 #3）。

## 7 审批与提问界面

- 消费事件：`question.asked` → 进入待答队列；`question.answered` / `question.closed` → 从队列移除并关闭浮层（其它客户端先答的情况）；`question.unanswerable` 只在底栏提示。
- 浮层内容（审批）：工具名、效果类别（`EffectClass`，用颜色区分 `Exec`/`Writes`/`Network`）、参数预览（`ApprovalPreview`：命令行全文、编辑类显示 unified diff 前 200 行、路径列表）、工作区与沙盒策略摘要。
- 操作：`y` 允许一次，`a` 本会话允许（仅当 08 允许该类别持久授权时显示），`n` 拒绝，`e` 拒绝并附说明（打开单行输入）。默认焦点在“拒绝”，Enter 不会误批准（失败关闭）。
- 提问（`Ask`）：单选（数字键或方向键）、多选（空格勾选）、自由文本（`allow_free_text`），多题时分页，最后一页确认提交。
- 多个待答问题按到达顺序排队，浮层标题显示 “1/3”。运行被取消时，其问题随 `question.closed{run_ended}` 自动消失。
- 等待回答期间，底栏显示 “awaiting approval”，转录中对应工具条目显示等待状态；用户可以 Esc 暂时收起浮层去翻转录，底栏提供 `F2` 重新打开。

## 8 会话、任务与子代理

### 8.1 会话切换器

- `Ctrl+S` 或 `/sessions` 打开浮层：列表来自 `GET /sessions`，按最近活动排序，显示标题、场所图标、是否有活动运行、未答问题数；输入即过滤（标题子串 + 拼音首字母可选，Q-14-5）。
- 切换会话：取消旧会话的 SSE 订阅，拉新会话 `view`，重建转录状态。切换不影响旧会话中正在运行的回合（执行在 daemon）。
- 全局流 `session.updated` 更新列表中的状态徽标，无需轮询。

### 8.2 任务 / 作业面板

- `Ctrl+J` 或 `/jobs`：右侧抽屉（宽度 ≥ 100 列时）或全屏浮层（窄终端）。数据来自 `GET /jobs` + 全局流 `job.*`。
- 每行：作业类型（后台命令、定时器、子代理后台）、状态、耗时、所属会话；选中后显示输出尾部（`GET /jobs/{id}/output`，跟随模式同 §5.3）；`c` 取消（带确认）。

### 8.3 子代理详情

- 在父转录的 `SubagentLink` 条目上按 Enter：打开子会话的只读视图（同一套转录组件，`read_only = true`，输入框隐藏）；标题栏显示父会话路径面包屑，Esc 返回父视图并恢复滚动锚点。
- 子会话是普通会话（03），因此其事件流、续传、审批都复用本文其它部分，不写专用代码。

## 9 图片

| 协议 | 探测 | 支持度 |
| --- | --- | --- |
| kitty 图形协议 | 发送查询 APC `_Gi=…;a=q` 并等待响应，超时 100 ms | 首选 |
| iTerm2 内联图片 | `TERM_PROGRAM` 等环境变量 + DA 响应 | 次选 |
| sixel | DA1 响应含 `4` | 次选 |
| 无 | 以上都没有，或 SSH 下用户关闭 | 占位文本 `[image 800×600 png · Enter to open]` |

- **图片是布局元素**：`ImagePlacement { item: ItemKey, cells: Rect, source: BlobRef, scale: Fit }`，占用布局模型分配的格子；换行缓存把图片高度计入条目高度，文字不会被压在图下（修 v1 事故）。
- **只画完全可见的图片**：放置矩形与转录视口的交集不等于放置矩形本身时，本帧画占位框而不是图片。这直接遵循 kitty 规范中“越界图片被裁剪并留在原地”的行为，避免残影。
- kitty 路径优先研究 **Unicode 占位符**（`U+10EEEE` 虚拟放置）：图片与字符格绑定，随文本滚动由终端处理。**研究项**：需要在 kitty、WezTerm、Ghostty 上实测后才能定为默认，结论写回本节（Q-14-1）。
- 图片数据经 `GET /blobs/{hash}` 拉取，按格子尺寸在客户端缩放；缓存上限 64 MiB（LRU）。
- 能力探测结果可由配置 `tui.image_protocol = "auto" | "kitty" | "iterm2" | "sixel" | "none"` 覆盖。

## 10 断线、重连与错误显示

- daemon 不可达：顶部状态条显示 “disconnected — retrying in Ns”，指数退避 0.5 s → 8 s；输入框仍可编辑，发送按钮禁用。重连后按 13 §6.4 续传。
- 收到 `stream.resync_required`：整页重建当前会话视图，保留滚动锚点（若锚点条目仍存在）。
- `run.failed`：在转录中以 Failed 条目显示 `ApiError.message`、`code` 与 `request_id`（便于在日志中定位，19）。
- `daemon.stopping`：状态条提示，并停止发送。

## 11 性能预算

| 指标 | 预算 | 测量方式 |
| --- | --- | --- |
| 首帧（daemon 已运行，进程启动到第一帧上屏） | ≤ 50 ms（p95） | `GQY2_TUI_FRAME_TRACE=1` 输出帧时间日志；CI 用基准进程测量 |
| 首帧（需拉起 daemon） | ≤ 50 ms 画出“starting daemon”占位，daemon 就绪后再加载 | 同上 |
| 按键到上屏 | ≤ 16 ms（p99） | criterion 基准：`update + compute + draw` 于 `TestBackend` |
| 流式 200 token/s 时 CPU | ≤ 10% 单核 | 基准 + 手工 |
| 10k 条目会话 resize | ≤ 30 ms | criterion |
| 10k 条目内存 | ≤ 50 MiB | 基准进程 RSS |

- 以上是设计预算，不是已测结果；P09 施工时用基准落地并写回实测值。超预算的 PR 必须附说明。

## 12 配置项

| 键 | 默认 | 说明 |
| --- | --- | --- |
| `tui.image_protocol` | `auto` | §9 |
| `tui.mouse` | `true` | 关闭后终端原生选择可用 |
| `tui.max_fps` | 60 | 范围 10–120 |
| `tui.history_page` | 200 | 首次加载条目数 |
| `tui.paste_collapse_lines` | 50 | |
| `tui.theme` | `auto` | 跟随终端亮/暗 |
| `tui.show_reasoning` | `collapsed` | `hidden` / `collapsed` / `expanded` |

## 13 测试与守护

- **快照测试**：`ratatui::backend::TestBackend` 在固定尺寸（40×12、80×24、120×40、200×60）下渲染典型状态（空会话大厅、流式中、审批浮层、会话切换器、小终端提示），与入库快照比对。快照差异以可读文本输出（按行 diff），符合 19 的测试日志要求。
- **布局不变量（属性测试，proptest）**：对随机尺寸与状态：所有矩形都在 `area` 内；`footer.height == 1`；`input_rows == EditorLayout::compute(buffer, input_text.width).rows.len()`（夹到上限）；非浮层矩形互不重叠。去掉 `input_rows` 与编辑器共享的计算后，最后一条必红（v1 #10 的回归守护）。
- **命中互逆（属性测试）**：对随机文本与宽度，对每个字素索引 i，`caret_index_at(pos_of(i)) == i`。
- **单一路径静态守护**：§4.1 规则 2 的 xtask 扫描。
- **底栏稳定性**：模拟“空会话 → 提交 → 转轮 tick × 20 → 回合结束”，断言每一帧底栏矩形完全相同（v1 #1 的回归守护）。
- **显示去向**：命令回执在 5 秒后（暂停时钟推进）仍在转录中（v1 #2 的回归守护）。
- **工具条目可展开**：对所有内置工具的 `tool.finished` 夹具，断言产生的条目 `expandable.is_some()`（v1 #9）。
- **降级**：吉祥物、图片在所有测试尺寸下 `fit()` 不返回“什么都不画”。
- **事件归约**：用 `crates/gqy-protocol/tests/fixtures/streams/*.jsonl` 中的事件序列驱动 `update`，断言最终视图与期望 JSON 相同。同一组夹具也用于 Web 的归约测试（15），保证两个前端口径一致。
- **审批默认拒绝**：浮层打开后直接按 Enter，断言发出的是 `Deny`。

## 14 与其他文档的关系

- 事件与路由见 13；审批语义见 08；会话/运行/作业/子代理语义见 03；配置与 i18n 见 12。
- 与 Web 共享事件归约夹具（15）；门禁与测试日志见 19；施工见 P05（最小 TUI）与 P09。

## 15 待定问题

| 编号 | 问题 | 推荐 | 状态 |
| --- | --- | --- | --- |
| Q-14-1 | kitty 图片用 Unicode 占位符还是直接放置 | 先研究占位符（kitty/WezTerm/Ghostty 实测），通过前默认直接放置 + “只画完全可见”规则 | 研究项 |
| Q-14-2 | 代码高亮库 | `syntect`（纯 Rust 可用 `fancy-regex` 后端，避免 onig C 依赖）；需评估二进制体积增量，超过 3 MiB 则只做轻量高亮 | 待用户确认 |
| Q-14-4 | 输入历史存 daemon 还是客户端本地 | 存 daemon（多客户端一致） | 待用户确认 |
| Q-14-5 | 会话过滤是否支持拼音首字母 | 第一版不做；作为可选增强 | 待用户确认 |
