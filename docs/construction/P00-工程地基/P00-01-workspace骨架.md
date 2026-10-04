# P00-01 · workspace 骨架

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 已完成 |
| 依赖 | —（本阶段起点） |
| 设计依据 | designs/01-总体架构.md §2、§3、§6；docs/tech-stack.md；designs/19-可观测性与测试.md §6.2 |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | 根 `Cargo.toml`、`rust-toolchain.toml`、`crates/`（17 个库空壳） |

## 目标

建立 Cargo workspace：固定工具链、edition 2024、按 01 §3 的层表建出全部 17 个库 crate 空壳，打开第一版 workspace lints，并把 `Cargo.lock` 入库。做完之后，后续每一张施工单都在这个骨架上加代码；空壳上 `cargo build --workspace`、`cargo test --workspace`、`cargo fmt --all --check`、`cargo clippy --workspace --all-targets -- -D warnings` 全绿。

“全部 crate 空壳”的口径：**只建 `crates/` 下的 17 个库 crate**（01 §3 表）。`xtask/` 由 P00-02 创建；`apps/gqy`、`apps/tui`、`apps/web-console`、`apps/desktop`、`apps/connectors` 分别由 P05-10、P05-13、P10-01、P16-06、P14 创建（P05 重排后更新的引用）。理由：每个入口的骨架与它的首张施工单一起出现，避免空 bin 长期存在（“不为以后写代码”）。

## 范围

- 做：
  - 根 `Cargo.toml`：`[workspace]`（members、resolver）、`[workspace.package]`（version / edition / rust-version / publish）、`[workspace.lints.rust]` 的第一版（`unsafe_code = "deny"`）；`[workspace.dependencies]` 本单保持为空段或暂不存在（第一处共享依赖在 P00-05 引入）。
  - `rust-toolchain.toml`：固定 stable 版本与组件（clippy、rustfmt）。
  - `crates/` 下 17 个 crate 空壳，每个只有 `Cargo.toml` 与 `src/lib.rs`（`//!` 模块文档，说明职责与设计出处）。
  - `Cargo.lock` 入库（`cargo build` 生成）。
- 不做：
  - `xtask/`、`.cargo/config.toml`（P00-02）；
  - clippy 全量 lints、`clippy.toml`、文件体积门禁（P00-03）；
  - 各 crate 的真实依赖、类型、错误定义（P01 起逐单添加）；
  - `apps/` 下任何入口（各自的施工单，见上）；
  - release-please 对 `Cargo.toml` 版本字段的自动更新与一致性检查（P16-04；本单阶段手工保持一致，见“风险与回退”）；
  - 自定义 `[profile]` 调优（没有测量前不调优；发布构建配置在 P16-04 定）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `Cargo.toml`（新增） | workspace 根：members、resolver、`[workspace.package]`、`[workspace.lints.rust]` |
| `rust-toolchain.toml`（新增） | `channel` 固定为施工当天的 stable；`components = ["clippy", "rustfmt"]` |
| `.gitattributes`（新增） | 所有文本文件一律 LF（`* text=auto eol=lf`），二进制夹具显式标记 `binary`（`*.db`、`*.png` 等） |
| `crates/gqy-core/Cargo.toml` 等 17 个（新增） | 包元数据全部 `.workspace = true` 继承；`[lints] workspace = true` |
| `crates/*/src/lib.rs`（新增 ×17） | 仅 `//!` 模块文档：一句话职责 + 设计出处（01 §3、对应设计文档） |
| `Cargo.lock`（新增） | 首次 `cargo build --workspace` 生成，入库 |
| `CHANGELOG.md` | 不加条目：本单无用户可见变化；PR 由维护者加 `skip-changelog` 标签 |

17 个 crate（照 01 §3，从低层到高层）：`gqy-core`、`gqy-protocol`、`gqy-sys`、`gqy-store`、`gqy-config`、`gqy-provider`、`gqy-ledger`、`gqy-tools`、`gqy-perception`、`gqy-memory`、`gqy-ext`、`gqy-engine`、`gqy-gateway`、`gqy-client`、`gqy-mesh`、`gqy-connector-sdk`、`gqy-daemon`。

## 接口草案

草案，以实现为准。

```toml
# Cargo.toml（根）
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"          # 与 version.txt / .release-please-manifest.json 当前值一致
edition = "2024"
rust-version = "<施工当天 stable，≥1.85>"   # 与 rust-toolchain.toml 的 channel 同一数值
publish = false

[workspace.lints.rust]
unsafe_code = "deny"       # 仅 gqy-sys 允许模块级 allow + // SAFETY:（01 §6；该豁免在 P07-04 落地时按需添加）
```

```toml
# rust-toolchain.toml
[toolchain]
channel = "<施工当天 stable，例如 1.x.y>"
components = ["clippy", "rustfmt"]
```

```toml
# crates/gqy-core/Cargo.toml（其余 16 个同构，只改 name）
[package]
name = "gqy-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
publish.workspace = true

[lints]
workspace = true

[dependencies]
# 本单为空；各 crate 的依赖随使用它的施工单添加
```

