# 顾清影 v2 (GQY v2) 技术栈与语言规范

> 本文档规范 GQY v2 项目的全层级开发语言、技术选型与工程原则，指导 v2 架构的纯血化重构与标准化施工。

---

## 一、核心工程原则

1. **纯血化（Pure Native）**：
   以 **Rust and c** 为绝对核心语言，杜绝任何外部解释型运行时（Python / Node.js）的强依赖，消除跨语言胶水层与环境配置负担。
2. **零运行时依赖（Zero Runtime Dependencies）**：
   核心产物为单一自包含可执行文件（Single Self-Contained Binary）。开箱即用，不依赖目标机器预装 Python 环境、Node 模块或系统级服务。
3. **强类型与可预测延迟（Predictable & Safe）**：
   利用 Rust 的所有权模型与无 GC 特性，保证在长时间（数天乃至数周）、数十万 Token 吞吐的高并发多轮对话中，内存占用稳定且无卡顿。
4. **测试内生化（In-Tree Testing）**：
   全面淘汰外挂式的 Python 探针与黑盒脚本，所有单元测试、集成测试、Mock 桩与基准评测全部使用 Rust 原生编写。
5. **脚本测试日志功能** 每次测试都能显示出反馈从而知道哪些地方出错需要修复和改进
---

## 二、语言与技术栈全景矩阵（参考）

```
┌─────────────────────────────────────────────────────────────────┐
│                    User Interface / Client                      │
│   • TUI: Rust (Ratatui + Crossterm)                            │
│   • Web Console: TypeScript / Modern Web (Embedded in Binary)   │
│   • Desktop (Optional): Tauri v2 (Rust Core + Webview)          │
├─────────────────────────────────────────────────────────────────┤
│                   GQY v2 Agent Harness Engine                    │
│   • Perception Matrix (Host / Workspace / Origin / Context)     │
│   • State Machine & Execution Loop (Tokio Async Core)           │
│   • Byte-Pure Prefix Cache Ledger (Append-only / Fossilize)     │
├─────────────────────────────────────────────────────────────────┤
│                    Storage, IPC & Networking                    │
│   • Embedded DB: SQLite (rusqlite, WAL Mode)                   │
│   • Server / Gateway: Axum (HTTP / SSE / WebSocket)             │
│   • Local IPC: Unix Domain Sockets                              │
├─────────────────────────────────────────────────────────────────┤
│                    System & FFI Primitives                      │
│   • C FFI / Libc: PTY, POSIX Signals, OS Probes, Landlock       │
└─────────────────────────────────────────────────────────────────┘
```

---

## 三、分层技术选型规范

### 1. 核心引擎与 Harness（Core & Harness）
* **主开发语言**：`Rust (2024 Edition / 1.85+)`
* **异步运行时**：`tokio`（多线程核心驱动 + 单线程 local task 隔离）
* **数据序列化**：`serde`, `serde_json`
* **结构化日志与追踪**：`tracing`, `tracing-subscriber`
* **时间与时区**：`jiff`（RFC 3339 / 本地时区；Unix 读系统 tzdb，Windows 默认内嵌 `jiff-tzdb`，WASM 内嵌；存储一律 Unix 毫秒 UTC——10 §4.4。2026-09-28 定，00 §5）
* **前缀缓存契约**：Rust 原生 Append-only 账本状态机，逐字节保持前缀一致性。

### 2. 底层系统交互与沙盒（System FFI & Sandbox）
* **开发语言**：`Rust` + `C / Libc FFI`
* **系统调用与进程**：`libc`, `nix`
* **伪终端（PTY）**：`portable-pty` / 原生 PTY 封装（支持流式输入输出与信号传递）
* **沙盒隔离机制**：
  * Linux：原生 `landlock` 内核级权限收敛
  * macOS：系统级资源隔离与工作区作用域限制

### 3. 数据存储与状态持久化（Storage & Persistence）
* **嵌入式数据库**：`SQLite`（通过 `rusqlite` 绑定）
* **并发控制**：开启 `WAL`（Write-Ahead Logging）模式，读写分离，零锁竞争。
* **数据演进**：纯增量 Migration 策略，确保历史会话数据向前兼容。

### 4. 终端界面（TUI / Terminal）
* **开发语言**：`Pure Rust`
* **渲染引擎**：`ratatui`
* **终端后端**：`crossterm`
* **设计目标**：毫秒级首屏渲染、平滑滚动、丰富组件（代码高亮、多会话切换、任务面板），无 Python / curses 外部依赖。

### 5. 跨平台桌面与 WebUI（Desktop & Web Console）
* **桌面端框架**：`Tauri v2`（Rust 核心宿主 + Web 前端）
  * 打包体积通常 < 15MB，运行时常驻内存仅 ~30MB。
* **Web 控制台技术**：
  * 语言：`TypeScript` + 现代化 Web 标准（HTML5 / CSS3 / ES Modules）
  * 分发模式：编译期通过 Rust `rust-embed` 或 `include_dir` 直接打包内嵌至二进制文件，无需独立部署 Node.js 服务。
* **HTTP / RPC 网关**：`axum`（高性能异步 Web 网关，支持 SSE 流式传输与 WebSocket 实时双工）。

### 6. 测试与基准体系（Testing & Benchmarking）
* **主开发语言**：`Pure Rust`
* **测试套件**：`cargo test`（内置单元测试 + `tests/` 目录集成测试）
* **Mock 与仿真**：`wiremock`（模拟 LLM Provider API 与通讯平台）、`tempfile`（工作区沙箱仿真）
* **性能基准**：`criterion`（前缀缓存命中率基准、Token 吞吐开销评测）



---
## 四、需要考虑的
- 破坏性变更
- 补丁更新
- 强有力的硬限制保持代码健壮性、可维护性

## 五、总结

GQY v2 的技术栈选型以 **「Rust 为体，Web 为面，C 为径」** 为基准：
* 保证了极致的执行性能、内存安全与单一二进制分发便利性；
* 提供了现代化的 TUI / Web / Desktop 多端交互形态；
* 彻底消灭历史积累的技术废料，建立清晰、高内聚、易演进的现代 Agent 架构。


# 此文档理解后可以删除