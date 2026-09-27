# 15 · Web 控制台与桌面

> 状态：【规划】。前置阅读：[00 设计理念](00-设计理念.md)（§4 非目标：不引入 Node；§5 前端工具链决策）、[01 总体架构](01-总体架构.md)、[13 网关与 API 协议](13-网关与API协议.md)、[14 TUI](14-TUI.md)（事件归约与单一路径的同一套原则）。本文描述尚未实现的设计。

## 1 目标

1. **同一个二进制内嵌 Web 控制台。** 发布构建把前端产物编进 `gqy-gateway`，浏览器访问即用；运行时与 `cargo` 用户都不需要 Node（00 §4、§5）。
2. **构建期只用独立二进制。** TypeScript 由 esbuild 独立二进制打包；版本与校验和固定，由 `cargo xtask` 下载并校验。
3. **前端不持有第二份事实。** API 类型从 `gqy-protocol` 生成；设置界面从配置 schema 生成，不手抄默认值；命令表来自 `GET /api/v1/commands`。
4. **前端也有分层门禁。** 模块依赖方向由 Rust 写的 xtask 检查（不再用 Python 脚本）。
5. **桌面宿主复用同一内核。** Tauri v2 在进程内组装 engine + gateway；若已有 daemon，则退化为客户端。

## 2 v1 教训

- 前端无构建、原生 ES 模块；`app.js` 曾达 13266 行、513 个函数、`state` 125 个键【v1 实测】。
- 近 300 提交中 42 个是 WebUI 修复；有 resync 死循环。
- 前端配置默认值手抄 Rust，schema 三个文件 3.2k 行。
- 改一行 CSS 要全量重编 5.5 分钟；v1 的解法是 debug 构建下的 `GQY_WEB_DIR` 覆盖目录，且**只认环境变量**——“模型能写的目录不能放前端”【v1 实测】。v2 继承这条约束。
- 模块分层门禁（WebUI 模块依赖方向、CSS 字号与层级用 token）是 Python 脚本；无浏览器自动化测试。

## 3 工具链

### 3.1 esbuild 独立二进制

- `xtask/src/web/toolchain.rs` 内置一张固定表：

```rust
// 草案，以实现为准
pub struct PinnedTool { name: &'static str, version: &'static str, target: &'static str, url: &'static str, sha256: &'static str }
pub const ESBUILD: &[PinnedTool] = &[ /* linux-x64, linux-arm64, darwin-x64, darwin-arm64 各一条 */ ];
```

- 下载源为 npm 注册表上的平台包 tarball（`@esbuild/<platform>`），xtask 直接用 HTTP 下载并解包取出二进制，**不调用 npm/Node**。下载后校验 sha256，不一致立即失败，错误信息给出期望与实际哈希。
- 缓存到 `target/gqy-tools/esbuild-<version>/`；离线环境可设置 `GQY2_ESBUILD=/path/to/esbuild`，xtask 仍校验其 `--version` 与固定版本一致。
- 升级 esbuild = 修改这张表（版本 + 4 个校验和）的一个 PR，由 Dependabot 之外的人工流程完成（Dependabot 不认识这张表）。

### 3.2 类型检查

**esbuild 只剥离类型，不做类型检查。** 类型检查由固定版本的原生 TypeScript 检查器二进制 `tsgo` 负责（00 §5 已定，原 Q-15-1）：与 esbuild 同样在 `toolchain.rs` 的固定表中登记版本与各平台 sha256，由 xtask 下载校验（`GQY2_TSGO` 可指向离线副本，仍校验版本），`cargo xtask web check` 与 CI 以 `--noEmit` 运行；零 Node。下表保留当时的比较：

| 方案 | 说明 | 取舍 |
| --- | --- | --- |
| A | TypeScript 原生编译器（Go 实现的 `tsgo`，以平台独立二进制分发），同样按 §3.1 固定版本 + 校验和下载，`cargo xtask web check` 运行 `--noEmit` | 无 Node；需确认其发布稳定度与平台覆盖 |
| B | 只在 CI 中用 Node 运行 `tsc` | 违反“Node 不作为构建依赖”（00 §4） |
| C | 不做类型检查 | 放弃 TS 的主要收益 |

