# P01-03 · GqyPaths 与最小配置加载

> 创建：AI 助手（Cline 会话），2026-09-28。改动本文件时，按根 `AGENTS.md` 规矩 7 更新这一行：写清谁改的、什么时候改的。

| 项 | 值 |
| --- | --- |
| 状态 | 未开始 |
| 依赖 | P01-01 |
| 设计依据 | designs/10-存储与数据演进.md §3（数据目录）、§12（`[store]` 配置）；designs/12-人格配置与场所.md §3.1–§3.3；designs/19-可观测性与测试.md §4.1（TestHome）；00 §5 的 D-01（`~/.gqy2` / `GQY2_HOME`） |
| 规模 | L（2–3 天） |
| 涉及 crate / 目录 | `crates/gqy-config`、`xtask/`（扫描器加一条规则） |

## 目标

`gqy-config` 成为**路径与配置默认值的唯一来源**：`GqyPaths`（解析数据根、给全部布局路径、`ensure()` 建骨架并加固权限）、环境快照解析（纯函数，三平台在任何机器上都可测）、`TestHome`（测试隔离夹具）、最小配置加载（`config/gqy.toml` 的 `[store]` 段）。做完之后：daemon/CLI 的数据目录只有一个算法；所有测试用 `TestHome` 而不是 `GQY2_HOME`；P01-04 的 store 参数、P01-07 的 blob 路径都有出处。

## 范围

- 做：
  - `paths`：`GqyPaths` 与 10 §3 布局的**全部**访问器（`config/`、`personas/`、`extensions*/`、`kb/`、`files/`、`data/`、`data/blobs/`、`run/`、`logs/`、`backups/`、`cache/`、`import/` 以及 `gqy.db` 三件套、锁/令牌文件路径）；`GqyPaths::at(root)`（测试与内部使用）与 `from_snapshot(&EnvSnapshot)`。
  - `resolve`：`EnvSnapshot`（家目录、`GQY2_HOME`、平台）——**解析是纯函数**，测试喂快照就能在任意机器上验证三平台行为（参考 miyu 3-1 的实测做法；不改进程环境变量，避免并行测试互相污染）。规则：`GQY2_HOME` 必须绝对，相对 → 报错；空 → 当未设；家目录找不到 → 报错不猜；Linux/macOS 默认 `$HOME/.gqy2`；Windows 默认 `%LOCALAPPDATA%\gqy2`（仅保证编译与单测）。
  - `ensure()`：**只认自己的数据根**（10 §3）——根已存在、非空且无 `.gqy2-root` 标记 → `NotOurs` 报错、什么都不建；新建骨架时写 `.gqy2-root`（一行 `This directory is a GQYv2 data root (layout 1).`）并同步目录。创建缺失目录（幂等）、新目录 Unix `0700`、已有目录比 `0700` 宽时收紧并 `warn`（只处理本树内、非符号链接条目）；符号链接指向树外 → `PathEscapes`。`umask(0o077)` 助手（Unix；接线在 P05-05 启动时调用）。
  - `config`：最小 `Config { store: StoreConfig }`（10 §12 全键与默认值；**默认值只出现在 `Default` 实现里**，12 §3.3）；`load(path) -> Result<Config, ConfigError>`：文件缺失 → 全默认；坏 TOML → 带路径的错误；未知键不报错（保留写回在 P11-01，本单只读）。
  - `testkit`（feature `testkit`，dev-dependencies 开启）：`TestHome::new()` = `tempfile::TempDir` + `GqyPaths::at`（10 §3、19 §4.1）。
  - xtask 扫描器加一条规则（10 §3）：`".gqy2"` 字面量只允许出现在 `crates/gqy-config` 内。
- 不做：
  - 完整配置模式、`validate()`、热加载、保存与锁（P11-01）；secrets（P11-02）；persona（P11-03）。
  - Windows 的权限语义（不收紧、如实报告；10 §3 已定）。
  - 目录属主/ACL 检查、`gqy doctor` 的权限报告（P11-05）。
  - `GQY2_HOME` 指向文件而非目录时的诊断文案细节（报错即可；不做删除 / 替换）。

## 改动清单

