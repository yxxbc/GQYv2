# P11-03 · persona 目录与代际切换

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P11-01 |
| 设计依据 | designs/12-人格配置与场所.md §5.1（目录格式）、§5.2（清单）、§5.3（内置 persona，无「模式」）、§5.4（代际）、§5.5（代际如何进入已有会话）、§5.6（公开版与私有版）、§10（dev 不是特例、Surface 冻结的守护）；designs/04-前缀缓存账本.md §9.1（`PersonaGeneration` 冷启动原因）、§9.2（persona 代际与跨文档冲突注记）；designs/13-网关与API协议.md §5.7（`GET/PUT /personas`）、§6.2（`persona.invalid`、`session.persona_generation`）；designs/10-存储与数据演进.md §7.6（`persona_generations` 占位）；designs/05-上下文与压缩.md §7.1（压缩采用新代际）；designs/19-可观测性与测试.md §3.4（`context_rewrite reason=persona_generation`）；designs/00-设计理念.md §5（「Persona 与 Preset 正交」）、§6（Q-12-2、Q-12-3、Q-12-5） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-config`（persona 清单解析与注册表）、`crates/gqy-store`（`persona_generations` 迁移与 repo）、`crates/gqy-engine`（代际切换接缝）、`crates/gqy-gateway`（`/personas` 路由）、`crates/gqy-daemon`（监视任务与组装）、`xtask`（`AgentMode` 扫描规则） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

persona 成为唯一的配置单位并被冻结进会话：目录被解析、校验、登记代际；`content_hash` 变化产生新 `generation`；编辑 persona 不碰已有会话的前缀——新代际只在「新会话 / 压缩 / `/persona reload`」三处生效（Q-12-2 推荐口径）。做完之后：`dev` 与任意自定义 persona 走同一条代码路径（复制 `dev` 改名后指纹相同）；群聊绑定没有 `public.md` 的 persona 报 `PersonaNotPublic`；代码里没有 `AgentMode`。

## 范围

- 做：
  - 清单解析与校验（在 P01-03 的加载骨架上扩展）：`id` 规则 `[a-z0-9][a-z0-9_-]{0,31}`；`[enable]` 的 `subsystems` / `toolsets` / `skills` / `mcp` / `scripts`；`[venues].allow`；`[model].tier`。未知子系统名 / 工具集名报错并列出合法值；退役名字常量表（例如 v1 的 `deep_research`）读到只 `warn`。
  - 内置 `dev`（`subsystems = []`、`toolsets = ["core"]`）与 `default`（全量）嵌入二进制；用户在 `personas/<id>/` 放同名目录时**整目录覆盖**，不做字段合并。
  - `PersonaSnapshot { id, generation, compatibility, content_hash, manifest, prompt_private, prompt_public, relations }`；`content_hash = blake3(清单规范化字节 ‖ 各提示词文件字节)`。
  - 代际登记：`persona_generations(persona_id, generation, content_hash, compatibility, created_at)` 追加型；加载时 `content_hash` 与最新行不同则插入并使 `generation + 1`。
  - 目录监视：每 2 s 检查 mtime；校验失败保留旧代际并发 `persona.invalid`（全局流，13 §6.2）。
  - 版本选择与安全边界：Owner / Member 私聊用 `prompt.md`；`External` 与任何群聊场所用 `public.md`，缺失时绑定报 `PersonaNotPublic`；`relations/<principal>.md` 只在 privileged 场所且 principal 匹配时注入。提示词中的“你不能……”不承担鉴权（铁律 7）。
  - 代际切换（Q-12-2，拍板前只做这一套）：进行中回合不变；已有会话后续回合用冻结代际；**压缩**与 `/persona reload` 时采用最新代际并记 `context_rewrite reason=persona_generation`、发 `session.persona_generation`；新会话用最新代际；`compatibility` 变化只提示用户，不自动切换。
  - API：`GET /personas`（列表 + 当前 generation）、`GET/PUT /personas/{name}`（原子写，写入产生新 generation）。
- 不做：
  - 场所注册与工具面编译（P11-04）；system 渲染（P11-06）；`/persona` 的 TUI 浮层（遗留）；v1 persona 导入（P16）；`persona_reminder` / `emotion` 子系统（Q-12-5：不进内核）；04 §9.2 的 `<persona-update>` 与「下一逻辑回合自动换纪元」（Q-12-2 拍板前不实现）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-config/src/persona/{manifest.rs,registry.rs,builtin.rs}`（新增 / 扩展） | 解析、校验、注册表、内置 persona |