**已定 A**（00 §5，2026-09-28）。

### 3.3 xtask 子命令

| 命令 | 作用 |
| --- | --- |
| `cargo xtask web build [--release]` | 生成 TS 类型（若过期）→ esbuild 打包 → 输出到 `apps/web-console/dist/` 并写 `dist/manifest.json`（源文件树哈希、esbuild 版本） |
| `cargo xtask web check` | 类型检查（`tsgo --noEmit`，§3.2）+ 分层检查（§4.2）+ 体积门禁 |
| `cargo xtask web watch` | 监视源文件，增量重建到 `dist/`，配合 `GQY2_WEB_DIR` 使用 |
| `cargo xtask gen-ts [--check]` | 从 `gqy-protocol` 生成类型（13 §10） |

## 4 代码组织与分层

```text
apps/web-console/
  src/
    core/        # 零依赖基础：generated/（生成类型）、api/（唯一的 fetch 与 SSE 封装）、i18n、dom 工具、markdown 渲染与消毒
    state/       # 事件归约与存储：sessionStore、eventReducer、resync 状态机
    widgets/     # 无业务的通用组件：列表、虚拟滚动、对话框、表单控件（按 schema 渲染的字段控件在这里）
    features/    # 业务功能：chat、approvals、sessions、jobs、settings、usage、connectors、devices
    app/         # 组装：路由、布局、启动
  styles/        # CSS，token 化（字号、颜色、层级）
  index.html
  dist/          # 构建产物（gitignore）
```

### 4.1 分层规则

| 层 | 可以 import |
| --- | --- |
| `core` | 无（只有 `core` 内部） |
| `state` | `core` |
| `widgets` | `core` |
| `features` | `core`、`state`、`widgets`；**同层 feature 之间不得互相 import** |
| `app` | 全部 |

附加规则：
- 只有 `core/api/` 可以调用 `fetch`、`EventSource`、`WebSocket`。
- 只有 `core/dom/safe.ts` 可以写 `innerHTML`，且只接受消毒器的输出类型（`SafeHtml` 品牌类型）。模型输出永远不直接进 `innerHTML`。
- CSS 字号、颜色、`z-index` 只能用 `styles/tokens.css` 中定义的变量（v1 已有此门禁，改用 Rust 实现）。
- 单文件体积门禁与 Rust 相同（800/1500/2000，00 §3），由 `cargo xtask size` 一并检查 `.ts` 与 `.css`。

### 4.2 分层检查器（Rust，xtask）

- `xtask/src/web/layers.rs`：用一个小型词法扫描器（识别字符串、注释、模板字面量）抽取 `import … from '…'`、`export … from '…'`、`import('…')`，把相对路径解析为文件，再映射到层。
- 不引入 JS 解析器依赖；扫描器只需要正确跳过注释与字符串，单测覆盖这些边界（注释中的 import、字符串中的 `from`、动态 import）。
- 输出格式与 `cargo xtask arch` 一致：`违规：features/chat/view.ts:12 → features/sessions/store.ts（features 同层互引）`。
- 没有白名单（与 01 §3 一致）。

## 5 数据流

- **类型**：全部 API 与事件类型来自 `core/generated/`（13 §10）。手写代码不得声明与生成类型同名的接口（分层检查器附带规则）。
- **事件归约**：`state/eventReducer.ts` 是唯一把事件应用到状态的地方，语义与 TUI 的归约一致，用同一组夹具验证（14 §13、§8.2 本文）。
- **resync**：`state/resync.ts` 实现 13 §6.4 的状态机：收到 `stream.resync_required` → 拉 `view` → 用 `as_of_seq` 重连；60 秒内至多 3 次，超出显示手动重连按钮。
- **SSE 连接数**：浏览器对同源 HTTP/1.1 连接数有限（通常 6）。前端同时只订阅“当前会话流 + 全局流”两条；后台标签页通过 `BroadcastChannel` 共享一条全局流（Q-15-4）。
- **命令表**：Web 的斜杠命令补全同样来自 `GET /api/v1/commands`（14 §6.2）。

## 6 设置界面由 schema 生成

