# P11-06 · system 分段组装与 i18n

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P11-03、P11-05 |
| 设计依据 | designs/12-人格配置与场所.md §8（system 八节与顺序、空节省略、回合级信息不进 system）、§9（语言规则：模型可见文本英文、用户内容不限、界面走 i18n、`[ui].language`）、§10（system 确定性、英文扫描的守护）；designs/04-前缀缓存账本.md §4.1（`[S]` 布局与 system 段）、§8（工具面规范化与形状夹具；system 字节同样进夹具）；designs/09-感知矩阵.md §4.1（`environment` 节的来源）、§5.4（渲染格式与预算）；designs/11-记忆与知识库.md §10.3（`kb-index` 节的冻结时机）；designs/18-扩展体系.md §4.3（`skills-index`）；designs/07-工具系统.md §5.3（字节进形状夹具）；designs/19-可观测性与测试.md §6.1（第 13 条模型可见文本英文门禁）；designs/13-网关与API协议.md §7（API 错误 code + 参数，不返回拼好的中文）；designs/00-设计理念.md §5（「Schema 驱动多端生成」） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-ledger`（`SystemSection` 枚举与渲染入口）、`crates/gqy-config`（persona / audience / venue-policy 文本）、`crates/gqy-perception`（`environment` 节）、`crates/gqy-protocol`（i18n 键常量）、`apps/tui`（消息目录装载）、`xtask`（CJK 扫描门禁） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

system 段成为确定性的、可夹具钉住的字节：`SystemSection` 枚举按固定顺序遍历（persona → audience → relations → venue-policy → tool-guidelines → skills-index → kb-index → environment），每节一个渲染函数、节间固定一个空行、空节整节省略；输入是冻结的 `SessionSurface` 值，输出确定字节并进形状夹具。语言规则落地：模型可见文本英文集中在各 crate 的 `model_text` 模块并由 CJK 扫描门禁守护；界面文本走 i18n 目录（`zh-CN` / `en`），API 错误返回 `code` + 参数。做完之后：同一 `SessionSurface` 渲染 100 次字节相同；故意含中文的夹具让门禁报红。

## 范围

- 做：
  - `SystemSection` 枚举与遍历渲染器（12 §8）：节顺序即枚举顺序；新增一节 = 改枚举 + 刷新夹具；每节只有一个渲染函数（铁律 6）。
  - 八节的实现与来源：`persona`（`prompt.md` / `public.md`，外壳 `<persona name="…">`）；`audience`（Owner / Member / External / 群聊四段内置英文模板，Owner 场所附 owner 档案）；`relations`（仅 privileged，`<relationship>`）；`venue-policy`（按 `VenueKind` 的内置英文策略：输出格式、消息长度、是否可提问）；`tool-guidelines`（工具面中启用工具的使用准则，按工具名排序）；`skills-index` 与 `kb-index`（先建提供者接缝：无内容时整节省略；内容分别归 P13-04 / P12-07）；`environment`（OS、架构、shell、工作区根——09 中会话内不变的部分）。
  - 回合级信息（时间、cwd、发送者昵称、召回记忆、复盘提示）不进 system（12 §8 规则，铁律 1 的前置）。
  - 形状夹具：system 字节进 07 §12.1 的形状夹具比对（与工具面同一机制）；节顺序、外壳名、空节省略都有夹具。
  - i18n：`[ui].language`（默认跟随系统区域设置）；`gqy-protocol` 定义界面文本键常量；TUI 装载 `zh-CN` / `en` 消息目录（P09-04 的 `label_key` / `summary_key` 接上真实目录）；API 错误沿用 `code` + 参数。
  - 英文门禁：各 crate 的 `model_text` 模块（模型可见字符串常量集中处）+ xtask 扫描规则（出现 CJK 即红；persona 侧文本与 i18n 目录在白名单）；附一个故意含中文的夹具证明规则生效。
- 不做：
  - `kb-index` / `skills-index` 的内容生产（P12-07 / P13-04 提供；本单只定接缝与省略规则）；Web 侧消息目录的完整装载（随 P10-06 / P10-07 的 i18n 收敛）；TUI `/config` 菜单（遗留）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-ledger/src/system/{mod.rs,sections.rs}`（新增） | `SystemSection` 枚举与遍历渲染器 |
