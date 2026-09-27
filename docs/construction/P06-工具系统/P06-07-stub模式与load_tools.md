# P06-07 · stub 模式与 load_tools

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P06-01 |
| 设计依据 | designs/07-工具系统.md §6（加载模式与 load_tools 契约、stub 描述格式、`available_load_targets`）、§5.2（序列化与形态）、§2（v1 的 stub 字节教训）、§10（load_tools 行）；designs/04-前缀缓存账本.md §8.3（stub 形态的字节）、§8.5–§8.6（冻结与「每回合至多一次变更」）；designs/00-设计理念.md §5（「工具三态与分组加载」，P06 施工前并入 07 §6） |
| 规模 | M（1–2 天） |
| 涉及 crate / 目录 | `crates/gqy-tools`（`stub.rs`、`groups.rs`、`builtin/load_tools.rs`、`surface.rs`）、`crates/gqy-config`（`[tools] loading_mode` 与分组默认值） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

默认 `LoadingMode::Stub` 生效：Lazy 工具以「真名 + `summary (stub)` + 最小参数壳」进入 tools 数组，`load_tools` 按名/按组返回完整契约（落在对话尾部，不碰前缀）。做完之后，工具面在会话内字节恒定（stub 字节由固定代码路径产出）；模型拿完整 schema 只有 `load_tools` 一条路；直接调用 stub 也能被收口兜住并得到 hint。

## 范围

- 做：
  - 形态生成：`Stub` 与 `Hybrid` 的初始形态（`Full` 已在 P06-01）；`Core` 驻留工具在三种模式下都是完整形态（07 §6）。
  - stub 描述：有 `summary` → `<summary> (stub)`；声明 `stub_example` → `<summary> (stub, e.g. {…})`；参数壳用固定字节（04 §8.3；**与 07 §6 的表述差异见 README 复核第 4 条**）；stub 的真实参数直接写在调用顶层，执行时无需拆壳。
  - 分组与三态（00 §5 决策，先并入 07 §6）：工具声明分组；`Full/Stub/Off` 三态 + 按组提升；默认只全量「代码基础组」；`Off` 的工具不进工具面。
  - `load_tools`：`{"names": [...]}`（≤ 16 项，含 `group:<name>`）；结果每工具一段 `<tool name="…"><description>…</description><schema>…</schema></tool>`，字节来自同一规范化函数；重复加载返回 `already loaded in this session` 与契约哈希；描述为常量字节，不嵌目录。
  - `<available_load_targets>` 文本片段（system 段用；组装归 P11-06，本单产出片段与测试）。
  - `Hybrid`：加载后合并为一次变更、**下一回合**生效的数据接口（开纪元动作归 P03-05/P04；本单只产出新面输入）。
  - stub 调用参数不合完整 schema 时的 hint 文本（07 §6；接 P06-02 留的注入口）。
- 不做：
  - `<available_load_targets>` 的 system 段最终组装（P11-06）。
  - 「每回合至多一次变更」的引擎侧执行（P04/P03-05；本单交付数据接口）。
  - 扩展来源的 stub 与列举（脚本、MCP；P13）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-tools/src/{stub.rs,groups.rs}`（新增） | 形态渲染、分组与三态 |
| `crates/gqy-tools/src/surface.rs`（修改） | 接入 Stub/Hybrid 生成，删除 `ModeNotReady` 过渡分支 |
| `crates/gqy-tools/src/builtin/load_tools.rs`（新增） | `load_tools` 工具与结果模板 |
| `crates/gqy-tools/src/builtin/mod.rs`（修改） | `BUILTINS` 加一行 |
| `crates/gqy-config/src/defaults.rs`（修改） | `[tools] loading_mode = "stub"` 与分组默认值 |
| `crates/gqy-tools/tests/{stub_surface,load_tools}.rs`（新增） | 见「测试与守护」 |

## 接口草案

草案，以实现为准。

```rust
// gqy-tools/src/stub.rs（04 §8.3 的字节；模板唯一）
pub const STUB_PARAM_SHELL: &str = …;
pub fn stub_description(spec: &ToolSpec, example: Option<&str>) -> String;  // `… (stub)` / `… (stub, e.g. …)`

// gqy-tools/src/groups.rs（00 §5 三态；并入 07 §6 后实现）
pub enum GroupState { Full, Stub, Off }
pub fn effective_state(group: &str, persona: &PersonaSnapshot, mode: LoadingMode) -> GroupState;

// gqy-tools/src/builtin/load_tools.rs
pub struct LoadToolsArgs { pub names: Vec<String> }      // ≤ 16；`group:<name>` 项
pub fn render_contracts(specs: &[&ToolSpec]) -> String;  // <tool>…</tool> 段落字节
pub fn available_targets_text(catalog: &RegistryGeneration, persona: &PersonaSnapshot) -> String;
```

## 实施步骤

1. `groups.rs` 与三态（先按 README 复核清单第 1 条把决策并入 07 §6）。
2. `stub.rs` 与 `surface.rs` 接入（删除 `ModeNotReady`）；字节先用固定向量钉住。
3. `load_tools` 工具：结果模板、16 上限、`group:` 项、重复加载、hint。
4. `available_targets_text` 与 Hybrid 的数据接口。
5. `cargo xtask check --fast`、`cargo test -p gqy-tools`；提交：`feat(tools): stub 模式与 load_tools`。

## 测试与守护

- **stub 字节**：固定向量（真名 + 描述 + 参数壳）；改壳或改一字符描述 → 向量红（前缀契约）。
- **60 字符边界**：summary 61 字符 → 编译期校验报错（复用 P03-04 的校验器）。
- **三态与分组**：默认只全量基础组；`Off` 不进面；按组提升生效。
- **load_tools**：16 项上限；`group:` 展开；重复加载返回 `already loaded` 与哈希；结果段落字节与 Full 形态条目一致（同一序列化函数）。
- **hint**：直接调 stub 且参数不合完整 schema → 错误附 `hint: call load_tools …`。
- **确定性**：不同 cwd / HOME / 时区 / locale 下编译字节相等（07 §12.2）。
- 先红后绿对照（PR 贴输出）：把参数壳改成长壳（04 §8.3 以外）、去掉重复加载保护各一次。

## 验收流程

```sh
cargo test -p gqy-tools            # 全绿
cargo xtask check --fast
# 手检：stub 模式编译含 TestTool 的面 → 打印 tools 数组字节与 surface_hash；
#   load_tools 加载一个组 → 打印结果段落首尾；重复加载 → already loaded
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] stub 形态字节与 04 §8.3 一致（或按拍板后的并入口径），有固定向量
- [ ] `Full/Stub/Off` 三态与分组按并入后的 07 §6 实现，默认值集中在 `gqy-config`
- [ ] `load_tools` 的模板、上限、重复加载、hint 各有区分能力测试
- [ ] 不同环境下编译字节相等（07 §12.2）

## 风险与回退

- **两份文档的壳字节**：04 §8.3 与 07 §6 不一致（README 复核第 4 条）；拍板前按 04 §8.3 实现（与 P03-04 的校验器同口径），拍板后回填另一份。
- **Hybrid 的纪元接口**：本单只产新面数据；若 P03-05/P04 的接口不足，先停下来改设计再动代码。
- **回退**：新文件 + `surface.rs` 的接入删掉，即回到 `ModeNotReady` 的过渡态。

