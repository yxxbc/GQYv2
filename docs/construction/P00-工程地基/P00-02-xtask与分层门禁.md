# P00-02 · xtask 与分层门禁

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 待验收 |
| 依赖 | P00-01 |
| 设计依据 | designs/01-总体架构.md §3、§6；designs/19-可观测性与测试.md §6.1（第 4、5 项）、§7、§8.3；AGENTS.md“依赖只朝一个方向”“上下文显式” |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `xtask/`、`.cargo/config.toml`、根 `Cargo.toml` |

## 目标

建立 `xtask` 开发工具 crate（`cargo xtask` 别名可用的骨架），落地两类机器门禁：

1. **层序门禁** `cargo xtask arch`：读取 `cargo metadata`，逐条核对 01 §3 的层表与附加约束，**不设白名单**，违规即红（铁律 4）；层表直接解析 01 §3 的文档表格，代码里不另存副本（参考 miyu 0-2：“图纸就是真相源”）；
2. **静态规则门禁**（19 §6.1 第 5 项中属于 P00 的三条）：`task_local!`（铁律 3）、可变全局 `static`（02 §8）、`#[instrument]` 必须带 `skip_all`（19 §3.1）。

同时建立**门禁自测机制**：每条规则都有违规 fixture（断言报红）与合规 fixture（断言通过），去掉规则实现后 fixture 测试必须变红——门禁自身也要有区分能力（19 §6.1 末尾）。

做完之后，任何人把依赖接错方向、写出被禁模式，`cargo xtask arch` 会拦下来并给出期望值与实际值。

## 范围

- 做：
  - `xtask` crate 骨架：子命令分派（`arch`）、统一退出码（通过 0、违规 1、工具错误 2）、帮助文本；`anyhow` 只在这里使用（01 §6）。
  - `.cargo/config.toml` 的 `[alias] xtask = "run --package xtask --"`。
  - 层表来源：直接解析 `docs/designs/01-总体架构.md` §3 的表格。解析结果含 17 个库 crate + `gqy`、`gqy-tui`、`gqy-desktop`、连接器，含“尚未创建”的包——层表是设计事实，施工进度不改变它；解析失败要指出是哪一行。`xtask` 自身是开发工具，不进层表（在规则里如实豁免，不搞隐性例外）。
  - `check_arch`：包归属检查（workspace 里的包必须在层表内）、依赖方向检查（只允许指向更低层）、附加约束（`gqy-tui` / 连接器 / 桌面前端不得依赖 L1–L4；只有 `gqy-daemon` 可同时依赖 `gqy-engine` 与 `gqy-gateway`；`gqy-connector-sdk` 只依赖 `gqy-protocol`）、重依赖白名单（reqwest→provider、axum/rust-embed→gateway、rusqlite→store、ratatui/crossterm→tui、tauri→desktop）。
  - 静态规则扫描：`task_local!`、`static` 行含 `Mutex|RwLock|RefCell`、`#[instrument...]` 属性内无 `skip_all`；扫描范围 `crates/*/src/**/*.rs` 与 `apps/**/*.rs`（`xtask/` 自身除外；测试目录 `tests/` 与 `#[cfg(test)]` 内代码首版不豁免——被禁模式对测试同样成立，唯一例外是 `OnceLock` 只读值不匹配可变全局规则）。
  - `xtask/tests/` 的门禁自测 fixture。
- 不做：
  - `size`、`check` 组装、`docs`（P00-03）；`test`（P00-04）；`fixtures` / `web` / `gen-ts` / `plan` 等后续子命令（各自的施工单）。
  - 静态规则的其余项：`deny_unknown_fields`（P05-01 随协议加入）、TUI 绕过帧缓冲（P09）、`disallowed_methods`（P00-03 由 clippy 覆盖，不在扫描器里重复实现）。
  - `#[instrument]` 的字段白名单检查（只查 `skip_all` 是否存在）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `.cargo/config.toml`（新增） | `[alias] xtask = "run --package xtask --"` |