```rust
// crates/gqy-core/src/lib.rs
//! L0 基础类型：ID、`Clock`、规范化 JSON、错误分类、安全文本工具与限额常量。
//! 设计：01 §3、02 §4、10 §4。
//!
//! 分层规则见 01 §3：本 crate 不得依赖任何更高层的 crate。
```

（每个空壳的 `//!` 都按此格式：第一句职责，第二句设计出处；没有公开项，`missing_docs` 门禁在 P00-03 打开后依然通过。）

## 实施步骤

1. 工具链：`rustup update stable`，记录 `rustc -V`；把版本号同时写进 `rust-toolchain.toml` 与 `rust-version`（两者必须相同）。下限 1.85（tech-stack「Rust 2024 Edition / 1.85+」、01 §6）。**升级工具链单独走一张单，不随手升**——clippy 每个版本会加新检查，跟着 stable 漂会出现“没改一行代码，CI 变红”（参考 gqy-agent-remake 施工单 0-1）。
2. 写根 `Cargo.toml`、`rust-toolchain.toml` 与 `.gitattributes`。`.gitattributes` 的作用是钉住换行：Windows 检出默认把文本换成 CRLF，会让按字节比对的夹具（04 的账本字节契约、19 §4 的 sha256 形状夹具）与 shell 脚本在不同平台上对不上（参考 gqy 0-3 的实测：没有它，字节级样本在 Windows 上必错）。
3. 生成 17 个空壳（可用一次性脚本生成，脚本不入库）。核对名单与 01 §3 完全一致。
4. `cargo build --workspace` 生成 `Cargo.lock`；`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace` 全部跑绿。
5. 自审：每个 `lib.rs` 的文档注释是否写清职责与出处；是否有多余文件（`.DS_Store` 等）。
6. 整理提交（`feat(workspace): 搭建 Cargo workspace 与全部库 crate 骨架` 之类，标题按 Conventional Commits），推分支跑 PR；PR 描述里贴步骤 4 的输出与工具链版本。

## 测试与守护

本单不引入任何行为，没有业务测试；它的守护对象是“结构”，按全局完成定义（施工总纲 §3.3）在本单交付范围解释执行：

- **自动检查**：`cargo build / test / fmt / clippy` 在本地（macOS）全绿。三平台验证在 P00-06 的 CI 矩阵接入后补做，本单 PR 里注明“三平台验证待 P00-06”。
- **结构断言**（手工，写进 PR 描述）：`cargo metadata --format-version 1` 输出中恰好包含上述 17 个包，且逐个对照 01 §3 名单；`edition` 全部为 2024；`Cargo.lock` 已被 `git ls-files` 收录。
- **区分能力说明**：本单没有可“去掉变红”的实现（交付物是配置与空壳），所以先在 PR 描述里给出“工具链版本 + 四命令输出”作为证据；从 P00-02 起，每张单都必须带先红后绿的测试。

## 验收流程

在本机（macOS 或 Linux）依次执行，期望：

```sh
rustup show                                   # active toolchain 与 rust-toolchain.toml 一致
cargo build --workspace                       # 全绿
cargo test --workspace                        # test result: ok，0 failed（允许 0 tests）
cargo fmt --all --check                       # 无输出
cargo clippy --workspace --all-targets -- -D warnings   # 无警告
ls crates | wc -l                             # 17
git ls-files Cargo.lock                       # 输出 Cargo.lock
cargo metadata --format-version 1 | python3 -c \
  "import json,sys; m=json.load(sys.stdin); print(sorted(p['name'] for p in m['packages']))"
```

最后一条输出应为 17 个 `gqy-*` 包名（逐个对照 01 §3）。Windows 上的“能编译、能跑单元测试”由 P00-06 的 `test-windows` 作业首次验证。

## 完成判据

- [x] 全局完成定义（施工总纲 §3.3）全部满足（其中 cargo/xtask 检查按本单交付范围解释）
- [x] 17 个 crate 空壳与 01 §3 名单一一对应，无多余、无遗漏
- [x] `rust-toolchain.toml` 的 channel 与 `rust-version` 为同一版本，且 ≥ 1.85，数值记录在 PR 描述
- [x] `Cargo.lock` 入库
- [x] `Cargo.toml` 的 `version` 与 `version.txt` 一致（当前均为 `0.1.0`）
- [x] 四个命令（build / test / fmt / clippy）输出贴进 PR 描述

## 风险与回退

- **版本漂移**：`Cargo.toml` 的 `version` 目前与 Release Please 的 `version.txt` 无自动联动（release-please 的 `simple` 类型不知道 Cargo.toml）。在 P16-04 做版本一致性检查前，发布 PR 合入后要手工核对三处（`Cargo.toml` / `version.txt` / tag）；本单在 PR 描述里写明这一点。
- **edition 2024 与工具链差异**：个别 stable 版本对 edition 2024 的 lints 有差异；工具链已固定，若 CI 与本地不一致，以 `rust-toolchain.toml` 为准排查。
- **members 通配**：`members = ["crates/*"]` 只覆盖 `crates/`；往 `apps/` 加包时由对应施工单显式加入 members（避免把尚无 `Cargo.toml` 的目录写进 members 导致 cargo 报错）。
- **回退**：本单是纯新增，`git revert` 即可；不涉及用户数据与兼容性。