- `GET /api/v1/config/schema` 返回 `ConfigSchema`（类型定义在 `gqy-protocol`，内容由 `gqy-config` 的字段表生成，12 负责字段表）：

```rust
// 草案，以实现为准
pub struct ConfigField {
    pub path: String,                 // "provider.timeouts.stream_idle_s"
    pub group: String,                // 分组 id（界面分组与 TUI /config 分组同源）
    pub ty: FieldType,                // Bool | Int{min,max} | Float{min,max} | Str{pattern?} | Enum{values} | Duration{min,max} | List(..) | Map(..)
    pub default: serde_json::Value,   // 唯一来源：gqy-config
    pub secret: bool,
    pub restart_required: bool,
    pub label_key: String,            // i18n
    pub help_key: String,
    pub since: String,                // 首次出现的版本
}
```

- `features/settings` 只有一个通用渲染器：按 `group` 分页、按 `ty` 选择 `widgets/fields/*` 控件，**没有逐字段代码**。显示“恢复默认”时读 schema 的 `default`，前端代码里不出现任何默认值字面量。
- secret 字段只显示“已设置 / 未设置”，修改走 `PUT /config/secrets/{path}`。
- 保存用 `PATCH /config` + `If-Match: revision`；409 时提示“配置已被其他端修改”，并显示差异让用户重试（13 §5.7）。
- 守护：浏览器测试断言 schema 中每个非隐藏字段都在设置页出现恰好一次；在前端代码中 grep 默认值（xtask 规则：`features/settings/` 下禁止数值字面量，除 0/1）。

## 7 内嵌与开发覆盖

### 7.1 发布内嵌

- `gqy-gateway` 用 `rust-embed` 嵌入 `apps/web-console/dist/`。静态资源响应带 `ETag`（内容 blake3）与 `Cache-Control`：`index.html` 为 `no-cache`，带哈希文件名的资源为 `immutable, max-age=31536000`。
- `gqy-gateway/build.rs` 只检查、不构建（build.rs 不联网、不下载）：
  - `dist/manifest.json` 存在且其源树哈希与当前源一致 → 正常嵌入；
  - 不存在或过期 → 在 **release CI**（环境变量 `GQY2_REQUIRE_WEB=1`）下编译失败，报错提示运行 `cargo xtask web build`；在其它情况下嵌入一个内置的占位页（“Web console not built”），并输出 `cargo:warning`。
- 这样 `cargo build` 在没有前端产物时仍可用（TUI、CLI 不受影响），发布产物一定带真实前端。

### 7.2 开发目录覆盖

- 环境变量 `GQY2_WEB_DIR=<绝对路径>` 让网关从磁盘读前端文件，免去重编（v1 教训：CSS 改一行全量重编 5.5 分钟）。
- 约束（全部由代码判定，失败即拒绝启动该覆盖并打印错误，daemon 继续用内嵌资源）：
  1. **只认环境变量**，配置文件与 API 都不能设置它。理由：配置可被 API 修改，模型的工具在某些档位下能写配置与工作区。
  2. 只在 debug 构建（`cfg(debug_assertions)`）生效；release 构建忽略并警告。
  3. 路径必须是绝对路径、存在、是目录、包含 `index.html`。
  4. 路径不得位于数据目录（`GqyPaths::root`）之内，也不得位于任何会话的工作区根之内（查询 store 中登记的工作区根；启动后新建的工作区若包含该目录，覆盖立即失效并记录警告）。
- 生效时，启动日志与 `/info.features` 中标注 `web_dir_override`，Web 页面顶部显示开发横幅。

## 8 安全头

所有 Web 响应附加：

```text
Content-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data: blob:;
  font-src 'self'; connect-src 'self'; media-src 'self' blob:; frame-ancestors 'none'; base-uri 'none';
  form-action 'self'; object-src 'none'
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
Permissions-Policy: camera=(), microphone=(), geolocation=()
```

