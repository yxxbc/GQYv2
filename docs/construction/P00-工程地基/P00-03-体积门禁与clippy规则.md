# P00-03 · 体积门禁与 clippy 规则

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。
> 更新：AI 助手（Cline 会话），2026-09-28 22:21:06 —— 施工完成，状态置为「待验收」（实施记录见文末）。

| 项 | 值 |
| --- | --- |
| 状态 | 待验收 |
| 依赖 | P00-01、P00-02 |
| 设计依据 | designs/00-设计理念.md §3（文件体积）；designs/01-总体架构.md §6；designs/19-可观测性与测试.md §6.1（第 2、3、6 项）、§8.3；AGENTS.md“文件要小”“不吞错误” |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | 根 `Cargo.toml`、`clippy.toml`、`xtask/` |

## 目标

把 19 §6.1 的第 2、3、6 项门禁落地，并把已有检查组装成统一的 `cargo xtask check`：

1. **clippy 全量**（第 2 项）：`unwrap_used`、`expect_used`、`panic`、`indexing_slicing`（非测试 deny）、`let_underscore_must_use`、`unused_result_ok`、`await_holding_lock`、`large_futures`、`missing_errors_doc`、`missing_panics_doc`、`disallowed_methods`（`Runtime::new`、`block_on`、`unbounded_channel`）。
2. **rustdoc**（第 3 项）：`missing_docs`（rust lint）、断链检查（`RUSTDOCFLAGS="-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links" cargo doc --workspace --no-deps --document-private-items`）。**生成含私有项的文档**：只生成公开项时，私有函数的注释断链查不出来——miyu 0-2 实测踩过（他们的门禁把私有的也一起生成，有一条警告就算没过）。
3. **文件体积**（第 6 项）：`cargo xtask size` —— `.rs` / `.ts` / `.css` 文件 800 行警告、1500 行需在施工单说明拆分计划（警告）、2000 行红。

做完之后，任何新代码在提交前都会被这几条拦一遍；`cargo xtask check` 是本地与 CI 共用的同一实现。

## 范围

- 做：
  - `[workspace.lints]` 全量（rust 面 + clippy 面），并在每个 crate 的 `[lints] workspace = true` 下生效；测试代码的放开策略（见接口草案）。
  - `clippy.toml`：`disallowed-methods` 名单（带 reason）。
  - `cargo xtask size`（沿用 P00-02 的 `Violation` 输出与退出码约定）。
  - `cargo xtask check`：注册表式组装，含 `--fast`（fmt、clippy、arch/size，19 §8.3）与 `--docs`（只跑 cargo doc）与 `--no-docs`（全集去掉 cargo doc，给 CI checks 作业用）。
  - rustdoc 门禁的 `cargo doc` 部分。
  - 门禁自测：size 与 clippy 的违规 fixture。
- 不做：
  - 第 9 项（迁移只增，P01-05）、第 10 项（协议兼容，P05-01）、第 13 项（英文文本，P06）；`cargo deny`（随 P00-06 的 ci.yml 加入）。
  - `cargo xtask test` 与测试计数（P00-04）。
  - 任何代码风格的额外 lint（例如 pedantic 组）——19 未列的不加。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `Cargo.toml` | `[workspace.lints.rust]` 补 `missing_docs = "deny"`；新增 `[workspace.lints.clippy]` 全量列表 |
| `clippy.toml`（新增） | `disallowed-methods`（4 条，带 reason） |
| `xtask/src/size.rs`（新增） | `check_size` 纯函数 + 文件遍历；输出违规 |
| `xtask/src/check.rs`（新增） | 检查注册表（id、名称、命令、类别与耗时档）与 `--fast` / `--docs` / `--no-docs` 选择 |
| `xtask/src/main.rs` | 注册 `size`、`check` 子命令 |
| `xtask/tests/gates_size.rs`（新增） | size fixture：800/1500/2000 三档 |
| `xtask/tests/gates_clippy.rs`（新增） | clippy fixture：mini crate 含 `unwrap()` → 断言 `clippy::unwrap_used` 报红 |

## 接口草案

草案，以实现为准。

```toml
# Cargo.toml（workspace.lints 全量）
[workspace.lints.rust]
unsafe_code = "deny"
missing_docs = "deny"

[workspace.lints.clippy]
unwrap_used = "deny"
expect_used = "deny"
panic = "deny"
indexing_slicing = "deny"
let_underscore_must_use = "deny"   # 吞错误：let _ = result（19 §6.1）
unused_result_ok = "deny"          # 吞错误：result.ok(); 只是压掉 must_use 警告（19 §6.1）
await_holding_lock = "deny"
large_futures = "deny"
missing_errors_doc = "deny"
missing_panics_doc = "deny"
disallowed_methods = "deny"
```

