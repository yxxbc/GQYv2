# P06-01 · Tool trait、注册表与工具面编译

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P05-13、P03-04 |
| 设计依据 | designs/07-工具系统.md §3.1（ToolSpec 与元数据）、§3.2（Tool trait 与 assess）、§3.3（ToolContext、ToolOutcome、ToolEffect）、§3.4（错误分类）、§4.1–§4.3（静态内置表、扩展来源、命名与冲突）、§5.1–§5.3（工具面编译、序列化、冻结与漂移）、§11（冻结面与实现不一致等失败模式）；designs/04-前缀缓存账本.md §8（工具面规范化）、§13（形状夹具）；designs/12-人格配置与场所.md §6.1–§6.2（Venue、信任级与交互性）；designs/08-权限审批与沙盒.md §6.5（代码约束：业务 crate 不出现 `target_os`）；designs/01-总体架构.md §3（层序：gqy-tools 在 L2） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-tools`（新增，L2）、`crates/gqy-core`（`ToolSpec` 等值类型）、`xtask/`（arch 扫描规则一条） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

`gqy-tools` 出现：Tool trait、元数据、静态注册表与注册表代际、工具面编译（筛选 → 排序 → 规范化 → `ToolSurface`）与冻结/漂移检查全部就位。做完之后：新增一个内置工具 = 一个文件 + `BUILTINS` 一行（07 §1）；工具面字节只经 P03-04 的规范化函数产出；daemon 重启后从冻结字节恢复，与当前注册表重编比对只记 `surface_drift`，不改已冻结字节。

## 范围

- 做：
  - 07 §3.1–§3.4 的类型逐项落地：`ToolSpec`（放 `gqy-core`）、`EffectClass`、`Residency`、`ToolMeta`、`Assessment`、`Tool`、`ToolContext`、`ToolOutcome`、`ToolEffect`、`Footprint`、`AuditFacts`、`Args` 与 `ToolErrorKind`。
  - 注册表：`ToolFactory`、`BUILTINS`（本单为空表 + 测试用 `TestTool`；内置工具随各自单登记）、`ToolSource`、`RegistryGeneration`（含 `generation` 与 `catalog_hash`）。
  - `compile_surface` 的筛选第 1–4 步照 07 §5.1（persona 启用集、场所信任、`interactive_only`、Host 能力），每一步记录被排除的工具与原因，供 `gqy doctor` 与 `/tools` 显示；档位不参与编译（第 5 条）。
  - 序列化与冻结照 07 §5.2–§5.3：排序后交给 P03-04 的 `normalize`；`freeze_surface` 把字节以 blob 落库并把 `surface_hash` 写会话记录；`check_drift` 与当前注册表重编比对，不同只记日志与会话标记；工具面超限时编译失败并列出超出数量（07 §11），不静默丢弃。
  - `LoadingMode` 先实现 `Full`；`Stub`/`Hybrid` 的形态生成归 P06-07（本单对这两个值返回 `SurfaceError::ModeNotReady`，**仅过渡**，P06-07 删除该分支）。
- 不做：
  - 分发管线与参数收口（P06-02）、stub 与 `load_tools`（P06-07）、任何具体内置工具（P06-03 起）。
  - `StorageDomain`/`DomainTable`：随第一个用它的工具（P06-03）落地。
  - 换面时机与 persona 代际（P11-04）、漂移换面（Q-07-3 待定）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-tools/{Cargo.toml,src/lib.rs}`（新增） | 新 crate（L2）与模块文档 |
| `crates/gqy-tools/src/{spec.rs,meta.rs,tool.rs,error.rs}`（新增） | 07 §3.1–§3.4 的类型、trait 与错误渲染骨架 |
| `crates/gqy-tools/src/{registry.rs,source.rs}`（新增） | 静态表、`ToolSource`、注册表代际 |
| `crates/gqy-tools/src/surface.rs`（新增） | `compile_surface`、`ToolSurface`、`freeze_surface`、`check_drift` |
| `crates/gqy-core/src/tools.rs`（新增/扩展） | `ToolSpec`、`ToolCall` 等 L0 值类型 |
| `crates/gqy-tools/tests/surface_compile.rs`（新增） | 见「测试与守护」 |
| `xtask/src/arch/scan.rs`（修改） | `gqy-tools` 业务代码不得出现 `cfg(target_os)`（08 §6.5 代码约束） |