- 禁止内联脚本与内联样式（`style-src 'self'` 无 `'unsafe-inline'`）；需要动态样式时用 CSS 变量 + `element.style.setProperty`（CSSOM 不受 `style-src` 限制）。
- markdown 渲染在 `core/markdown` 中构建 DOM 节点而不是拼 HTML 字符串；链接统一 `rel="noopener noreferrer"`，只允许 `http`、`https`、`mailto` 协议。
- 图片与附件经 `/api/v1/blobs/{hash}` 同源读取，不从模型输出中的外部 URL 直接加载（防止外带数据）；外部图片显示为可点击链接。
- 守护：网关集成测试断言每个静态与 API 响应带上述头；浏览器测试中注入含 `<script>` 与 `onerror=` 的模型输出，断言不执行（监听全局标志）。

## 9 浏览器测试

- 方案：Rust 集成测试通过 WebDriver 驱动真实浏览器，客户端库 `fantoccini`。**选型待定（Q-15-2）**，备选 `chromiumoxide`（CDP，无需单独的 driver 进程）。
- 结构：`tests/web/` 下的测试启动临时数据目录的 daemon（mock 供应商）→ 获取配对码 → 浏览器打开登录 → 执行场景。
- 运行条件：需要 `GQY2_WEBDRIVER_URL`（例如本机 `chromedriver --port=9515`）；未设置时测试以 `#[ignore]` 形式跳过，并在测试报告中列为“跳过（缺少 WebDriver）”，而不是计为通过（19 §5）。
- 首批场景：登录；发送消息并看到流式输出；审批浮层允许/拒绝；断开网络后 resync；设置页字段完整性（§6）；XSS 注入不执行（§8）；TUI 与 Web 同时观察同一会话事件一致（M3 里程碑）。
- 前端单元测试（归约器、resync 状态机、markdown 消毒）：打包成一个测试页，在同一浏览器会话中运行并把结果以 JSON 回传给 Rust 测试，统一进入测试报告。**备选**：xtask 内嵌 JS 引擎（如 `boa`）直接运行，无需浏览器（Q-15-3）。

## 10 桌面宿主（Tauri v2）

### 10.1 两种模式

```text
启动 gqy-desktop
  ├─ 尝试获取 <data>/run/daemon.lock
  │    ├─ 拿到 → 内嵌模式：调用 gqy-daemon 在进程内组装（与 `gqy daemon run` 共用同一函数）
  │    │         同时开 UDS（TUI、gqy ask 可以连进来）；HTTP 监听遵循配置（默认同 daemon）
  │    └─ 被占用 → UDS ping 成功 → 客户端模式：WebView 通过宿主转发访问该 daemon
  │                 ping 失败（锁被占用但不响应）→ 报错并提示 `gqy daemon status`
  └─ WebView 加载内嵌的同一份前端产物
```

- 组装代码只有一份（铁律 6）：`gqy_daemon::assemble_daemon(paths: GqyPaths, cfg: ConfigSnapshot) -> DaemonHandle`，位于组装层 crate `gqy-daemon`（01 §3 的 L6）；`apps/gqy` 与桌面宿主（L7）都依赖它（00 §5 已定，原 Q-15-6）。
- 内嵌模式下退出窗口：默认最小化到托盘继续服务；“退出”菜单执行 02 §9 的停机流程。之后若用户启动 `gqy daemon start`，它会因锁被占用而提示“desktop host is serving this data directory”。

### 10.2 WebView 如何访问 API

| 方案 | 说明 | 取舍 |
| --- | --- | --- |
| A 自定义协议 | 注册 `gqy://` 协议，宿主把请求转发到进程内 Router 或 UDS | 不开 TCP 端口、无需令牌；**需验证 Tauri v2 自定义协议能否流式返回 SSE**（研究项） |
| B 回环 HTTP | 宿主启动时在 `127.0.0.1:0` 随机端口监听，把一次性令牌经初始化脚本注入 WebView | 通用、确定可行；多一个端口，令牌在页面 JS 可见 |

推荐：先验证 A；A 不支持流式时用 B，并把令牌 scope 限定为该次启动（Q-15-5）。

### 10.3 其它

- 体积与内存：技术栈文档给出的量级预期（打包 < 15 MB、常驻约 30 MB）是参考，不是验收承诺；P16 实测后写回。
- 自动更新：不在第一版；版本一致性规则见 17 §9 与 `docs/release-versioning.md`。
- 前端能力差异：桌面模式下 `info.features` 含 `desktop`，前端据此显示原生文件选择、托盘设置等；业务逻辑不分叉。