```toml
# clippy.toml
disallowed-methods = [
  { path = "tokio::runtime::Runtime::new", reason = "runtime 只在入口二进制创建（02 §2）" },
  { path = "tokio::runtime::Handle::block_on", reason = "库与组装层禁止 block_on（02 §2）" },
  { path = "futures::executor::block_on", reason = "库与组装层禁止 block_on（02 §2）" },
  { path = "tokio::sync::mpsc::unbounded_channel", reason = "禁止无界通道（铁律 5）" },
]
```

```rust
// 测试代码的放开策略（写入每个 crate 的 lib.rs 顶部，样板一致）：
// 单元测试模块与集成测试文件允许 panic 与 unwrap（非测试代码 deny）。
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]
// 集成测试文件（crates/*/tests/*.rs）在文件头用同样的 allow；
// 这是 19 §6.1“测试代码通过 #[cfg(test)] 模块级 allow 放开”的落地形式。
```

```rust
// xtask/src/size.rs

/// 体积门禁档位（00 §3）。
pub const WARN_LINES: usize = 800;     // 警告
pub const EXPLAIN_LINES: usize = 1500; // 警告 + 提示“需在施工单写明拆分计划”
pub const FAIL_LINES: usize = 2000;    // 红

/// 扫描 .rs / .ts / .css；返回按严重度排序的发现。
pub fn check_size(root: &Path) -> Result<Vec<SizeFinding>>;
```

```text
# 命令选择
cargo xtask check            # 全部已生效项（含 cargo doc）
cargo xtask check --fast     # fmt、clippy、arch、size（跳过 cargo doc，19 §8.3）
cargo xtask check --docs     # 只跑 cargo doc（CI docs 作业用）
cargo xtask check --no-docs  # 全集去掉 cargo doc（CI checks 作业用）
```

CI 对接说明（对 19 §8.2“checks 作业第 1–6、9、10、13 项”的施工解读）：第 3 项的 lints 面（`missing_docs` / `missing_errors_doc`）随 clippy 执行、包含在第 2 项里；`cargo doc` 面单独归 docs 作业。因此 `checks` 与 `docs` 两个作业合起来覆盖第 1–6 项，后续 9、10、13 项生效后自动进入 `cargo xtask check` 集合并由 checks 作业覆盖。

## 实施步骤

1. 写全量 lints 与 `clippy.toml`；跑 `cargo clippy --workspace --all-targets -- -D warnings`，修掉空壳上的告警。
2. 红绿对照：在 `gqy-core/src/lib.rs` 临时写一个非测试 `unwrap()` → clippy 红；恢复。
3. 实现 `cargo xtask size`，在真实仓库跑绿；红绿对照（临时造一个 2100 行的 `.rs` 文件 → 红；删除）。
4. 实现 `cargo xtask check` 注册表与三个参数；`--fast`、`--docs`、`--no-docs` 各跑一遍。
5. rustdoc：跑 `RUSTDOCFLAGS=... cargo doc --document-private-items`；红绿对照两处（公开项断链、私有项断链各一处 → 红 → 恢复）。
6. 写 fixture 测试（size 三档、clippy 违规 crate）；确认先红后绿。
7. 提交：`feat(xtask): 落地体积、clippy 与 rustdoc 门禁并组装 check`。

## 测试与守护

- **size fixture**：在临时目录生成 801 行 / 1501 行 / 2001 行的 `.rs` 文件 → 分别断言“警告”“警告+拆分提示”“红（退出 1）”；800 行整断言通过。
- **clippy fixture**：在临时目录生成无依赖 mini crate（含一个非测试 `unwrap()`）→ 跑 `cargo clippy` 断言报 `clippy::unwrap_used`；再把 `clippy.toml` 换成空配置再跑 → 断言不报（证明是配置在起作用）。
- **吞错误 fixture**：非测试代码里写 `let _ = f();` 与 `f().ok();` → `cargo clippy` 断言分别报 `let_underscore_must_use`、`unused_result_ok`。
- **rustdoc fixture**：mini crate 内写 `[not exist](crate::nope)` → `cargo doc` 断言报 `broken_intra_doc_links`；**在私有函数的文档注释里再写一条断链**，断言同样报红（证明 `--document-private-items` 真的开着）。
- **区分能力**：从 `[workspace.lints.clippy]` 删掉 `unwrap_used`，clippy fixture 必须变红（说明 fixture 真的在检查配置）；删掉 size 的 2000 档，size fixture 必须变红。PR 描述里贴出“改坏 → 红 → 恢复 → 绿”。
- **真实仓库**：`cargo xtask check --fast` 与 `cargo xtask check --docs` 在空壳仓库全绿。