## 接口草案

草案，以实现为准。

```rust
// gqy-tools/src/tool.rs（照 07 §3.2，签名以实现为准）
pub trait Tool: Send + Sync + 'static {
    fn spec(&self) -> &ToolSpec;
    fn meta(&self) -> &ToolMeta;
    /// 纯函数：不做 IO（路径规范化除外）；默认返回 meta.effect、无目标。
    fn assess(&self, ctx: &ToolContext, args: &Args) -> Result<Assessment, ToolError>;
    fn invoke<'a>(&'a self, ctx: ToolContext, args: Args) -> BoxFuture<'a, ToolOutcome>;
    fn resume(&self, ctx: &ToolContext, result: EffectResult) -> ToolOutcome;
}

// gqy-tools/src/surface.rs（照 07 §5.1–§5.3）
pub fn compile_surface(reg: &RegistryGeneration, persona: &PersonaSnapshot,
    venue: &Venue, host: &HostFacts, mode: LoadingMode) -> Result<ToolSurface, SurfaceError>;

pub struct ToolSurface {
    pub entries: Vec<ToolEntry>,          // 名字升序；stub 形态由 P06-07 填充
    pub normalized: NormalizedSurface,    // P03-04 的 bytes + surface_hash
    pub registry_generation: u64,
    pub mode: LoadingMode,
}
pub async fn freeze_surface(store: &Store, session: &SessionId, s: &ToolSurface) -> Result<(), SurfaceError>;
pub fn check_drift(s: &ToolSurface, reg: &RegistryGeneration) -> Option<SurfaceDrift>;
```

## 实施步骤

（L 单：每一步都能单独编译通过。）

1. crate 骨架、L0 值类型（`ToolSpec` 等）与 `Tool`/`ToolMeta`/`ToolErrorKind`；`TestTool` 仅用于测试（命名与文档标注防误用）。
2. 注册表、`ToolFactory`、`RegistryGeneration`（两代 `TestTool` → `catalog_hash` 不同）。
4. `compile_surface`：先写筛选四步的会红断言，再实现；接 P03-04 的 `normalize`；冻结、漂移与超限失败一并在本步收口。
5. `cargo xtask check --fast`、`cargo test -p gqy-tools -p gqy-core`；提交：`feat(tools): Tool trait、注册表与工具面编译`。

## 测试与守护

- **筛选四步**：每步一条会被排除的用例（未启用、信任不足、非交互、缺 Host 能力），并断言原因可读；删掉对应一步，此用例红（07 §5.1）。
- **确定性**：同一输入编译两次字节相等；打乱注册顺序输出不变（排序生效，04 §8.1）。
- **冻结与漂移**：改一个工具描述字符 → `check_drift` 命中、`surface_hash` 变化，冻结字节不动；去掉「从冻结字节恢复」路径测试红。
- **超限与扫描规则**：超上限的工具集 → 编译报错含超出数量与上限值；`gqy-tools` 里放一个 `cfg(target_os)` 违规 fixture，规则去掉即失效。
- 先红后绿对照（PR 贴输出）：删掉排序、删掉信任筛选各一次。

## 验收流程

```sh
cargo test -p gqy-tools -p gqy-core   # 全绿
cargo xtask arch                      # gqy-tools 只依赖更低层；target_os 规则覆盖 gqy-tools
cargo xtask check --fast
# 手检：用 TestTool 按乱序注册编译两份面 → 打印 surface_hash，应相等；改一个字符 → hash 变、check_drift 命中
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 07 §3.1–§3.4 的类型逐项落地，字段差异在 PR 描述里说明
- [ ] 筛选四步、确定性、冻结与漂移、超限各有区分能力测试
- [ ] `surface_hash` 只由 P03-04 的规范化函数产出，仓库里没有第二份序列化；`cfg(target_os)` 扫描规则覆盖 `gqy-tools`

## 风险与回退

- **与 P04-01 的边界**：`SessionSurface` 的承载与 `load` 在 P04-01；本单只提供编译与冻结写出，接口对不上时先改 07 §5.3 再改代码。
- **stub 分支的过渡**：`ModeNotReady` 只是接线顺序的产物；P06-07 未落地前不得让 `Stub` 进入任何会话记录；回退时 `gqy-tools` 是新 crate，连同 `xtask` 规则一起 revert 即可。