## 11 边界与失败模式

| 情况 | 行为 |
| --- | --- |
| 发布构建缺少前端产物 | release CI 编译失败；本地构建嵌入占位页 |
| esbuild 校验和不符 | xtask 失败，不使用该文件，并删除缓存 |
| `GQY2_WEB_DIR` 指向工作区内 | 拒绝覆盖，使用内嵌资源，日志 error |
| 浏览器会话 cookie 过期 | 页面回到配对界面 |
| 事件流跟不上 | 13 §6.4 的 resync；超出预算后停止自动重连 |
| 桌面宿主崩溃（内嵌模式） | 锁随进程释放；下次启动时按 02 §9 恢复非终态运行 |
| WebView 版本过旧（系统 WebView） | 启动时检查必要特性（ES2022、`EventSource`），不满足则显示说明页 |

## 12 配置项

| 键 | 默认 | 说明 |
| --- | --- | --- |
| `web.enabled` | `true` | 关闭后网关不提供静态资源（API 仍在） |
| `desktop.close_to_tray` | `true` | |
| `desktop.api_transport` | `auto` | `custom_protocol` / `loopback`，Q-15-5 |
| 环境变量 `GQY2_WEB_DIR` | 未设置 | 仅 debug 构建；§7.2 |
| 环境变量 `GQY2_ESBUILD` / `GQY2_TSGO` | 未设置 | 离线构建用，仍校验版本 |
| 环境变量 `GQY2_WEBDRIVER_URL` | 未设置 | 浏览器测试 |

## 13 测试与守护

- `cargo xtask web check` 纳入 CI：分层检查、`tsgo --noEmit` 类型检查、`gen-ts --check`、体积门禁。
- 分层检查器自测：构造违规样例（features 互引、`fetch` 出现在 `features/`、`innerHTML` 出现在 `core/dom/safe.ts` 之外），断言检查器报红；去掉任一规则对应的样例测试变红。
- `GQY2_WEB_DIR` 约束单测：工作区内路径、数据目录内路径、相对路径、release 构建，全部拒绝。
- CSP 与安全头集成测试（§8）。
- 浏览器测试（§9），缺 WebDriver 时显式“跳过”。
- 与 TUI 共享的事件归约夹具（14 §13）。
- 桌面：模式选择逻辑以纯函数实现（输入：锁状态、ping 结果），单测覆盖三种分支；Tauri 本身的端到端测试不在第一版范围。

## 14 与其他文档的关系

- API、事件、鉴权与浏览器会话见 13；TUI 的同类原则见 14；配置字段表见 12；设备共享与版本一致性见 17；门禁与测试日志见 19。
- 施工：P10（esbuild 工具链、类型生成、内嵌、骨架、对话、设置、浏览器测试）、P16（桌面宿主）。

## 15 待定问题

| 编号 | 问题 | 推荐 | 状态 |
| --- | --- | --- | --- |
| Q-15-1 | TS 类型检查如何在无 Node 前提下实现 | 方案 A：固定版本 + 校验和的 `tsgo` 二进制，`--noEmit` | 已定（00 §5，2026-09-28） |
| Q-15-2 | 浏览器测试客户端库 | `fantoccini`（WebDriver，跨浏览器）；备选 `chromiumoxide` | 待用户确认 |
| Q-15-3 | 前端单元测试运行环境 | 浏览器测试页；备选 xtask 内嵌 JS 引擎 | 待用户确认 |
| Q-15-4 | 多标签页共享全局事件流 | 用 `BroadcastChannel` 共享；第一版可不做 | 待用户确认 |
| Q-15-5 | 桌面 WebView 访问 API 的方式 | 先验证自定义协议流式能力；不行则回环 HTTP + 启动级令牌 | 研究项 |
| Q-15-6 | daemon 组装函数放在哪里（apps/gqy 与桌面宿主共用） | 组装层 crate `gqy-daemon`（01 §3 的 L6），两个 L7 入口都依赖它 | 已定（00 §5，2026-09-28） |
