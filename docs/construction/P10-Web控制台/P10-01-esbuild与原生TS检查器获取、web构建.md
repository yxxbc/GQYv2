# P10-01 · esbuild 与原生 TS 检查器获取、web 构建

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P09-07 |
| 设计依据 | designs/15-Web控制台与桌面.md §3.1（esbuild 独立二进制与固定表、缓存与离线覆盖）、§3.2（tsgo 与 `--noEmit`）、§3.3（`web build / check / watch` 子命令）、§11（校验和不符、缺少产物等失败模式）；designs/00-设计理念.md §4（不引入 Node）、§5（2026-09-28 前端工具链与 TS 类型检查已定）；designs/19-可观测性与测试.md §6.1 第 12 项（`web check` 是门禁，本单先接类型检查） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `xtask`（`src/web/*`）、`apps/web-console`（入口与 tsconfig）、`.github/workflows/ci.yml` |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

`cargo xtask web build / check / watch` 可用且**零 Node**：esbuild 与 tsgo 按固定表下载并校验 sha256，缓存到 `target/gqy-tools/`；构建产出 `apps/web-console/dist/` 与 `dist/manifest.json`（源树哈希 + esbuild 版本）。做完之后，离线环境可用 `GQY2_ESBUILD` / `GQY2_TSGO` 指向本地副本（仍校验版本），没有 node / npm 的 PATH 下也能构建与检查。

## 范围

- 做：
  - `xtask/src/web/toolchain.rs`：`PinnedTool` 表（esbuild ×4 平台、tsgo ×4 平台：name / version / target / url / sha256）；esbuild 从 npm 注册表的平台包 tarball（`@esbuild/<platform>`）HTTP 下载并解包取二进制，**不调用 npm / Node**；校验失败 → 不使用该文件、删除缓存、错误写清期望与实际哈希（15 §11）。
  - 缓存目录 `target/gqy-tools/<name>-<version>/`；`GQY2_ESBUILD` / `GQY2_TSGO` 指离线副本，仍校验 `--version` 与固定版本一致，不一致拒绝。
  - `cargo xtask web build [--release]`：esbuild 打包 `apps/web-console/src/app/main.ts`（bundle、ESM；release 加 minify，debug 带 sourcemap）→ `dist/` + `dist/manifest.json`（源文件树哈希、esbuild 版本、构建时间）。
  - `cargo xtask web check`（本单先做类型检查部分）：`tsgo --noEmit` + tsconfig 严格档；分层与体积随 P10-04 接入。
  - `cargo xtask web watch`：监视源文件增量重建到 `dist/`，配合 `GQY2_WEB_DIR` 使用（覆盖逻辑在 P10-03）。
  - 最小 `apps/web-console/`：`index.html`、`src/app/main.ts` 占位、`tsconfig.json`，作为构建输入与 P10-04 的骨架起点。
- 不做：
  - 类型生成（P10-02）、分层与体积检查（P10-04）、内嵌与安全头（P10-03）、真正的前端界面（P10-04 起）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `xtask/src/web/{mod.rs,toolchain.rs,build.rs,check.rs,watch.rs}`（新增） | 固定表、下载校验、构建、检查、监视 |
| `xtask/src/main.rs`（修改） | 注册 `web build / check / watch` |
| `apps/web-console/{index.html,tsconfig.json}`、`src/app/main.ts`（新增） | 构建入口与 TS 工程 |
| `xtask/Cargo.toml`（修改） | tarball 解包与哈希依赖 |
| `.github/workflows/ci.yml`（修改） | `web check` 步骤（门禁化随 P10-04） |

## 接口草案

草案，以实现为准。

```rust
// xtask/src/web/toolchain.rs（照 15 §3.1）
pub struct PinnedTool { pub name: &'static str, pub version: &'static str, pub target: &'static str,
                        pub url: &'static str, pub sha256: &'static str }
pub const ESBUILD: &[PinnedTool] = &[/* linux-x64, linux-arm64, darwin-x64, darwin-arm64 */];
pub const TSGO: &[PinnedTool] = &[/* 同上四平台 */];

pub async fn ensure(tool: &PinnedTool, cache: &Path) -> Result<PathBuf, ToolError>;  // 下载 + 校验 + 缓存
pub fn verify_version(bin: &Path, expected: &str) -> Result<(), ToolError>;          // 离线覆盖仍校验

// xtask/src/web/build.rs
pub struct Manifest { pub source_hash: String, pub esbuild_version: String, pub built_at: String }
pub async fn bundle(ws: &Workspace, release: bool) -> Result<Manifest, WebError>;
```

## 实施步骤

（L 单：每一步都能单独编译通过。）

1. 固定表与 `ensure`（先写「哈希不符 → 拒绝并清缓存」的会红测试）。
2. tarball 解包与缓存；离线覆盖的版本校验。
3. `web build` 与 `manifest.json`（源树哈希）。
4. `web check`（tsgo `--noEmit`）与 tsconfig 严格档。
5. `web watch` 增量重建。
6. `cargo xtask check --fast`、`cargo test -p xtask`；提交：`build(web): esbuild 与 tsgo 工具链、web 构建`。

## 测试与守护

- **校验和**：改固定表一位 → 报红，错误含期望与实际哈希；缓存被清除、坏文件不落盘（去掉校验，测试红）。
- **版本覆盖**：`GQY2_ESBUILD` 指向错误版本 → 拒绝；指向正确版本 → 不联网复用（测试断言无网络调用）。
- **零 Node**：在 PATH 中移除 node / npm（本地用 `env -i`、CI 天然满足）后 `web build` 成功。
- **幂等与源树哈希**：连续两次 build，manifest 的源树哈希相同；改一个字节 → 哈希变化（去掉哈希计算，测试红）。
- **watch**：文件事件触发一次重建的单测（重复事件合并）。
- **解包安全**：夹具 tarball 中的目录穿越条目被拒绝。
- 先红后绿对照（PR 贴输出）：去掉 sha256 校验、去掉源树哈希各一次。

## 验收流程

```sh
cargo xtask web build && cat apps/web-console/dist/manifest.json
cargo xtask web check
GQY2_ESBUILD=/path/to/esbuild cargo xtask web build   # 离线副本（仍校验版本）
env PATH=/usr/bin:/bin cargo xtask web build           # 无 node / npm 也能构建
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 固定表覆盖四个平台（linux / darwin × x64 / arm64），版本与校验和写死在代码里
- [ ] 零 Node：构建与检查不调用 npm / node（CI 断言）
- [ ] 校验和不符与离线覆盖两条失败路径各有区分能力测试
- [ ] `dist/` 进 `.gitignore`；`manifest.json` 供 P10-03 的 build.rs 判定

## 风险与回退

- **tsgo 发布渠道与平台覆盖**（README 复核清单第 2 条）：开工前核实 URL 与校验和；覆盖不了四平台时先并入 15 并写明降级口径。
- **npm 注册表可达性**：只影响首次下载；镜像环境配置只是“URL 换源”，不引入 Node。
- **回退**：`xtask/src/web` 是新模块；revert 后无 `web` 子命令，仓库其它部分不受影响。