| `crates/gqy-config/src/persona/watch.rs`（新增） | 2 s mtime 监视任务（注入 `Clock` 可快进） |
| `crates/gqy-store/migrations/*`（新增） | `persona_generations` 表（10 §7.6、§8 只增） |
| `crates/gqy-store/src/repo/persona.rs`（新增） | 追加与最新代际查询 |
| `crates/gqy-engine/src/session.rs`（修改） | 会话冻结 `(persona_id, generation)`；压缩与 `/persona reload` 的换代接缝 |
| `crates/gqy-gateway/src/routes/personas.rs`（新增） | `GET/PUT /personas` |
| `crates/gqy-daemon/src/assemble.rs`（修改） | 监视任务与注册表接线 |
| `xtask/src/arch/rules.rs`（修改） | 禁 `AgentMode` 标识符规则 + 违规样例 |

## 接口草案

草案，以实现为准。

```rust
pub struct PersonaSnapshot {
    pub id: PersonaId, pub generation: u32, pub compatibility: u32,
    pub content_hash: ContentHash, pub manifest: PersonaManifest,
    pub prompt_private: String, pub prompt_public: Option<String>,
    pub relations: BTreeMap<Principal, String>,
}

pub struct PersonaRegistry { /* ArcSwap<BTreeMap<PersonaId, PersonaSnapshot>> + watch 任务 */ }
impl PersonaRegistry {
    /// 最新代际；校验失败时返回旧快照（调用方据此发 `persona.invalid`）。
    pub fn latest(&self, id: &PersonaId) -> Option<Arc<PersonaSnapshot>>;
    pub fn reload(&self, id: &PersonaId) -> Result<GenerationChange, PersonaLoadError>;
}

/// 会话创建时选择提示词版本；群聊 / External 缺 public.md → Err(PersonaNotPublic)。
pub fn select_prompt(p: &PersonaSnapshot, trust: Trust, group: bool) -> Result<&str, PersonaError>;
```

## 实施步骤

1. 清单解析 + 校验 + 内置 persona（先写「未知子系统名报错列合法值」）。
2. `content_hash`、代际表与追加规则；复制 `dev` 改名 → 指纹断言。
3. 监视任务与 `persona.invalid`；坏文件保留旧代际的用例。
4. 冻结与切换接缝：压缩路径（P08 已就位，接上换代）与 `/persona reload`（`SessionCmd::Exclusive` 投影）。
5. `GET/PUT /personas`；xtask `AgentMode` 规则与违规样例。
6. `cargo xtask check`、`cargo test -p gqy-config -p gqy-engine`；提交：`feat(persona): persona 目录、代际与冻结`。

## 测试与守护

- **dev 不是特例**：复制 `dev` 为 `foo`（启用集相同）→ 两者 `content_hash` 与（P11-04 联测的）`surface_hash` 相同；代码里按名字 `"dev"` 分支会让该测试红。
- **`AgentMode` 扫描**：xtask 规则命中违规样例；去掉规则，样例测试红。
- **Surface 冻结**：会话创建后改 persona 提示词与 `[venues]` 配置，后续 10 轮请求的 system 与 tools 字节不变（与 P11-04 / P11-06 联测）。
- **代际生效**：执行压缩 → system 更新为新代际，恰好一条 `context_rewrite reason=persona_generation`；`/persona reload` 同样换一次且**不删除任何历史**（v1 缺陷 C1 的回归）。
- **公开版**：没有 `public.md` 的 persona 绑定群聊场所 → `PersonaNotPublic`（去掉检查，红）。
- **校验失败保留旧代际**：写坏 `persona.toml` → 旧代际继续服务 + `persona.invalid`。
- 先红后绿对照（PR 贴输出）：去掉冻结（直接读最新代际）、去掉 `PersonaNotPublic` 各一次。

## 验收流程

```sh
cargo xtask check
GQY2_HOME=$(mktemp -d) cargo run -p apps/gqy -- persona list
# 手检（daemon 运行中）：编辑 personas/<id>/prompt.md →
#   已有会话下一回合 system 字节不变；执行 /persona reload → system 更新、历史仍在、日志一条 persona_generation；
#   把无 public.md 的 persona 绑到群聊场所 → PersonaNotPublic
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] `persona_generations` 表经迁移只增落地，repo 无“整行替换”式写
- [ ] 冻结、代际生效、`PersonaNotPublic`、dev 特例、`AgentMode` 扫描各有区分能力测试
- [ ] Q-12-2 拍板前：仓库中不存在 `<persona-update>` 注入与自动换纪元路径（grep 佐证）
- [ ] 13 §5.7 的时机措辞已按 README 复核清单第 4 条回填

## 风险与回退

- **Q-12-2 未拍板**：若结论选 04 §9.2 方案，新增的是“下一回合换代 + `<persona-update>`”路径，本单的冻结与 `context_rewrite` 机制不变；不预先两套都写。
- **Preset 合并悬而未决**：`[enable]` 与 `[model]` 段的字段边界按 README 复核清单第 1 条先并入 12 再定；本单不发明 `Preset` 类型。
- **回退**：persona 注册表与监视可整体 revert（退回 P01-03 的最小加载）；已写入 `persona_generations` 的行是追加数据，不影响回退后的运行。