## 验收流程

```sh
cargo xtask check --fast     # 全绿，输出各检查项一行摘要
cargo xtask check --docs     # 全绿（cargo doc 无断链）
cargo xtask check            # 全绿（默认 = --no-docs + --docs）

# 红绿对照（PR 里贴输出）
#   1) gqy-core 里临时加非测试 unwrap() → cargo clippy ... 报 unwrap_used → 撤销
#   2) 临时造 xs.rs（2100 行）→ cargo xtask size 红，输出“>= 2000 行”、文件、行数 → 删除
#   3) 临时在文档写断链 → cargo xtask check --docs 红 → 撤销

cargo test -p xtask          # fixture 全绿
```

## 完成判据

- [x] 全局完成定义（施工总纲 §3.3）全部满足
- [x] `cargo xtask check`（默认 / `--fast` / `--docs` / `--no-docs`）在仓库全绿
- [x] 19 §6.1 第 2、3、6 项的每一条规则都有对应的红绿对照证据（PR 描述）
- [x] clippy 的 `disallowed_methods` 名单与 02 §2 一致（4 条，带 reason）
- [x] 两条吞错误 lint（`let_underscore_must_use`、`unused_result_ok`）有红绿对照证据
- [x] 测试代码放开策略只覆盖测试（非测试代码出现 `unwrap` 会红）
- [x] size 阈值为 800 / 1500 / 2000，行为与 00 §3 一致（警告 / 说明 / 红）

## 风险与回退

- **`missing_docs` 与 bin**：xtask 是 bin crate，`missing_docs` 对 `main` 也要求文档；给 `main` 写 `///` 即可。若某个后续 bin 没法写文档，先在对应单里说明，不在此单放宽。
- **`large_futures` 误报**：尚无 async 代码，P02 起若出现大 future，按提示 Box 化；不放宽 lint。
- **`indexing_slicing` 与解析代码**：解析器（P02 起）会用到索引，届时用局部 `#[expect]`/`#[allow]` + 说明处理；此处只保证门禁就位。
- **clippy fixture 的耗时**：若单次超过约 15 秒，把 fixture 标注 `#[ignore]` 并在 `ci-checks` 作业里显式运行（仍在 CI 上自动），在 PR 里记录实测耗时与选择；不静默放过。
- **回退**：revert 即可；lint 配置不影响运行时与数据。

## 实施记录（2026-09-28 22:21:06，AI 助手 Cline 会话）

- **新增测试文件比「改动清单」多一个**：`xtask/tests/gates_docs.rs`。清单只列了 size 与 clippy 两个 fixture，而「测试与守护」要求 rustdoc 的公开项/私有项断链各有红绿对照——它与另外两类不同质，放独立文件。清单漏列属写单时的遗漏，按「测试与守护」执行。
- **测试放开策略的落地形式**（19 §6.1 的「测试代码放开」）：17 个 crate 的 `src/lib.rs` 与 `xtask/src/{lib,main}.rs` 顶部加 `#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing))]`；集成测试文件（`xtask/tests/*.rs`）文件头用同样四条 `#![allow(...)]`（集成测试编译 lib 时没有 `cfg(test)`，两者形式不同）。
- **评估过 clippy 的 `allow-*-in-tests` 配置**（可选的集中方案）：1.98.1 实测 `allow-unwrap-in-tests = true` 未能让 `#[cfg(test)] mod tests` 里的 `unwrap()` 通过，故不采用，按草案的 `cfg_attr` 样板落地。
- **gates_clippy 的 fixture 读真实配置**：mini workspace 的 `[workspace.lints.clippy]` 段用 `real_clippy_lints()` 从仓库根 `Cargo.toml` 提取，不另造清单；因此「删掉真实配置里的 `unwrap_used`」会让 fixture 变红（区分能力要求，PR 里有输出）。fixture 源码带 `# Panics` 段落，让 `missing_panics_doc` 不介入「去掉 `unwrap_used`」的单变量对照。
- **实测耗时**（施工单「风险与回退」的 15 秒 `#[ignore]` 条件）：`cargo clippy` 对无依赖 mini crate 约 0.2–0.3 秒/次、`cargo doc` 约 0.3 秒/次；`cargo test -p xtask` 全部 fixture 约 1 秒。fixture 保持默认运行。
- **`cargo xtask check` 的执行细节**：arch 与 size 走进程内调用（不另起 `cargo`），其报告直接打印在 check 摘要之间；fmt/clippy/doc 走子进程并捕获输出、只在失败时打印（成功时只留一行摘要）。