| `crates/gqy-config/src/model_text/*`（新增） | audience / venue-policy 内置英文模板与 persona 外壳 |
| `crates/gqy-perception/src/render.rs`（修改） | `environment` 节的 system 用法（复用 09 §5.4 渲染） |
| `crates/gqy-protocol/src/i18n.rs`（新增） | 界面文本键常量 |
| `apps/tui/src/i18n/*`（新增） | `zh-CN` / `en` 消息目录与选择逻辑 |
| `xtask/src/arch/cjk.rs`（新增） | CJK 扫描门禁 + 违规样例 |
| `crates/gqy-tools/tests/fixtures/shapes/*`（修改） | system 字节进夹具 |

## 接口草案

草案，以实现为准。

```rust
pub enum SystemSection {        // 顺序即枚举顺序（12 §8）
    Persona, Audience, Relations, VenuePolicy,
    ToolGuidelines, SkillsIndex, KbIndex, Environment,
}

/// 唯一渲染入口：输入是冻结的 `SessionSurface`，输出确定字节。
/// 空节整节省略；节间固定一个空行；外壳名与属性顺序固定。
pub fn render_system(surface: &SessionSurface, sources: &SectionSources) -> SystemBytes;

pub struct SectionSources {     // 组装处注入；无内容时返回 None（整节省略）
    pub skills_index: Option<Box<dyn Fn() -> String + Send + Sync>>,
    pub kb_index: Option<Box<dyn Fn() -> String + Send + Sync>>,
}
```

## 实施步骤

1. `SystemSection` 枚举与遍历渲染器（先写「节顺序与夹具一致」用例）。
2. persona / audience / relations / venue-policy 四节的文本与渲染。
3. tool-guidelines 与 environment 两节；skills / kb 接缝（空省略）。
4. 形状夹具接入 07 的比对器；确定性 100 次用例。
5. i18n 键常量、TUI 目录装载与 `[ui].language`。
6. CJK 扫描门禁 + 违规样例；`cargo xtask check`；提交：`feat(ledger): system 分段组装与 i18n 门禁`。

## 测试与守护

- **system 确定性**：同一 `SessionSurface` 渲染 100 次字节相同；节顺序与夹具一致（打乱枚举遍历顺序，红）。
- **空节省略**：kb / skills 无内容时不输出空外壳；输出空外壳，红。
- **回合级信息不进 system**：夹具断言 system 中不含时间、cwd、昵称、召回块（把 `now` 塞进 system，红）。
- **英文扫描**：故意含 CJK 的 `model_text` 夹具 → 门禁红；白名单（persona 文本、i18n 目录）不误伤。
- **i18n 回退**：`[ui].language = "en"` 与 `zh-CN` 各渲染一次；未知语言值 → 校验失败并列合法值。
- **API 错误形状**：错误体是 `code` + 参数，不含拼好的中文句子（夹具）。
- 先红后绿对照（PR 贴输出）：把 `now` 注入 system、去掉空节省略各一次。

## 验收流程

```sh
cargo xtask check
cargo test -p gqy-ledger -p gqy-config
# 手检：同一会话连续 3 轮 → system 字节不变（doctor --model-view 对照）；
#   临时在 model_text 里加一个中文字符串 → cargo xtask check 报红
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 八节顺序、外壳、空节省略进形状夹具；同输入渲染字节恒定
- [ ] CJK 扫描门禁进 CI 且有违规样例；白名单范围写清
- [ ] TUI 的 `label_key` / `summary_key` 接到真实消息目录；API 错误不含拼好的整句
- [ ] skills / kb 接缝的“无内容即省略”行为有测试，P12-07 / P13-04 可直接填

## 风险与回退

- **「Schema 驱动多端生成」尚未并入 15 / 14**：本单只落键常量与目录机制，不落 Web 渲染；开工前按 README 复核清单第 2 条处理。
- **文本来源边界**：`audience` / `venue-policy` 是内置常量（唯一来源在 `model_text`），persona 提示词是用户内容；两者不许混写。
- **回退**：渲染器可整体 revert 到“单块拼接”；i18n 目录与键常量单独 revert 不影响 P11 其余部分。