| `Cargo.toml` | members 加 `"xtask"` |
| `xtask/Cargo.toml`（新增） | bin crate；依赖：`clap`（derive）、`serde_json`、`anyhow`；dev 依赖：`tempfile` |
| `xtask/src/lib.rs`（新增） | 库入口：检查逻辑放库里，`tests/` 才能直接调用纯函数（实施时的补充，见 PR 说明） |
| `xtask/src/main.rs`（新增） | 子命令分派与退出码 |
| `xtask/src/arch/mod.rs`（新增） | `ArchCmd` 入口：跑 `cargo metadata`、汇总两类检查、打印报告 |
| `xtask/src/arch/layers.rs`（新增） | 从 01 §3 解析层表（`parse_layers`）与附加约束规则、`check_arch` 纯函数 |
| `xtask/src/arch/scan.rs`（新增） | 静态规则扫描器，输入为文件路径列表与源码文本，输出违规列表 |
| `xtask/tests/arch.rs`（新增） | 层序门禁 fixture：违规/合规 mini workspace |
| `xtask/tests/scan_rules.rs`（新增） | 静态规则 fixture：三条规则的违规/合规样例 |

## 接口草案

草案，以实现为准。

```rust
// xtask/src/arch/layers.rs

/// 一个包在 01 §3 分层表中的位置。
pub struct LayerEntry {
    /// 包名（crates.io 包名，即 `gqy-*`）。
    pub name: &'static str,
    /// 层号；越小越底层（L0 = 0 … L7 = 7）。
    pub layer: u8,
    /// 允许出现的外部重依赖前缀（01 §3“允许的外部重依赖”列的机器可读形式）。
    pub allowed_heavy_deps: &'static [&'static str],
}

/// 从 `docs/designs/01-总体架构.md` 的 §3 分层表解析出“包 → 层号 → 允许重依赖”。
/// 解析失败返回带行号的错误（例如“第 N 行：层号列缺失”），让改图纸的人一眼看出哪里破坏了格式。
pub fn parse_layers(doc: &str) -> Result<LayerTable, LayerParseError>;

/// 对 `cargo metadata` 的结果做层序核对。纯函数：便于用 fixture 单测。
///
/// 返回违规列表；空列表 = 通过。违规信息包含规则名、涉及包/文件、
/// 期望值与实际值（19 §4.1：失败输出要能定位），并用中文说明违反的是 01 §3 的哪一条。
pub fn check_arch(meta: &serde_json::Value, table: &LayerTable) -> Vec<Violation>;

pub struct Violation {
    /// 例如 "layer-order" / "unknown-package" / "heavy-dep" / "task-local"。
    pub rule: &'static str,
    /// 例如 "gqy-core -> gqy-store"。
    pub location: String,
    pub expected: String,
    pub actual: String,
}
```

```rust
// xtask/src/arch/scan.rs

/// 扫描一条源码文本，返回违规（行号从 1 开始）。
/// `path` 只用于组装违规信息，便于测试直接传字符串。
pub fn scan_source(path: &str, source: &str) -> Vec<Violation>;

/// 扫描真实仓库：crates/*/src/**/*.rs、apps/**/*.rs。
pub fn scan_repo(root: &Path) -> Result<Vec<Violation>>;
```

```text
# 命令与退出码
cargo xtask arch          # 层序 + 静态规则；全部通过退出 0
                          # 有违规：逐条打印（规则 / 位置 / 期望 / 实际），退出 1
                          # 工具自身出错（cargo metadata 失败等）：退出 2
```

## 实施步骤

1. 建 `xtask` 骨架与 `.cargo/config.toml` 别名；`cargo xtask arch` 先打印“未实现”并退 2，确认管线通。
2. 实现 `parse_layers`：解析 `docs/designs/01-总体架构.md` §3 的表格与附加约束清单；先写解析测试（合规文档、缺列、坏层号）。
3. 实现 `check_arch` 纯函数；在真实 workspace 上跑（空壳期无依赖，只验证“包都在层表里”；`xtask` 与尚未创建的包按规则豁免）。
4. 实现 `scan_repo`；在真实仓库跑绿（空壳期没有源码模式）。
5. 写 fixture 测试（先写违规样例、断言报红；再补合规样例、断言通过）。
6. `cargo xtask arch` 接进日常流程；`--help` 文本写清每个子命令。
7. 提交：`feat(xtask): 建立 xtask 与分层/静态规则门禁`。

## 测试与守护

