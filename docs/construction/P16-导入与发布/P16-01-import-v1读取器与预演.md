# P16-01 · import-v1 读取器与预演

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P12-08、P13-07 |
| 设计依据 | designs/20-v1数据导入.md §1（源只读、预演 → 核对 → 应用、目标必须新目录、不静默丢数据、确定性）、§2（v1 数据全景与 conversation.db 表清单）、§3（命令、参数、退出码）、§4.1–§4.3（discover / snapshot / plan）、§5.1–§5.5（映射表与配置转换）、§6（不导入清单）、§9（报告位置与 plan 字段）、§10（边界与失败模式）、§11（夹具 / 源不变 / 映射完整性 / 测试隔离）、§13（D-01、D-06、D-07、Q-20-2/3/4/5）；designs/10-存储与数据演进.md §3（数据目录与标记、`GQY2_HOME`）、§8（迁移只增）、§9.1（三件套规则；禁止对活库 `fs::copy`）；designs/12-人格配置与场所.md §3.2（配置段）、§5.2（persona 目录）、§8（owner 档案）；designs/18-扩展体系.md §7.3（扩展目录）、§10（哈希不符禁用）；designs/11-记忆与知识库.md §3.3（记忆表）；designs/19-可观测性与测试.md §4.1（`#[ignore = \"needs:…\"]` 与夹具口径） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-import-v1`（新增；复核第 1 条定落点）、`apps/gqy`（子命令挂载）、`tests/fixtures/v1/`（脱敏夹具）、`tests/`（源不变与映射完整性） |

> **草案（开工前复核）**：本单写于前序阶段实现之前；开工前先按当时代码的真实接口复核「接口草案」与「改动清单」，发现偏差先改设计文档、再改本单。

## 目标

`gqy import-v1`（D-06 拍板前按 `gqy2` 口径，一处常量）的只读前半程成立：discover 识别 `.layout-v1` / `.resource-layout-v1` / `.home-layout-v1` 标记与各库位置、检查 v1 daemon（套接字 / 锁）运行中退出 3；对将读取的每个文件记基线 `(路径, 大小, mtime, blake3)`；snapshot 把每个 SQLite 与 `-wal` / `-shm` 三件套复制进 staging 的 `source-snapshot/`，在副本上做检查点与 `PRAGMA quick_check` 后只读打开，结束重算源哈希、不一致即中止；plan 逐类别生成计划（源行数 / 目标行数 / 跳过及原因 / 文件字节 / 配置转换 / 不导入清单）并写出预演报告；预演**不创建目标**；退出码 `0/1/2/3/4` 与 20 §3 一致；映射是常量表驱动——每张 v1 表必须出现在 §5 映射表或 §6 不导入清单之一。

## 范围

- 做：
  - 上述流程与退出码；`--from/--to/--report/--include/--exclude/--sample/--seed` 解析与校验。
  - 布局标记与旧布局兼容（`state/conversation.db`、`data/personas/*`、`data/memory.db`）；`user_version` > 39 时按列存在性探测读取。
  - `home/<user>` 身份 → principal 映射（owner / member，Q-20-4）；JSONC 配置解析、密钥打码与转换预览。
  - 夹具（脱敏，覆盖 conversation.db v12 / v20 / v25 / v39）与「表必须登记」守护。
- 不做：
  - staging 构建与八项核对（P16-02）；commit 与回退（P16-03）；增量导入（Q-20-1）；写目标库。
  - `--apply`：本单解析到该参数直接报「apply 未实现（P16-03）」并退出 2，不做半套写入（见风险）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `crates/gqy-import-v1/src/{discover.rs,snapshot.rs,plan.rs,mapping.rs,report.rs,cli.rs}`（新增） | 只读读取器与预演（复核第 1 条定落点后开工） |
| `crates/gqy-import-v1/src/v1db.rs`（新增） | v1 库只读连接（副本）与列存在性探测 |
| `apps/gqy/src/main.rs`（修改） | 挂 `import-v1` 子命令 |
| `tests/fixtures/v1/`（新增） | 脱敏夹具：四版本 conversation.db、JSONC 配置、多身份、多 persona 记忆、附件 |
| `tests/import_reader.rs`（新增） | 源不变、映射完整、daemon 运行、退出码 |
| `xtask`（修改） | 夹具校验入口（生成工具在 v1 仓库时只做校验） |

## 接口草案

草案，以实现为准。

```rust
pub struct ImportOptions { pub from: PathBuf, pub to: PathBuf, pub report_dir: PathBuf,
                           pub include: CategorySet, pub exclude: CategorySet,
                           pub sample: u32, pub seed: u64, pub apply: bool }
pub struct FileStamp { pub path: String, pub size: u64, pub mtime_ms: i64, pub blake3: String }
pub enum ImportError { SourceBusy, TargetNotEmpty, Parameter(String), Blocked { reasons: Vec<BlockReason> }, SourceChanged }
/// 只读发现：布局标记、库位置、daemon 运行检测、列存在性。
pub fn discover(from: &Path) -> Result<SourceLayout, ImportError>;
/// 三件套复制到 staging/source-snapshot/ 并返回基线（副本上 checkpoint + quick_check）。
pub fn snapshot(from: &Path, staging: &Path) -> Result<Vec<FileStamp>, ImportError>;
/// 逐类别计划（读副本，不写目标）。
pub fn plan(src: &SourceLayout, opts: &ImportOptions) -> Result<Plan, ImportError>;
```

## 实施步骤

1. 落点与重依赖拍板（复核第 1 条）；夹具先落（四版本、脱敏）。
2. discover：布局标记、库定位、daemon 运行检测（退出 3）、列存在性探测。
3. snapshot：三件套复制、副本 `quick_check`、基线哈希与结束重算（先写「以读写方式打开源库 → 红」对照）。
4. plan：每类别行数与跳过原因、配置转换、不导入清单；映射常量表与「表必须登记」测试。
5. 报告（plan 部分）与退出码；`--apply` 拒绝路径与提示。
6. `cargo xtask check`；提交：`feat(import): import-v1 读取器与预演`。

## 测试与守护

- **源不变**：夹具目录导入前后完整哈希树一致；故意以读写方式打开源库，红（20 §11）。
- **映射完整性**：对每张 v1 表断言在映射表或不导入清单；夹具出现未登记表 → 测试失败（常量驱动）。
- **退出码**：v1 daemon 运行（假锁 / 假套接字）→ 3；目标非空 → 4；参数错 → 2；`quick_check` 失败 → 阻断、`--exclude` 后放行。
- **夹具覆盖**：四个 conversation.db 版本、摘要回合、隐藏回合、`running` 回合、附件、多身份、多 persona、JSONC 含注释与多 key。
- **不打正文**：终端输出与报告样本不含消息正文与密钥（断言子串）。
- 先红后绿对照（PR 贴输出）：去掉源哈希重算、去掉「表必须登记」检查各一次。

## 验收流程

```sh
cargo xtask check
cargo test -p gqy-import-v1
# 预演（只读；目标必须不存在或为空）：
export GQY2_HOME=$(mktemp -d)
cargo run -p gqy -- import-v1 --from tests/fixtures/v1/v39 --to "$GQY2_HOME"/target \
  --report /tmp/imp-report --sample 5 --seed 1
echo $?                          # 0（无阻断）
cat /tmp/imp-report/report.md    # 结论 → 待处理 → 统计 → 不导入清单
test ! -e "$GQY2_HOME"/target    # 预演不创建目标
# 失败路径：把 --to 指向非空目录 → 退出 4
# 真实数据演练（需用户环境）：--from ~/.gqy；先确认 v1 daemon 已停止
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 源只读（哈希树前后一致）；预演不创建目标（测试钉住）
- [ ] 映射表常量驱动；未登记表 → 测试失败
- [ ] 退出码 0/1/2/3/4 与 20 §3 一致；`--apply` 未接通前明确拒绝
- [ ] 夹具四版本与关键形态齐备；报告 plan 字段齐全且不含正文 / 密钥

## 风险与回退

- **中间态**：`--apply` 在 P16-03 接通；本单内解析到 `--apply` 直接报「apply 未实现（P16-03）」退出 2，不做半套写入。
- **v1 的新版本**：`user_version` > 39 按列存在性读，报告标注「未知的新版本」（20 §10）。
- **夹具来源**：由 v1 仓库测试工具生成并脱敏；v1 工具不可用时按 20 §2 表结构手工构造，PR 说明来源。
- **回退**：本单只读；删除报告目录即回到起点。