| 文件 | 内容 |
| --- | --- |
| `Cargo.toml` | `[workspace.dependencies]` 加 `tempfile`？（若尚无）、`toml`；`gqy-config` 的依赖 |
| `crates/gqy-config/Cargo.toml` | 依赖 `gqy-core`、`toml`；dev 依赖 `tempfile` |
| `crates/gqy-config/src/paths.rs`（新增） | `GqyPaths` + 布局访问器 + `ensure()`/`PathEscapes` |
| `crates/gqy-config/src/resolve.rs`（新增） | `EnvSnapshot` + 纯解析 + `umask` 助手 |
| `crates/gqy-config/src/config.rs`（新增） | `Config` / `StoreConfig`（10 §12 全键） + `load()` |
| `crates/gqy-config/src/testkit.rs`（新增） | `TestHome`（feature `testkit`） |
| `crates/gqy-config/src/lib.rs` | 模块声明与文档 |
| `xtask/src/arch/scan.rs` 等 | 加 `.gqy2` 字面量规则 + fixture |

## 接口草案

草案，以实现为准。

```rust
// gqy-config/src/paths.rs

/// 数据目录的全部路径。所有路径只由这里计算；
/// 其他 crate 不得拼路径字符串（10 §3；xtask 拦 `.gqy2` 字面量）。
pub struct GqyPaths { root: PathBuf, /* … */ }

impl GqyPaths {
    /// 直接指定根（测试与内部使用；测试夹具 TestHome 用这个）。
    pub fn at(root: PathBuf) -> Self;

    /// 从环境快照解析（见 resolve.rs）。非测试代码用它。
    pub fn from_snapshot(snap: &EnvSnapshot) -> Result<Self, PathsError>;

    pub fn root(&self) -> &Path;
    pub fn config_dir(&self) -> PathBuf;
    pub fn gqy_toml(&self) -> PathBuf;
    pub fn secrets_toml(&self) -> PathBuf;
    pub fn data_dir(&self) -> PathBuf;
    pub fn db_path(&self) -> PathBuf;          // data/gqy.db
    pub fn blobs_dir(&self) -> PathBuf;        // data/blobs
    pub fn logs_dir(&self) -> PathBuf;
    pub fn run_dir(&self) -> PathBuf;
    pub fn backups_dir(&self) -> PathBuf;
    /* …布局里其余目录同理 */

    /// 建骨架：先认根——非空且无 `.gqy2-root` 标记 → `NotOurs`，什么都不建；
    /// 创建缺失目录（幂等）；新目录 0700；已有目录宽于 0700 时收紧并 warn；
    /// 符号链接指向树外 → `PathEscapes`；新建时写 `.gqy2-root` 并同步目录（10 §3）。
    pub fn ensure(&self) -> Result<(), PathsError>;
}
```

```rust
// gqy-config/src/resolve.rs

/// 解析数据根要看的环境事实，进程里读一次。
/// 测试喂快照：三平台的默认位置在任何一台机器上都测得到（参考 miyu 3-1）。
pub struct EnvSnapshot {
    pub home: Option<PathBuf>,
    pub gqy2_home: Option<String>,     // 原样；相对/空在解析里处置
    pub platform: Platform,
}

/// Unix 的 umask 收紧助手（Windows no-op）；由 daemon 启动时调用（P05-05）。
pub fn tighten_umask();
```

```rust
// gqy-config/src/config.rs

/// 最小配置：本阶段只需要 `[store]`（10 §12）。
/// 各字段默认值只在 `Default` 实现里出现一次（12 §3.3）。
#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct Config { pub store: StoreConfig }

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct StoreConfig {
    pub synchronous: Synchronous,          // full | normal
    pub group_commit_ms: u64,              // 5
    pub read_pool_size: u32,               // 4
    pub write_queue_timeout_ms: u64,       // 5000
    pub blob_inline_max: usize,            // 8192
    pub blob_gc_grace_hours: u64,          // 24
    pub backup: BackupConfig,              // daily_at: "03:00"
}

/// 文件缺失 → 全默认；坏 TOML → 带路径与行号的错误；未知键不报错。
pub fn load(path: &Path) -> Result<Config, ConfigError>;
```

## 实施步骤