- **层序 fixture**（`xtask/tests/arch.rs`，用 `tempfile` 生成 mini workspace 后跑真实 `cargo metadata`）：
  1. 违规：低层包依赖高层包（`gqy-core` → `gqy-store`）→ 断言报 `layer-order`，`expected` 含“指向更低层”；
  2. 违规：附加约束（`gqy-tui` 依赖 `gqy-engine`）→ 断言报错；
  3. 违规：未知包（workspace 里出现层表外的包）→ 断言报 `unknown-package`；
  4. 合规：与层表一致的依赖图 → 断言 0 违规；
  5. 文档解析：内嵌 01 §3 样本（合规一份、缺列与坏层号各一份）→ 断言解析成功 / 报错带行号。
- **文档即真相源**：解析器读的是仓库里的真实 `docs/designs/01-总体架构.md`。验收时演示一次：把表里某个 crate 的行删掉 → 该 crate 变成 `unknown-package` 报红；恢复。
- **静态规则 fixture**（`xtask/tests/scan_rules.rs`，直接喂源码字符串，免编译）：
  `task_local! { static X: u32 = 0; }` / `static CACHE: Mutex<u32> = ...` / `#[instrument]` 无 `skip_all` 各自断红；对应的 `OnceLock`、`#[instrument(skip_all, fields(x))]` 断言通过。
- **区分能力**：删掉 `check_arch` 里任一条规则，对应的 fixture 测试必须红；删掉扫描器的任一条模式同理。PR 描述里贴出“注释掉规则 → 测试红 → 恢复 → 绿”的输出。
- **真实仓库绿**：`cargo xtask arch` 在空仓骨架上退出 0；P00-02 之后每次加依赖的施工单都要跑它（P00-06 接进 CI）。

## 验收流程

```sh
cargo xtask arch                    # 退出码 0；打印检查摘要（包数、检查项）
cargo test -p xtask                 # fixture 全部通过

# 手工红绿对照 1：依赖方向
#   在 crates/gqy-core/Cargo.toml 临时加 [dependencies] gqy-store = { path = "../gqy-store" }
cargo xtask arch                    # 退出码 1，输出含 "layer-order"、"gqy-core -> gqy-store"、期望/实际
#   撤销后再次运行，退出码 0

# 手工红绿对照 2：静态规则
#   在 crates/gqy-core/src/lib.rs 临时加 task_local! 写法
cargo xtask arch                    # 退出码 1，输出含文件与行号
#   撤销后退出码 0
```

## 完成判据

- [x] 全局完成定义（施工总纲 §3.3）全部满足（xtask 检查即本单交付）
- [x] `cargo xtask arch` 在真实仓库跑绿；`cargo test -p xtask` 全绿
- [x] 层表与 01 §3 逐行一致（PR 描述里贴对照或说明核对方式）
- [x] 两类检查共 7 个 fixture 用例（4 层序 + 3 静态规则中的违规/合规对），全部自动化
- [x] 违规输出含规则名、位置、期望值、实际值
- [x] `cargo xtask --help` 与 `cargo xtask arch --help` 可用

## 风险与回退

- **`cargo metadata` 的调用**：用子进程调用并与 `rust-toolchain.toml` 一致；离线可用（空壳无外部依赖）。若 CI 首次拉取索引慢，属于 P00-06 的缓存问题。
- **扫描器误报**：字符串/注释中出现 `task_local!` 会误报。首版接受（宁可多报），若实际遇到，先把该行改写而不是放宽规则；确需放宽时先改设计文档。
- **图纸格式是门禁的输入**：01 §3 表格的列可以加，改列义要同步解析器；解析失败时错误带行号，改图纸的人一看就知道（miyu 0-2 已验证过的取舍）。
- **规则集按阶段扩展**：本单只落地层序 + 三类静态规则；后续单按 19 §6.1“随各阶段加入”的原则扩展同一扫描器（例如 04 §7 的 `gqy-ledger` 禁 IO、10 §3 的 `.gqy2` 字面量、10 §4.4 的 repo 纪律），不另建机制。
- **fixture 里建 mini workspace**：`cargo metadata` 对无依赖 mini workspace 很快（毫秒级）；若在 CI 上变慢，把 fixture 断言改在纯函数层（strings）而保留一个端到端用例。
- **回退**：本单只新增开发工具，revert 即可；不影响运行时与数据。