1. 建 `gqy-config` 依赖与模块骨架；`cargo xtask arch` 绿（L1 只依赖 L0）。
2. `resolve.rs`：先写快照解析测试（三平台、`GQY2_HOME` 的三种形态、家目录缺失），再实现。
3. `paths.rs`：布局访问器对照 10 §3 逐行核一遍；`ensure()` 的幂等与权限行为。
4. `config.rs`：`[store]` 全键 + 默认值（照 10 §12 表）；`load` 的三种情形（缺失/正常/坏 TOML）。
5. `testkit`：`TestHome`；把所有新测试改用 `TestHome`（不读 `GQY2_HOME`）。
6. xtask 扫描器：`.gqy2` 字面量规则 + fixture（合规在 `gqy-config` 内、违规在别处）。
7. `cargo xtask check --fast`、`cargo test`；提交：`feat(config): GqyPaths、环境快照与最小配置`。

## 测试与守护

- **三平台解析（喂快照）**：Linux/macOS 默认 `$HOME/.gqy2`、Windows 默认 `%LOCALAPPDATA%\gqy2`；`GQY2_HOME` 覆盖默认；**相对路径的 `GQY2_HOME` 拒绝**、**空串当未设**；家目录/`LOCALAPPDATA` 缺失 → 报错（不猜、不落到当前目录）。
- **ensure()**：临时目录里建骨架 → 10 §3 的目录全部存在；建两次不出错；新目录 Unix 权限 0700；已有 0755 目录 → 收紧为 0700 并出 warn（tracing 测试订阅器断言）；目录里有文件时不移动/不删除；**非空外来目录（放一个文件 / 只放一个隐藏文件）→ `NotOurs`，一个字节没动**；带标记的目录照常（幂等）；新建后 `.gqy2-root` 存在、内容正确。**区分能力**：逐条改坏（不设 0700、少了某个目录、相对 `GQY2_HOME` 也收、空串当设了、不看 XDG 类变量（如涉及）、不查家目录缺失、不认标记也建、标记内容写错）→ 对应测试红。
- **符号链接**：树内指向树外 → `PathEscapes`；链接本身不被当作目录创建对象。
- **TestHome**：构造不读任何环境变量；进程里先设 `GQY2_HOME` 指向别处，TestHome 仍用临时目录（证明隔离）。
- **配置**：缺文件 → 所有字段与 `Default` 相等；`[store]` 每个键各改一次 → 读回生效；坏 TOML → 错误含路径；未知键 → 不报错。
- **扫描器**：`.gqy2` 字面量在 `crates/gqy-config` 内合规、在别处违规（fixture 断言）。
- **进程环境读取**：读进程家目录的那个函数在三平台 CI 各跑一遍（只找、不建、不删）。

## 验收流程

```sh
cargo test -p gqy-config           # 全绿
cargo xtask arch                   # 绿（含新扫描规则）
cargo xtask check --fast           # 全绿
# 手动冒烟（可选）：用 TestHome 的临时目录跑 ensure() 后
#   ls -la <临时目录>               # 10 §3 布局齐全；新目录权限 drwx------（Unix）
# 先红后绿对照（PR 贴输出）：
#   1) 相对 GQY2_HOME 也接受 → 对应测试红 → 恢复
#   2) 新目录不设 0700 → 权限测试红 → 恢复
#   3) 在 gqy-core 里写 "~/.gqy2" 字面量 → 扫描器红 → 恢复
```

## 完成判据

- [ ] 全局完成定义（施工总纲 §3.3）全部满足
- [ ] 三平台解析全部由快照测试覆盖（CI 三平台各跑一遍真实环境读取函数）
- [ ] `ensure()` 的幂等/权限/符号链接行为有测试；先红后绿证据齐全
- [ ] `[store]` 默认值与 10 §12 表逐项一致，且只存在于一处
- [ ] `TestHome` 被 gqy-config 自己的测试使用；P01-04 起其他 crate 复用
- [ ] 扫描器新规则有合规/违规 fixture

## 风险与回退

- **Windows 路径**：`%LOCALAPPDATA%` 的读取与权限分支只做编译保证与单测（00 §5 平台口径）；权限测试 `#[cfg(unix)]`。
- **`ensure()` 碰撞语义**：按 10 §3 已定（`.gqy2-root` + 拒绝）；错误信息写清目录与处置建议（把 `GQY2_HOME` 设到一个空目录，或先用迁移程序处理）。
- **配置未知键的保留**：本单只读，写回（保留注释与未知键）在 P11-01；不要为此在 P01-03 引入 `toml_edit`（届时再加）。
- **回退**：纯新增，revert 即可；不涉及真实数据（测试全在临时目录）。


